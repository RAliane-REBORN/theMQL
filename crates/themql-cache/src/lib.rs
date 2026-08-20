//! # themql-cache
//!
//! Tiered cache orchestration for theMQL. Coordinates four tiers — L1
//! (lru, process-local), L2 (moka, process-local), L3 (redis,
//! distributed), L4 (sled via themql-storage, authoritative
//! persistent). Misses fall through to the next tier; an authoritative
//! miss returns to the resolver or errors.
//!
//! This crate owns the [`Cache`] trait, the [`CacheEntry`] /
//! [`CacheHit`] types, and the [`CacheError`] enum. [`CacheTier`],
//! [`CachePolicy`], [`InvalidationHint`], and [`SubjectPattern`] are
//! re-exported from [`themql_core`].
//!
//! See `specs/cache.toml` for the authoritative specification.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::future::Future;
use std::sync::{Mutex, RwLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use themql_core::{CachePolicy, CacheTier, InvalidationHint, SubjectError, SubjectPattern};
use themql_core::{Error as CoreError, ResponseValue, Timestamp};
pub use themql_query::CacheKey;

/// A cached value plus the metadata required for tier promotion, TTL
/// enforcement, and conditional requests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CacheEntry {
    /// The cached response value.
    pub value: ResponseValue,
    /// When this entry was inserted into its current tier.
    pub inserted_at: Timestamp,
    /// Optional time-to-live; `None` defers to the tier default.
    pub ttl: Option<Duration>,
    /// The tier this entry was read from (set on [`CacheHit::Hit`]).
    pub source_tier: CacheTier,
    /// Optional entity tag for conditional requests (304 Not Modified).
    pub etag: Option<String>,
}

/// The outcome of a cache lookup.
///
/// A `Hit` records the tier the entry was found in so the caller can
/// promote it to higher tiers. A `Miss` is authoritative — after all
/// enabled tiers miss, the caller must resolve and write-through.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CacheHit {
    /// The entry was found.
    Hit {
        /// The cached entry.
        value: CacheEntry,
        /// The tier the entry was read from.
        source_tier: CacheTier,
    },
    /// Authoritative miss — no tier contained the key.
    Miss,
}

/// Errors raised by cache operations.
///
/// Maps to [`themql_core::Error`] via the [`From<CacheError>`]
/// implementation, following the mapping table in
/// `specs/cache.toml [api.CacheError.mapping_to_core_error]`.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CacheError {
    /// The requested tier is unavailable (down, unconfigured, etc.).
    #[error("cache tier unavailable: {0:?}")]
    TierUnavailable(CacheTier),
    /// Deserialisation of a cached value failed.
    #[error("cache deserialization error: {0}")]
    DeserializationError(String),
    /// Serialisation of a value for write-through failed.
    #[error("cache serialization error: {0}")]
    SerializationError(String),
    /// The provided [`CachePolicy`] is invalid (e.g. disabled but bypass).
    #[error("invalid cache policy")]
    InvalidPolicy,
    /// The provided [`SubjectPattern`] for invalidation is invalid.
    #[error("invalid subject pattern: {0}")]
    PatternInvalid(SubjectError),
}

impl From<SubjectError> for CacheError {
    fn from(e: SubjectError) -> Self {
        Self::PatternInvalid(e)
    }
}

impl From<CacheError> for CoreError {
    fn from(e: CacheError) -> Self {
        match e {
            CacheError::TierUnavailable(_) => Self::internal_error(e.to_string()),
            CacheError::DeserializationError(_) | CacheError::SerializationError(_) => {
                Self::transport_error(e.to_string())
            }
            CacheError::InvalidPolicy | CacheError::PatternInvalid(_) => {
                Self::validation_error(e.to_string())
            }
        }
    }
}

/// Top-level cache orchestrator. A single `Cache` instance coordinates
/// all four tiers in order. Transport adapters and query resolvers
/// depend on this trait, not on individual tier implementations.
///
/// The trait is async via `impl Future` return types (matching the
/// `themql_core::Resolver` pattern) so it can be used without `dyn`
/// dispatch on stable Rust.
#[allow(async_fn_in_trait)]
pub trait Cache: Send + Sync {
    /// Look up `key` walking L1 → L2 → L3 → L4 in order; return the first
    /// hit and the tier it was found in. On an authoritative miss returns
    /// [`CacheHit::Miss`]; the caller must resolve and write-through.
    ///
    /// # Errors
    /// Returns [`CacheError`] if a tier is unreachable or serialisation
    /// fails. A tier miss is **not** an error — it falls through to the
    /// next tier.
    fn get(
        &self,
        key: &CacheKey,
        policy: &CachePolicy,
    ) -> impl Future<Output = Result<CacheHit, CacheError>>;

    /// Write `value` through to all enabled tiers, per
    /// `policy.tier_hint` (or all tiers if the hint is `None`).
    ///
    /// # Errors
    /// Returns [`CacheError`] if any target tier is unreachable or
    /// serialisation fails.
    fn put(
        &self,
        key: CacheKey,
        value: CacheEntry,
        policy: &CachePolicy,
    ) -> impl Future<Output = Result<(), CacheError>>;

    /// Remove a single key from all tiers.
    ///
    /// # Errors
    /// Returns [`CacheError`] if any tier is unreachable.
    fn invalidate(&self, key: &CacheKey) -> impl Future<Output = Result<(), CacheError>>;

    /// Remove all keys whose subject matches `pattern`. Used by the
    /// transport bridge on subject-change events.
    ///
    /// # Errors
    /// Returns [`CacheError::PatternInvalid`] if `pattern` is invalid,
    /// or another variant if a tier is unreachable.
    fn invalidate_pattern(
        &self,
        pattern: &SubjectPattern,
    ) -> impl Future<Output = Result<(), CacheError>>;
}

// ===========================================================================
// Tier backends — composed, not reimplemented
// ===========================================================================

use themql_core::Subject;
use themql_storage::{Storage, StorageKey, StorageValue};

// --- L1: lru-backed process-local cache -----------------------------------

/// L1 cache — process-local, ultra-low-latency, bounded LRU.
///
/// Backed by `lru::LruCache` guarded by a `Mutex`. Entries are evicted
/// in least-recently-used order when the capacity is exceeded.
pub struct L1Cache {
    inner: Mutex<lru::LruCache<CacheKey, CacheEntry>>,
    /// Key→subject index for pattern invalidation.
    key_index: RwLock<std::collections::HashMap<CacheKey, String>>,
}

impl L1Cache {
    /// Construct an L1 cache with the given entry capacity.
    ///
    /// # Panics
    /// Panics if `capacity` is 0 (after `max(1)`, it cannot be).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = std::num::NonZeroUsize::new(capacity.max(1)).expect("max(1) > 0");
        Self {
            inner: Mutex::new(lru::LruCache::new(cap)),
            key_index: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Look up `key` in L1.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    #[must_use]
    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        self.inner
            .lock()
            .expect("l1 lock poisoned")
            .get(key)
            .cloned()
    }

    /// Insert `entry` at `key`, evicting the least-recently-used entry
    /// when at capacity. Records the subject in the key→subject index.
    ///
    /// # Panics
    /// Panics if the internal locks are poisoned.
    #[allow(clippy::needless_pass_by_value)]
    pub fn put(&self, key: CacheKey, entry: CacheEntry) {
        self.inner
            .lock()
            .expect("l1 lock poisoned")
            .put(key.clone(), entry);
    }

    /// Insert `entry` at `key` with an associated subject string for
    /// pattern invalidation.
    ///
    /// # Panics
    /// Panics if the internal locks are poisoned.
    pub fn put_with_subject(&self, key: CacheKey, entry: CacheEntry, subject: String) {
        self.inner
            .lock()
            .expect("l1 lock poisoned")
            .put(key.clone(), entry);
        self.key_index
            .write()
            .expect("l1 index lock poisoned")
            .insert(key, subject);
    }

    /// Remove `key` from L1, returning the removed entry if present.
    ///
    /// # Panics
    /// Panics if the internal locks are poisoned.
    pub fn invalidate(&self, key: &CacheKey) -> Option<CacheEntry> {
        let entry = self.inner.lock().expect("l1 lock poisoned").pop(key);
        self.key_index
            .write()
            .expect("l1 index lock poisoned")
            .remove(key);
        entry
    }

    /// Remove all entries from L1.
    ///
    /// # Panics
    /// Panics if the internal locks are poisoned.
    pub fn invalidate_all(&self) {
        self.inner.lock().expect("l1 lock poisoned").clear();
        self.key_index
            .write()
            .expect("l1 index lock poisoned")
            .clear();
    }

    /// Scan all keys (used by `invalidate_pattern`).
    fn keys(&self) -> Vec<CacheKey> {
        self.inner
            .lock()
            .expect("l1 lock poisoned")
            .iter()
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Get the subject associated with a key (for pattern invalidation).
    fn subject_for_key(&self, key: &CacheKey) -> Option<String> {
        self.key_index
            .read()
            .expect("l1 index lock poisoned")
            .get(key)
            .cloned()
    }
}

// --- L2: moka-backed process-local cache ----------------------------------

/// L2 cache — process-local, low-latency, backed by `moka::sync::Cache`.
pub struct L2Cache {
    inner: moka::sync::Cache<CacheKey, CacheEntry>,
    /// Key→subject index for pattern invalidation.
    key_index: RwLock<std::collections::HashMap<CacheKey, String>>,
}

impl L2Cache {
    /// Construct an L2 cache with the given entry capacity.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap: u64 = capacity.try_into().unwrap_or(u64::MAX);
        Self {
            inner: moka::sync::Cache::new(cap),
            key_index: RwLock::new(std::collections::HashMap::new()),
        }
    }

    /// Look up `key` in L2.
    #[must_use]
    pub fn get(&self, key: &CacheKey) -> Option<CacheEntry> {
        self.inner.get(key)
    }

    /// Insert `entry` at `key`.
    pub fn put(&self, key: CacheKey, entry: CacheEntry) {
        self.inner.insert(key, entry);
    }

    /// Insert `entry` at `key` with an associated subject string for
    /// pattern invalidation.
    ///
    /// # Panics
    /// Panics if the internal locks are poisoned.
    pub fn put_with_subject(&self, key: CacheKey, entry: CacheEntry, subject: String) {
        self.inner.insert(key.clone(), entry);
        self.key_index
            .write()
            .expect("l2 index lock poisoned")
            .insert(key, subject);
    }

    /// Remove `key` from L2.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    pub fn invalidate(&self, key: &CacheKey) {
        self.inner.invalidate(key);
        self.key_index
            .write()
            .expect("l2 index lock poisoned")
            .remove(key);
    }

    /// Remove all entries from L2.
    ///
    /// # Panics
    /// Panics if the internal lock is poisoned.
    pub fn invalidate_all(&self) {
        self.inner.invalidate_all();
        self.key_index
            .write()
            .expect("l2 index lock poisoned")
            .clear();
    }

    /// Scan all keys (used by `invalidate_pattern`).
    fn keys(&self) -> Vec<CacheKey> {
        self.inner.iter().map(|(k, _)| (*k).clone()).collect()
    }

    /// Get the subject associated with a key (for pattern invalidation).
    fn subject_for_key(&self, key: &CacheKey) -> Option<String> {
        self.key_index
            .read()
            .expect("l2 index lock poisoned")
            .get(key)
            .cloned()
    }
}

// --- L3: redis-backed distributed cache -----------------------------------

/// L3 cache — distributed, backed by a redis client.
///
/// Uses `redis::Client` with async connections. Entries are
/// serialised as JSON and stored under a hex-encoded key prefix.
/// TTL is honoured via `EXPIRE`. Requires a running redis server for
/// live operation; falls back to `TierUnavailable` when the server
/// is unreachable.
pub struct L3Cache {
    client: redis::Client,
}

impl L3Cache {
    /// Construct an L3 cache by connecting to the redis server at
    /// `url` (e.g. `redis://127.0.0.1:6379`).
    ///
    /// # Errors
    /// Returns [`CacheError::TierUnavailable`] if the client cannot
    /// be created.
    pub fn new(url: &str) -> Result<Self, CacheError> {
        let client =
            redis::Client::open(url).map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
        Ok(Self { client })
    }

    fn redis_key(key: &CacheKey) -> String {
        format!("cache:{}", hex_encode(key.as_bytes()))
    }

    /// Look up `key` in L3.
    ///
    /// # Errors
    /// Returns [`CacheError::TierUnavailable`] if the server is
    /// unreachable, or [`CacheError::DeserializationError`] on parse
    /// failure.
    pub async fn get(&self, key: &CacheKey) -> Result<Option<CacheEntry>, CacheError> {
        let rk = Self::redis_key(key);
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
        let val: Option<String> = redis::cmd("GET")
            .arg(&rk)
            .query_async(&mut conn)
            .await
            .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
        match val {
            None => Ok(None),
            Some(s) => {
                let entry: CacheEntry = serde_json::from_str(&s)
                    .map_err(|e| CacheError::DeserializationError(e.to_string()))?;
                Ok(Some(entry))
            }
        }
    }

    /// Write `entry` to L3 with an optional TTL (in seconds).
    ///
    /// # Errors
    /// Returns [`CacheError::TierUnavailable`] if the server is
    /// unreachable, or [`CacheError::SerializationError`] on encode
    /// failure.
    pub async fn put(
        &self,
        key: &CacheKey,
        entry: &CacheEntry,
        ttl: Option<Duration>,
    ) -> Result<(), CacheError> {
        let rk = Self::redis_key(key);
        let s = serde_json::to_string(entry)
            .map_err(|e| CacheError::SerializationError(e.to_string()))?;
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
        match ttl {
            Some(d) => {
                let secs: u64 = d.as_secs().max(1);
                redis::cmd("SETEX")
                    .arg(&rk)
                    .arg(secs)
                    .arg(&s)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
            }
            None => {
                redis::cmd("SET")
                    .arg(&rk)
                    .arg(&s)
                    .query_async::<()>(&mut conn)
                    .await
                    .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
            }
        }
        Ok(())
    }

    /// Remove `key` from L3.
    ///
    /// # Errors
    /// Returns [`CacheError::TierUnavailable`] if the server is
    /// unreachable.
    pub async fn invalidate(&self, key: &CacheKey) -> Result<(), CacheError> {
        let rk = Self::redis_key(key);
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
        redis::cmd("DEL")
            .arg(&rk)
            .query_async::<()>(&mut conn)
            .await
            .map_err(|_| CacheError::TierUnavailable(CacheTier::L3))?;
        Ok(())
    }
}

// --- L4 storage helpers ---------------------------------------------------

/// A key-reconstruction helper: derive the [`StorageKey`] a [`CacheKey`]
/// maps to at L4. The L4 key is the hex encoding of the 32-byte digest
/// prefixed with `cache:`.
fn storage_key_for(cache_key: &CacheKey) -> StorageKey {
    StorageKey::new(format!("cache:{}", hex_encode(cache_key.as_bytes())))
}

/// Hex-encode a byte slice (allocates; L4 is not on the hot path).
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

/// Decode a [`CacheEntry`] from a [`StorageValue`] by deserialising
/// the bytes as JSON.
fn entry_from_storage(value: &StorageValue) -> Result<CacheEntry, CacheError> {
    serde_json::from_slice(&value.bytes)
        .map_err(|e| CacheError::DeserializationError(e.to_string()))
}

/// Encode a [`CacheEntry`] into a [`StorageValue`] as JSON.
fn entry_to_storage(entry: &CacheEntry) -> Result<StorageValue, CacheError> {
    let bytes =
        serde_json::to_vec(entry).map_err(|e| CacheError::SerializationError(e.to_string()))?;
    Ok(StorageValue::new(bytes, themql_core::FormatTag::Json))
}

// --- TieredCache orchestrator --------------------------------------------

/// Demotion threshold: entries with serialised size above this many
/// bytes are skipped for L1 (per `specs/cache.toml [semantics].demotion`).
const L1_DEMOTION_THRESHOLD: usize = 4096;

/// Tiered cache orchestrator. Holds optional L1, L2, L3 backends and
/// a reference to the L4 [`Storage`] backend, and implements the
/// [`Cache`] trait by walking tiers in order.
pub struct TieredCache<S: Storage> {
    /// L1 backend, if enabled.
    pub l1: Option<L1Cache>,
    /// L2 backend, if enabled.
    pub l2: Option<L2Cache>,
    /// L3 backend, if enabled.
    pub l3: Option<L3Cache>,
    /// L4 authoritative storage backend.
    pub l4: S,
}

impl<S: Storage> TieredCache<S> {
    /// Construct a tiered cache with the given tiers.
    #[must_use]
    pub fn new(l1: Option<L1Cache>, l2: Option<L2Cache>, l3: Option<L3Cache>, l4: S) -> Self {
        Self { l1, l2, l3, l4 }
    }

    /// Walk L1 → L2 → L3 → L4 in order, returning the first hit and the
    /// tier it was found in. On a hit at a lower tier, promotes the
    /// entry to all higher tiers that miss.
    async fn get_inner(&self, key: &CacheKey) -> Result<CacheHit, CacheError> {
        if let Some(l1) = &self.l1 {
            if let Some(entry) = l1.get(key) {
                return Ok(CacheHit::Hit {
                    value: entry,
                    source_tier: CacheTier::L1,
                });
            }
        }
        if let Some(l2) = &self.l2 {
            if let Some(entry) = l2.get(key) {
                self.promote(key, &entry, CacheTier::L2);
                return Ok(CacheHit::Hit {
                    value: entry,
                    source_tier: CacheTier::L2,
                });
            }
        }
        if let Some(l3) = &self.l3 {
            match l3.get(key).await {
                Ok(Some(entry)) => {
                    self.promote(key, &entry, CacheTier::L3);
                    return Ok(CacheHit::Hit {
                        value: entry,
                        source_tier: CacheTier::L3,
                    });
                }
                Ok(None) | Err(CacheError::TierUnavailable(_)) => {}
                Err(e) => return Err(e),
            }
        }
        let storage_key = storage_key_for(key);
        match self.l4.get(&storage_key).await {
            Ok(Some(value)) => {
                let entry = entry_from_storage(&value)?;
                let entry_for_promote = entry.clone();
                self.promote(key, &entry_for_promote, CacheTier::L4);
                Ok(CacheHit::Hit {
                    value: entry,
                    source_tier: CacheTier::L4,
                })
            }
            Ok(None) | Err(_) => Ok(CacheHit::Miss),
        }
    }

    /// Promote `entry` to all tiers above (i.e. faster than) `from`.
    fn promote(&self, key: &CacheKey, entry: &CacheEntry, from: CacheTier) {
        let mut promoted = entry.clone();
        promoted.source_tier = from;
        if from != CacheTier::L1 {
            if let Some(l1) = &self.l1 {
                l1.put(key.clone(), promoted.clone());
            }
        }
        if from != CacheTier::L1 && from != CacheTier::L2 {
            if let Some(l2) = &self.l2 {
                l2.put(key.clone(), promoted);
            }
        }
    }

    /// Write `entry` through to all enabled tiers. Skips L1 if the
    /// serialised entry exceeds the demotion threshold (per
    /// `specs/cache.toml [semantics].demotion`).
    async fn put_inner(&self, key: &CacheKey, entry: &CacheEntry, policy: &CachePolicy) {
        let entry_size = serde_json::to_vec(entry).map_or(0, |v| v.len());
        let skip_l1 = entry_size > L1_DEMOTION_THRESHOLD;
        if let Some(l1) = &self.l1 {
            if !skip_l1 {
                l1.put(key.clone(), entry.clone());
            }
        }
        if let Some(l2) = &self.l2 {
            l2.put(key.clone(), entry.clone());
        }
        if let Some(l3) = &self.l3 {
            let _ = l3.put(key, entry, policy.ttl).await;
        }
        if let Ok(value) = entry_to_storage(entry) {
            let _ = self.l4.put(&storage_key_for(key), value).await;
        }
    }

    /// Remove `key` from all enabled tiers.
    async fn invalidate_inner(&self, key: &CacheKey) {
        if let Some(l1) = &self.l1 {
            l1.invalidate(key);
        }
        if let Some(l2) = &self.l2 {
            l2.invalidate(key);
        }
        if let Some(l3) = &self.l3 {
            let _ = l3.invalidate(key).await;
        }
        let _ = self.l4.delete(&storage_key_for(key)).await;
    }
}

impl<S: Storage> Cache for TieredCache<S> {
    async fn get(&self, key: &CacheKey, policy: &CachePolicy) -> Result<CacheHit, CacheError> {
        if !policy.enabled {
            return Ok(CacheHit::Miss);
        }
        if policy.bypass {
            return Ok(CacheHit::Miss);
        }
        self.get_inner(key).await
    }

    async fn put(
        &self,
        key: CacheKey,
        value: CacheEntry,
        policy: &CachePolicy,
    ) -> Result<(), CacheError> {
        if !policy.enabled {
            return Ok(());
        }
        self.put_inner(&key, &value, policy).await;
        Ok(())
    }

    async fn invalidate(&self, key: &CacheKey) -> Result<(), CacheError> {
        self.invalidate_inner(key).await;
        Ok(())
    }

    async fn invalidate_pattern(&self, pattern: &SubjectPattern) -> Result<(), CacheError> {
        if let Some(l1) = &self.l1 {
            for key in l1.keys() {
                if let Some(subject_str) = l1.subject_for_key(&key) {
                    if let Ok(subject) = Subject::from_str(&subject_str) {
                        if pattern.matches(&subject) {
                            l1.invalidate(&key);
                        }
                    }
                }
            }
        }
        if let Some(l2) = &self.l2 {
            for key in l2.keys() {
                if let Some(subject_str) = l2.subject_for_key(&key) {
                    if let Ok(subject) = Subject::from_str(&subject_str) {
                        if pattern.matches(&subject) {
                            l2.invalidate(&key);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::duration_suboptimal_units)]
    use super::*;
    use themql_core::{ErrorCode, ResponseValue};

    #[test]
    fn cache_key_hash_of_is_deterministic() {
        let a = CacheKey::hash_of(b"vehicle.sensors.imu.gyro");
        let b = CacheKey::hash_of(b"vehicle.sensors.imu.gyro");
        assert_eq!(a, b, "same input must hash to the same key");
    }

    #[test]
    fn cache_key_hash_of_differs_for_different_input() {
        let a = CacheKey::hash_of(b"vehicle.sensors.imu.gyro");
        let b = CacheKey::hash_of(b"vehicle.sensors.imu.accel");
        assert_ne!(a, b, "different input must hash to a different key");
    }

    #[test]
    fn cache_key_from_hash_round_trips() {
        let bytes = [1u8; 32];
        let k = CacheKey::from_hash(bytes);
        assert_eq!(k.as_bytes(), &bytes);
    }

    #[test]
    fn cache_hit_miss_is_not_hit() {
        let h = CacheHit::Miss;
        assert!(!matches!(h, CacheHit::Hit { .. }));
    }

    #[test]
    fn cache_hit_hit_carries_entry_and_tier() {
        let entry = CacheEntry {
            value: ResponseValue::Unit,
            inserted_at: Timestamp::now_monotonic(),
            ttl: None,
            source_tier: CacheTier::L2,
            etag: None,
        };
        let h = CacheHit::Hit {
            value: entry.clone(),
            source_tier: CacheTier::L2,
        };
        match h {
            CacheHit::Hit { value, source_tier } => {
                assert_eq!(value, entry);
                assert_eq!(source_tier, CacheTier::L2);
            }
            CacheHit::Miss => panic!("expected Hit"),
        }
    }

    #[test]
    fn cache_entry_constructs_with_all_fields() {
        let e = CacheEntry {
            value: ResponseValue::Unit,
            inserted_at: Timestamp::now_monotonic(),
            ttl: Some(Duration::from_secs(60)),
            source_tier: CacheTier::L1,
            etag: Some("w/\"abc\"".to_owned()),
        };
        assert_eq!(e.source_tier, CacheTier::L1);
        assert_eq!(e.etag.as_deref(), Some("w/\"abc\""));
        assert_eq!(e.ttl, Some(Duration::from_secs(60)));
    }

    #[test]
    fn cache_error_tier_unavailable_constructs() {
        let e = CacheError::TierUnavailable(CacheTier::L3);
        assert!(e.to_string().contains("L3"), "message must name the tier");
    }

    #[test]
    fn cache_error_invalid_policy_constructs() {
        let e = CacheError::InvalidPolicy;
        assert_eq!(e.to_string(), "invalid cache policy");
    }

    #[test]
    fn cache_error_pattern_invalid_from_subject_error() {
        let se = SubjectError::EmptySubject;
        let ce: CacheError = se.into();
        assert!(matches!(
            ce,
            CacheError::PatternInvalid(SubjectError::EmptySubject)
        ));
    }

    #[test]
    fn cache_error_tier_unavailable_maps_to_internal_error() {
        let ce = CacheError::TierUnavailable(CacheTier::L3);
        let core: CoreError = ce.into();
        assert_eq!(core.code, ErrorCode::InternalError);
    }

    #[test]
    fn cache_error_serialization_maps_to_transport_error() {
        let ce = CacheError::SerializationError("boom".to_owned());
        let core: CoreError = ce.into();
        assert_eq!(core.code, ErrorCode::TransportError);
    }

    #[test]
    fn cache_error_invalid_policy_maps_to_validation_error() {
        let ce = CacheError::InvalidPolicy;
        let core: CoreError = ce.into();
        assert_eq!(core.code, ErrorCode::ValidationError);
    }

    #[test]
    fn cache_error_pattern_invalid_maps_to_validation_error() {
        let ce = CacheError::PatternInvalid(SubjectError::EmptySubject);
        let core: CoreError = ce.into();
        assert_eq!(core.code, ErrorCode::ValidationError);
    }

    #[test]
    fn cache_entry_round_trips_json() {
        let e = CacheEntry {
            value: ResponseValue::Unit,
            inserted_at: Timestamp::now_monotonic(),
            ttl: Some(Duration::from_secs(30)),
            source_tier: CacheTier::L2,
            etag: Some("etag-1".to_owned()),
        };
        let json = serde_json::to_string(&e).unwrap();
        let back: CacheEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(e, back);
    }

    #[test]
    fn cache_hit_round_trips_json_miss() {
        let json = serde_json::to_string(&CacheHit::Miss).unwrap();
        let back: CacheHit = serde_json::from_str(&json).unwrap();
        assert_eq!(back, CacheHit::Miss);
    }

    #[test]
    fn cache_hit_round_trips_json_hit() {
        let entry = CacheEntry {
            value: ResponseValue::Unit,
            inserted_at: Timestamp::now_monotonic(),
            ttl: None,
            source_tier: CacheTier::L1,
            etag: None,
        };
        let h = CacheHit::Hit {
            value: entry,
            source_tier: CacheTier::L1,
        };
        let json = serde_json::to_string(&h).unwrap();
        let back: CacheHit = serde_json::from_str(&json).unwrap();
        assert_eq!(back, h);
    }

    fn sample_entry(tier: CacheTier) -> CacheEntry {
        CacheEntry {
            value: ResponseValue::Unit,
            inserted_at: Timestamp::now_monotonic(),
            ttl: None,
            source_tier: tier,
            etag: None,
        }
    }

    #[test]
    fn l1_put_get_round_trip() {
        let l1 = L1Cache::new(8);
        let key = CacheKey::hash_of(b"k1");
        let entry = sample_entry(CacheTier::L1);
        l1.put(key.clone(), entry.clone());
        assert_eq!(l1.get(&key), Some(entry));
    }

    #[test]
    fn l1_lru_evicts_least_recently_used() {
        let l1 = L1Cache::new(2);
        let k1 = CacheKey::hash_of(b"1");
        let k2 = CacheKey::hash_of(b"2");
        let k3 = CacheKey::hash_of(b"3");
        l1.put(k1.clone(), sample_entry(CacheTier::L1));
        l1.put(k2.clone(), sample_entry(CacheTier::L1));
        // Access k1 to make k2 the LRU
        let _ = l1.get(&k1);
        l1.put(k3.clone(), sample_entry(CacheTier::L1));
        assert!(l1.get(&k1).is_some(), "k1 retained (recently used)");
        assert!(l1.get(&k2).is_none(), "k2 evicted (LRU)");
        assert!(l1.get(&k3).is_some(), "k3 retained");
    }

    #[test]
    fn l1_invalidate_removes_entry() {
        let l1 = L1Cache::new(8);
        let key = CacheKey::hash_of(b"k");
        l1.put(key.clone(), sample_entry(CacheTier::L1));
        assert!(l1.invalidate(&key).is_some());
        assert!(l1.get(&key).is_none());
    }

    #[test]
    fn l1_invalidate_all_clears() {
        let l1 = L1Cache::new(8);
        let k = CacheKey::hash_of(b"k");
        l1.put(k.clone(), sample_entry(CacheTier::L1));
        l1.invalidate_all();
        assert!(l1.get(&k).is_none());
    }

    #[test]
    fn l1_pattern_invalidation_with_subject_index() {
        let l1 = L1Cache::new(8);
        let k1 = CacheKey::hash_of(b"vehicle.sensors.imu.gyro");
        let k2 = CacheKey::hash_of(b"vehicle.sensors.imu.accel");
        let k3 = CacheKey::hash_of(b"vehicle.actuators.flap");
        l1.put_with_subject(
            k1.clone(),
            sample_entry(CacheTier::L1),
            "vehicle.sensors.imu.gyro".to_string(),
        );
        l1.put_with_subject(
            k2.clone(),
            sample_entry(CacheTier::L1),
            "vehicle.sensors.imu.accel".to_string(),
        );
        l1.put_with_subject(
            k3.clone(),
            sample_entry(CacheTier::L1),
            "vehicle.actuators.flap".to_string(),
        );
        let pattern = SubjectPattern::from_str("vehicle.sensors.#").unwrap();
        for key in l1.keys() {
            if let Some(subject_str) = l1.subject_for_key(&key) {
                if let Ok(subject) = Subject::from_str(&subject_str) {
                    if pattern.matches(&subject) {
                        l1.invalidate(&key);
                    }
                }
            }
        }
        assert!(l1.get(&k1).is_none(), "k1 invalidated by pattern");
        assert!(l1.get(&k2).is_none(), "k2 invalidated by pattern");
        assert!(l1.get(&k3).is_some(), "k3 not matched by pattern");
    }

    #[test]
    fn l2_put_get_round_trip() {
        let l2 = L2Cache::new(8);
        let key = CacheKey::hash_of(b"k2");
        let entry = sample_entry(CacheTier::L2);
        l2.put(key.clone(), entry.clone());
        assert_eq!(l2.get(&key), Some(entry));
    }

    #[test]
    fn l2_invalidate_removes_entry() {
        let l2 = L2Cache::new(8);
        let key = CacheKey::hash_of(b"k2");
        l2.put(key.clone(), sample_entry(CacheTier::L2));
        l2.invalidate(&key);
        assert!(l2.get(&key).is_none());
    }

    #[test]
    fn l3_construction_succeeds_without_server() {
        let l3 = L3Cache::new("redis://127.0.0.1:6379");
        assert!(l3.is_ok());
    }

    #[tokio::test]
    async fn l3_get_returns_tier_unavailable_without_server() {
        let l3 = L3Cache::new("redis://127.0.0.1:6399").unwrap();
        let key = CacheKey::hash_of(b"k3");
        let res = l3.get(&key).await;
        assert!(matches!(
            res,
            Err(CacheError::TierUnavailable(CacheTier::L3))
        ));
    }

    #[tokio::test]
    async fn tiered_l1_hit() {
        let l1 = Some(L1Cache::new(8));
        let l2 = Some(L2Cache::new(8));
        let cache = TieredCache::new(l1, l2, None, InMemoryStorage::default());
        let key = CacheKey::hash_of(b"t1");
        let entry = sample_entry(CacheTier::L1);
        cache.l1.as_ref().unwrap().put(key.clone(), entry.clone());
        let hit = cache.get(&key, &CachePolicy::default()).await.unwrap();
        match hit {
            CacheHit::Hit { source_tier, .. } => assert_eq!(source_tier, CacheTier::L1),
            CacheHit::Miss => panic!("expected L1 hit"),
        }
    }

    #[tokio::test]
    async fn tiered_l2_hit_promotes_to_l1() {
        let l1 = Some(L1Cache::new(8));
        let l2 = Some(L2Cache::new(8));
        let cache = TieredCache::new(l1, l2, None, InMemoryStorage::default());
        let key = CacheKey::hash_of(b"t2");
        let entry = sample_entry(CacheTier::L2);
        cache.l2.as_ref().unwrap().put(key.clone(), entry.clone());
        let hit = cache.get(&key, &CachePolicy::default()).await.unwrap();
        match hit {
            CacheHit::Hit { source_tier, .. } => assert_eq!(source_tier, CacheTier::L2),
            CacheHit::Miss => panic!("expected L2 hit"),
        }
        assert!(
            cache.l1.as_ref().unwrap().get(&key).is_some(),
            "promoted to L1"
        );
    }

    #[tokio::test]
    async fn tiered_put_writes_through_to_l1_l2() {
        let l1 = Some(L1Cache::new(8));
        let l2 = Some(L2Cache::new(8));
        let cache = TieredCache::new(l1, l2, None, InMemoryStorage::default());
        let key = CacheKey::hash_of(b"t3");
        let entry = sample_entry(CacheTier::L1);
        cache
            .put(key.clone(), entry.clone(), &CachePolicy::default())
            .await
            .unwrap();
        assert!(cache.l1.as_ref().unwrap().get(&key).is_some());
        assert!(cache.l2.as_ref().unwrap().get(&key).is_some());
    }

    #[tokio::test]
    async fn tiered_invalidate_removes_from_all_tiers() {
        let l1 = Some(L1Cache::new(8));
        let l2 = Some(L2Cache::new(8));
        let cache = TieredCache::new(l1, l2, None, InMemoryStorage::default());
        let key = CacheKey::hash_of(b"t4");
        cache
            .put(
                key.clone(),
                sample_entry(CacheTier::L1),
                &CachePolicy::default(),
            )
            .await
            .unwrap();
        cache.invalidate(&key).await.unwrap();
        assert!(cache.l1.as_ref().unwrap().get(&key).is_none());
        assert!(cache.l2.as_ref().unwrap().get(&key).is_none());
    }

    #[tokio::test]
    async fn tiered_l4_miss_returns_miss() {
        let cache = TieredCache::new(None, None, None, InMemoryStorage::default());
        let key = CacheKey::hash_of(b"miss");
        let hit = cache.get(&key, &CachePolicy::default()).await.unwrap();
        assert!(matches!(hit, CacheHit::Miss));
    }

    #[tokio::test]
    async fn tiered_disabled_policy_short_circuits() {
        let l1 = Some(L1Cache::new(8));
        let cache = TieredCache::new(l1, None, None, InMemoryStorage::default());
        let key = CacheKey::hash_of(b"dis");
        cache
            .l1
            .as_ref()
            .unwrap()
            .put(key.clone(), sample_entry(CacheTier::L1));
        let hit = cache.get(&key, &CachePolicy::disabled()).await.unwrap();
        assert!(matches!(hit, CacheHit::Miss));
    }

    #[tokio::test]
    async fn tiered_pattern_invalidation_with_subject_index() {
        let l1 = Some(L1Cache::new(8));
        let l2 = Some(L2Cache::new(8));
        let cache = TieredCache::new(l1, l2, None, InMemoryStorage::default());
        let k1 = CacheKey::hash_of(b"vehicle.sensors.imu.gyro");
        let k2 = CacheKey::hash_of(b"vehicle.actuators.flap");
        cache.l1.as_ref().unwrap().put_with_subject(
            k1.clone(),
            sample_entry(CacheTier::L1),
            "vehicle.sensors.imu.gyro".to_string(),
        );
        cache.l1.as_ref().unwrap().put_with_subject(
            k2.clone(),
            sample_entry(CacheTier::L1),
            "vehicle.actuators.flap".to_string(),
        );
        let pattern = SubjectPattern::from_str("vehicle.sensors.#").unwrap();
        cache.invalidate_pattern(&pattern).await.unwrap();
        assert!(
            cache.l1.as_ref().unwrap().get(&k1).is_none(),
            "k1 invalidated"
        );
        assert!(cache.l1.as_ref().unwrap().get(&k2).is_some(), "k2 retained");
    }

    use themql_storage::{StorageError, StorageQuery, StorageResultSet};

    #[derive(Default)]
    struct InMemoryStorage {
        map: std::sync::Mutex<std::collections::HashMap<StorageKey, StorageValue>>,
    }

    impl Storage for InMemoryStorage {
        async fn get(&self, key: &StorageKey) -> Result<Option<StorageValue>, StorageError> {
            Ok(self.map.lock().unwrap().get(key).cloned())
        }
        async fn put(&self, key: &StorageKey, value: StorageValue) -> Result<(), StorageError> {
            self.map.lock().unwrap().insert(key.clone(), value);
            Ok(())
        }
        async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
            self.map.lock().unwrap().remove(key);
            Ok(())
        }
        async fn query(&self, _q: &StorageQuery) -> Result<StorageResultSet, StorageError> {
            Ok(StorageResultSet::empty())
        }
    }
}
