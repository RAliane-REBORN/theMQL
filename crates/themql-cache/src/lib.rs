//! # themql-cache
//!
//! Tiered cache orchestration for theMQL. Coordinates four tiers — L1
//! (cachelito, process-local), L2 (moka, process-local), L3 (valkey,
//! distributed), L4 (helix-db, authoritative persistent). Misses fall
//! through to the next tier; an authoritative miss returns to the
//! resolver or errors.
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
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use themql_core::{CachePolicy, CacheTier, InvalidationHint, SubjectError, SubjectPattern};
use themql_core::{Error as CoreError, ResponseValue, Timestamp};

/// A deterministic, content-addressed cache key (32-byte BLAKE3 hash).
///
/// The hash is computed from the canonical serialisation of the query
/// (resource subject, selection, arguments, projection). Implementations
/// of [`CacheKeyer`] produce keys; the key itself is opaque bytes so it
/// can index any tier without transport-specific encoding.
///
/// Defined locally in `themql-cache` (not re-exported from `themql-query`)
/// so this crate does not take a hard dependency on `themql-query`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey(
    /// The 32-byte BLAKE3 digest.
    pub [u8; 32],
);

impl CacheKey {
    /// Construct a cache key from pre-computed bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Construct a cache key by hashing the given input with BLAKE3.
    #[must_use]
    pub fn hash_of(input: &[u8]) -> Self {
        Self(blake3::hash(input).into())
    }

    /// The raw 32-byte digest.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
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
    fn cache_key_from_bytes_round_trips() {
        let bytes = [1u8; 32];
        let k = CacheKey::from_bytes(bytes);
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
            ttl: Some(Duration::from_mins(1)),
            source_tier: CacheTier::L1,
            etag: Some("w/\"abc\"".to_owned()),
        };
        assert_eq!(e.source_tier, CacheTier::L1);
        assert_eq!(e.etag.as_deref(), Some("w/\"abc\""));
        assert_eq!(e.ttl, Some(Duration::from_mins(1)));
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
}
