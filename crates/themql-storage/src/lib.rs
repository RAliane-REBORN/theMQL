//! # themql-storage
//!
//! Authoritative storage adapter for theMQL. Wraps `sled` as the L4
//! authoritative tier of the cache stack and the durable persistence
//! layer for telemetry, model artifacts, and query results that must
//! survive process restarts.
//!
//! This crate owns the [`Storage`], [`StorageReader`], and
//! [`StorageWriter`] traits plus the [`StorageKey`] / [`StorageValue`] /
//! [`StorageQuery`] / [`StorageResultSet`] / [`StorageError`] types.
//! The traits are storage-agnostic — `sled` is one implementation, not
//! a hard requirement of the trait surface.
//!
//! See `specs/storage.toml` for the authoritative specification.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::future::Future;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use themql_core::{Error as CoreError, FormatTag, Subject, SubjectPattern};

/// Opaque, stable-hash storage key.
///
/// Wraps a `String` that is the canonical, deterministic key for a stored
/// value. Implementations derive the inner string from the resource
/// subject and (optionally) a content hash.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StorageKey(
    /// The opaque key string.
    pub String,
);

impl StorageKey {
    /// Construct a storage key from an opaque string.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The inner opaque string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A stored value: raw bytes plus the format tag identifying how to
/// decode them.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StorageValue {
    /// The raw encoded bytes.
    pub bytes: Vec<u8>,
    /// The format identifying the encoding of `bytes`.
    pub format: FormatTag,
}

impl StorageValue {
    /// Construct a storage value from bytes and a format tag.
    #[must_use]
    pub fn new(bytes: Vec<u8>, format: FormatTag) -> Self {
        Self { bytes, format }
    }
}

/// A query against storage. Mirrors three access patterns: direct key
/// lookup, subject-pattern scan, and a predicate-tree query expressed
/// as a JSON value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StorageQuery {
    /// Look up a single entry by exact key.
    ByKey(StorageKey),
    /// Scan all entries whose subject matches `pattern`.
    BySubjectPattern(SubjectPattern),
    /// Run a predicate query expressed in a simple JSON DSL with
    /// `field`, `op`, and `value` keys.
    ByPredicate(serde_json::Value),
}

/// A page of results from a [`StorageQuery`]. `has_more` and `cursor`
/// together support pagination — pass `cursor` back to the next query
/// to resume scanning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StorageResultSet {
    /// The matched (key, value) pairs in this page.
    pub entries: Vec<(StorageKey, StorageValue)>,
    /// Whether more results are available beyond this page.
    pub has_more: bool,
    /// Opaque cursor for the next page; `None` when exhausted.
    pub cursor: Option<String>,
}

impl StorageResultSet {
    /// Construct an empty result set (no entries, no cursor).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
            has_more: false,
            cursor: None,
        }
    }

    /// Whether this result set contains no entries.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Errors raised by storage operations.
///
/// Maps to [`themql_core::Error`] via the [`From<StorageError>`]
/// implementation, per `specs/storage.toml [error_model]`: connection /
/// timeout / internal failures map to [`ErrorCode::InternalError`];
/// not-found / already-exists / query failures map to
/// [`ErrorCode::ResolverError`]; serialisation failures map to
/// [`ErrorCode::TransportError`] (the value could not be moved across
/// the storage boundary).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StorageError {
    /// The requested key was not found.
    #[error("storage key not found")]
    NotFound,
    /// An entry with the given key already exists (on a create-only op).
    #[error("storage key already exists")]
    AlreadyExists,
    /// The connection to the storage backend could not be established.
    #[error("storage connection failed")]
    ConnectionFailed,
    /// The operation did not complete before its deadline.
    #[error("storage operation timed out")]
    Timeout,
    /// Serialisation of a value for write, or deserialisation on read,
    /// failed.
    #[error("storage serialization error: {0}")]
    SerializationError(String),
    /// A predicate query could not be executed (malformed DSL, unsupported
    /// operator, etc.).
    #[error("storage query error: {0}")]
    QueryError(String),
    /// An unexpected internal failure in the storage backend.
    #[error("storage internal error")]
    InternalError,
}

impl From<StorageError> for CoreError {
    fn from(e: StorageError) -> Self {
        match e {
            StorageError::NotFound | StorageError::AlreadyExists | StorageError::QueryError(_) => {
                Self::resolver_error(e.to_string())
            }
            StorageError::SerializationError(_) => Self::transport_error(e.to_string()),
            StorageError::ConnectionFailed
            | StorageError::Timeout
            | StorageError::InternalError => Self::internal_error(e.to_string()),
        }
    }
}

/// Top-level storage trait combining read, write, delete, and query.
///
/// Implementations wrap `sled` (L4 authoritative) or any other
/// storage backend that can fulfil the contract. Async via
/// `impl Future` return types (matching the `themql_core::Resolver`
/// pattern).
#[allow(async_fn_in_trait)]
pub trait Storage: Send + Sync {
    /// Read the value at `key`, if present.
    ///
    /// # Errors
    /// Returns [`StorageError::NotFound`] only when a distinguishing
    /// not-found is required; otherwise return `Ok(None)`. Other
    /// failures return the appropriate [`StorageError`] variant.
    fn get(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<Option<StorageValue>, StorageError>>;

    /// Write `value` at `key`, overwriting any existing entry.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the write cannot be durably
    /// acknowledged.
    fn put(
        &self,
        key: &StorageKey,
        value: StorageValue,
    ) -> impl Future<Output = Result<(), StorageError>>;

    /// Delete the entry at `key`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the delete fails.
    fn delete(&self, key: &StorageKey) -> impl Future<Output = Result<(), StorageError>>;

    /// Run `q` against storage, returning a page of results.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the query cannot be executed.
    fn query(
        &self,
        q: &StorageQuery,
    ) -> impl Future<Output = Result<StorageResultSet, StorageError>>;
}

/// Read-only view of storage for query resolvers. Composed of `get` +
/// `query` only — resolvers must not mutate.
#[allow(async_fn_in_trait)]
pub trait StorageReader: Send + Sync {
    /// Read the value at `key`, if present.
    ///
    /// # Errors
    /// Returns [`StorageError`] on failure.
    fn get(
        &self,
        key: &StorageKey,
    ) -> impl Future<Output = Result<Option<StorageValue>, StorageError>>;

    /// Run `q` against storage, returning a page of results.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the query cannot be executed.
    fn query(
        &self,
        q: &StorageQuery,
    ) -> impl Future<Output = Result<StorageResultSet, StorageError>>;
}

/// Write-only view of storage for telemetry ingest and artifact
/// persistence. Composed of `put` + `delete` only — writers must not
/// read.
#[allow(async_fn_in_trait)]
pub trait StorageWriter: Send + Sync {
    /// Write `value` at `key`, overwriting any existing entry.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the write cannot be durably
    /// acknowledged.
    fn put(
        &self,
        key: &StorageKey,
        value: StorageValue,
    ) -> impl Future<Output = Result<(), StorageError>>;

    /// Delete the entry at `key`.
    ///
    /// # Errors
    /// Returns [`StorageError`] if the delete fails.
    fn delete(&self, key: &StorageKey) -> impl Future<Output = Result<(), StorageError>>;
}

// ---------------------------------------------------------------------------
// SledStorage backend — embedded key-value store (disk-backed)
// ---------------------------------------------------------------------------

/// Embedded key-value storage backed by `sled`. Disk-backed, durable
/// across restarts, and suitable for the L4 authoritative tier. No
/// external server required.
///
/// Keys are serialised as `StorageKey` via bincode; values as
/// `StorageValue` via bincode. Subject-pattern queries scan the keyspace
/// and parse each key back to a `Subject` for matching. Predicate
/// queries do a simple JSON field-equality scan.
pub struct SledStorage {
    db: sled::Db,
}

impl SledStorage {
    /// Open a `SledStorage` at the given filesystem path.
    ///
    /// # Errors
    /// Returns [`StorageError::ConnectionFailed`] if sled cannot open
    /// the database at `path`.
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, StorageError> {
        let db = sled::open(path).map_err(|_| StorageError::ConnectionFailed)?;
        Ok(Self { db })
    }

    /// Open a temporary `SledStorage` backed by a temp directory.
    /// Useful for tests.
    ///
    /// # Errors
    /// Returns [`StorageError::ConnectionFailed`] on failure.
    pub fn open_temp() -> Result<Self, StorageError> {
        let dir = std::env::temp_dir().join(format!(
            "themql-sled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0u128, |d| d.as_nanos()),
        ));
        Self::open(dir)
    }

    fn serialize_key(key: &StorageKey) -> Result<Vec<u8>, StorageError> {
        bincode::serialize(key).map_err(|e| StorageError::SerializationError(e.to_string()))
    }

    fn deserialize_key(bytes: &[u8]) -> Result<StorageKey, StorageError> {
        bincode::deserialize(bytes).map_err(|e| StorageError::SerializationError(e.to_string()))
    }

    fn serialize_value(value: &StorageValue) -> Result<Vec<u8>, StorageError> {
        bincode::serialize(value).map_err(|e| StorageError::SerializationError(e.to_string()))
    }

    fn deserialize_value(bytes: &[u8]) -> Result<StorageValue, StorageError> {
        bincode::deserialize(bytes).map_err(|e| StorageError::SerializationError(e.to_string()))
    }
}

#[allow(async_fn_in_trait)]
impl Storage for SledStorage {
    async fn get(&self, key: &StorageKey) -> Result<Option<StorageValue>, StorageError> {
        let kb = Self::serialize_key(key)?;
        match self.db.get(kb).map_err(|_| StorageError::InternalError)? {
            None => Ok(None),
            Some(vb) => Ok(Some(Self::deserialize_value(&vb)?)),
        }
    }

    async fn put(&self, key: &StorageKey, value: StorageValue) -> Result<(), StorageError> {
        let kb = Self::serialize_key(key)?;
        let vb = Self::serialize_value(&value)?;
        self.db
            .insert(kb, vb)
            .map_err(|_| StorageError::InternalError)?;
        Ok(())
    }

    async fn delete(&self, key: &StorageKey) -> Result<(), StorageError> {
        let kb = Self::serialize_key(key)?;
        self.db
            .remove(kb)
            .map_err(|_| StorageError::InternalError)?;
        Ok(())
    }

    async fn query(&self, q: &StorageQuery) -> Result<StorageResultSet, StorageError> {
        match q {
            StorageQuery::ByKey(key) => {
                let kb = Self::serialize_key(key)?;
                match self.db.get(kb).map_err(|_| StorageError::InternalError)? {
                    None => Ok(StorageResultSet::empty()),
                    Some(vb) => {
                        let v = Self::deserialize_value(&vb)?;
                        Ok(StorageResultSet {
                            entries: vec![(key.clone(), v)],
                            has_more: false,
                            cursor: None,
                        })
                    }
                }
            }
            StorageQuery::BySubjectPattern(pattern) => {
                let mut entries = Vec::new();
                for item in self.db.iter() {
                    let (kb, vb) = item.map_err(|_| StorageError::InternalError)?;
                    let sk = Self::deserialize_key(&kb)?;
                    if let Some(subject) = subject_from_storage_key(&sk) {
                        if pattern.matches(&subject) {
                            let v = Self::deserialize_value(&vb)?;
                            entries.push((sk, v));
                        }
                    }
                }
                Ok(StorageResultSet {
                    entries,
                    has_more: false,
                    cursor: None,
                })
            }
            StorageQuery::ByPredicate(pred) => {
                let field = pred.get("field").and_then(|v| v.as_str()).ok_or_else(|| {
                    StorageError::QueryError("predicate requires 'field'".to_owned())
                })?;
                let target_val = pred.get("value");
                let mut entries = Vec::new();
                for item in self.db.iter() {
                    let (kb, vb) = item.map_err(|_| StorageError::InternalError)?;
                    let sk = Self::deserialize_key(&kb)?;
                    let v = Self::deserialize_value(&vb)?;
                    if predicate_matches(field, target_val, &sk) {
                        entries.push((sk, v));
                    }
                }
                Ok(StorageResultSet {
                    entries,
                    has_more: false,
                    cursor: None,
                })
            }
        }
    }
}

/// Simple predicate matching: checks if the storage key contains the
/// field string and optionally matches the value.
fn predicate_matches(
    field: &str,
    target_val: Option<&serde_json::Value>,
    key: &StorageKey,
) -> bool {
    if !key.as_str().contains(field) {
        return false;
    }
    match target_val {
        Some(serde_json::Value::String(s)) => key.as_str().contains(s.as_str()),
        Some(serde_json::Value::Number(n)) => key.as_str().contains(&n.to_string()),
        _ => true,
    }
}

/// Backward-compatible alias. `HelixStorage` is now backed by sled.
/// Use `SledStorage` for new code.
pub type HelixStorage = SledStorage;

impl HelixStorage {
    /// Construct a `HelixStorage` backed by a temporary sled database.
    ///
    /// # Panics
    /// Panics if sled cannot open a temporary directory.
    #[must_use]
    #[allow(clippy::missing_panics_doc)]
    pub fn new_in_memory() -> Self {
        Self::open_temp().expect("sled temp open")
    }
}

impl Default for HelixStorage {
    fn default() -> Self {
        Self::new_in_memory()
    }
}

/// Best-effort reconstruction of a [`Subject`] from a [`StorageKey`].
/// Storage keys are opaque strings; if the key is a dot-joined
/// subject, it is parsed back into a `Subject`. Otherwise `None` is
/// returned and the key is skipped by pattern queries.
fn subject_from_storage_key(key: &StorageKey) -> Option<Subject> {
    Subject::from_str(key.as_str()).ok()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use themql_core::{ErrorCode, SubjectPattern};

    #[test]
    fn storage_key_new_and_as_str() {
        let k = StorageKey::new("vehicle.sensors.imu.gyro/abc123");
        assert_eq!(k.as_str(), "vehicle.sensors.imu.gyro/abc123");
        assert_eq!(k.0, "vehicle.sensors.imu.gyro/abc123");
    }

    #[test]
    fn storage_key_eq_and_hash() {
        let a = StorageKey::new("k1");
        let b = StorageKey::new("k1");
        let c = StorageKey::new("k2");
        assert_eq!(a, b);
        assert_ne!(a, c);
        let mut set = std::collections::HashSet::new();
        set.insert(a);
        set.insert(b);
        assert_eq!(set.len(), 1, "equal keys collapse in a set");
    }

    #[test]
    fn storage_key_round_trips_json() {
        let k = StorageKey::new("opaque-key");
        let json = serde_json::to_string(&k).unwrap();
        assert_eq!(json, "\"opaque-key\"", "transparent serialisation");
        let back: StorageKey = serde_json::from_str(&json).unwrap();
        assert_eq!(back, k);
    }

    #[test]
    fn storage_value_new() {
        let v = StorageValue::new(vec![1, 2, 3], FormatTag::Json);
        assert_eq!(v.bytes, vec![1, 2, 3]);
        assert_eq!(v.format, FormatTag::Json);
    }

    #[test]
    fn storage_value_eq() {
        let a = StorageValue::new(vec![1], FormatTag::Json);
        let b = StorageValue::new(vec![1], FormatTag::Json);
        let c = StorageValue::new(vec![1], FormatTag::Bincode);
        assert_eq!(a, b);
        assert_ne!(a, c, "different format => different value");
    }

    #[test]
    fn storage_value_round_trips_json() {
        let v = StorageValue::new(vec![10, 20, 30], FormatTag::Postcard);
        let json = serde_json::to_string(&v).unwrap();
        let back: StorageValue = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn storage_query_by_key() {
        let q = StorageQuery::ByKey(StorageKey::new("k"));
        match q {
            StorageQuery::ByKey(k) => assert_eq!(k.as_str(), "k"),
            _ => panic!("expected ByKey"),
        }
    }

    #[test]
    fn storage_query_by_subject_pattern() {
        let p = SubjectPattern::from_str("vehicle.#").unwrap();
        let q = StorageQuery::BySubjectPattern(p);
        match &q {
            StorageQuery::BySubjectPattern(p) => assert_eq!(p.as_str(), "vehicle.#"),
            _ => panic!("expected BySubjectPattern"),
        }
    }

    #[test]
    fn storage_query_by_predicate() {
        let pred = serde_json::json!({"field": "position.x", "op": "gt", "value": 0});
        let q = StorageQuery::ByPredicate(pred.clone());
        match q {
            StorageQuery::ByPredicate(v) => assert_eq!(v, pred),
            _ => panic!("expected ByPredicate"),
        }
    }

    #[test]
    fn storage_result_set_empty_is_empty() {
        let rs = StorageResultSet::empty();
        assert!(rs.is_empty());
        assert!(!rs.has_more);
        assert!(rs.cursor.is_none());
    }

    #[test]
    fn storage_result_set_with_entries() {
        let rs = StorageResultSet {
            entries: vec![(
                StorageKey::new("k1"),
                StorageValue::new(vec![1], FormatTag::Json),
            )],
            has_more: true,
            cursor: Some("cursor-abc".to_owned()),
        };
        assert!(!rs.is_empty());
        assert_eq!(rs.entries.len(), 1);
        assert!(rs.has_more);
        assert_eq!(rs.cursor.as_deref(), Some("cursor-abc"));
    }

    #[test]
    fn storage_result_set_round_trips_json() {
        let rs = StorageResultSet {
            entries: vec![
                (
                    StorageKey::new("a"),
                    StorageValue::new(vec![1], FormatTag::Json),
                ),
                (
                    StorageKey::new("b"),
                    StorageValue::new(vec![2], FormatTag::Bincode),
                ),
            ],
            has_more: false,
            cursor: None,
        };
        let json = serde_json::to_string(&rs).unwrap();
        let back: StorageResultSet = serde_json::from_str(&json).unwrap();
        assert_eq!(back, rs);
    }

    #[test]
    fn storage_error_not_found_constructs() {
        let e = StorageError::NotFound;
        assert_eq!(e.to_string(), "storage key not found");
    }

    #[test]
    fn storage_error_already_exists_constructs() {
        let e = StorageError::AlreadyExists;
        assert!(e.to_string().contains("already exists"));
    }

    #[test]
    fn storage_error_query_error_carries_message() {
        let e = StorageError::QueryError("malformed predicate".to_owned());
        assert!(e.to_string().contains("malformed predicate"));
    }

    #[test]
    fn storage_error_serialization_error_carries_message() {
        let e = StorageError::SerializationError("decode failed".to_owned());
        assert!(e.to_string().contains("decode failed"));
    }

    #[test]
    fn storage_error_not_found_maps_to_resolver_error() {
        let se = StorageError::NotFound;
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::ResolverError);
    }

    #[test]
    fn storage_error_already_exists_maps_to_resolver_error() {
        let se = StorageError::AlreadyExists;
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::ResolverError);
    }

    #[test]
    fn storage_error_query_error_maps_to_resolver_error() {
        let se = StorageError::QueryError("bad".to_owned());
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::ResolverError);
    }

    #[test]
    fn storage_error_serialization_maps_to_transport_error() {
        let se = StorageError::SerializationError("bad".to_owned());
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::TransportError);
    }

    #[test]
    fn storage_error_connection_failed_maps_to_internal_error() {
        let se = StorageError::ConnectionFailed;
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::InternalError);
    }

    #[test]
    fn storage_error_timeout_maps_to_internal_error() {
        let se = StorageError::Timeout;
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::InternalError);
    }

    #[test]
    fn storage_error_internal_error_maps_to_internal_error() {
        let se = StorageError::InternalError;
        let core: CoreError = se.into();
        assert_eq!(core.code, ErrorCode::InternalError);
    }

    // --- SledStorage backend ------------------------------------------

    fn sv(b: &[u8]) -> StorageValue {
        StorageValue::new(b.to_vec(), FormatTag::Json)
    }

    #[tokio::test]
    async fn sled_put_get_round_trip() {
        let s = SledStorage::open_temp().unwrap();
        let k = StorageKey::new("k1");
        s.put(&k, sv(&[1, 2, 3])).await.unwrap();
        let got = s.get(&k).await.unwrap();
        assert_eq!(got, Some(sv(&[1, 2, 3])));
    }

    #[tokio::test]
    async fn sled_delete_removes_entry() {
        let s = SledStorage::open_temp().unwrap();
        let k = StorageKey::new("k");
        s.put(&k, sv(&[1])).await.unwrap();
        s.delete(&k).await.unwrap();
        assert_eq!(s.get(&k).await.unwrap(), None);
    }

    #[tokio::test]
    async fn sled_get_not_found_returns_none() {
        let s = SledStorage::open_temp().unwrap();
        let k = StorageKey::new("absent");
        assert_eq!(s.get(&k).await.unwrap(), None);
    }

    #[tokio::test]
    async fn sled_query_by_key() {
        let s = SledStorage::open_temp().unwrap();
        let k = StorageKey::new("qk");
        s.put(&k, sv(&[9])).await.unwrap();
        let rs = s.query(&StorageQuery::ByKey(k.clone())).await.unwrap();
        assert_eq!(rs.entries.len(), 1);
        assert_eq!(rs.entries[0].0, k);
        assert!(!rs.has_more);
    }

    #[tokio::test]
    async fn sled_query_by_subject_pattern() {
        let s = SledStorage::open_temp().unwrap();
        let k1 = StorageKey::new("vehicle.sensors.imu.gyro");
        let k2 = StorageKey::new("vehicle.sensors.imu.accel");
        let k3 = StorageKey::new("vehicle.actuators.flap");
        s.put(&k1, sv(&[1])).await.unwrap();
        s.put(&k2, sv(&[2])).await.unwrap();
        s.put(&k3, sv(&[3])).await.unwrap();
        let p = SubjectPattern::from_str("vehicle.sensors.#").unwrap();
        let rs = s
            .query(&StorageQuery::BySubjectPattern(p.clone()))
            .await
            .unwrap();
        assert_eq!(rs.entries.len(), 2, "pattern matches two sensors keys");
    }

    #[tokio::test]
    async fn sled_query_by_predicate_with_field() {
        let s = SledStorage::open_temp().unwrap();
        let k1 = StorageKey::new("vehicle.sensors.imu.gyro");
        let k2 = StorageKey::new("vehicle.actuators.flap");
        s.put(&k1, sv(&[1])).await.unwrap();
        s.put(&k2, sv(&[2])).await.unwrap();
        let pred = serde_json::json!({"field": "sensors", "value": "imu"});
        let rs = s.query(&StorageQuery::ByPredicate(pred)).await.unwrap();
        assert_eq!(rs.entries.len(), 1);
        assert_eq!(rs.entries[0].0, k1);
    }

    #[tokio::test]
    async fn sled_query_by_predicate_missing_field_errors() {
        let s = SledStorage::open_temp().unwrap();
        let pred = serde_json::json!({"op": "gt", "value": 0});
        let res = s.query(&StorageQuery::ByPredicate(pred)).await;
        assert!(matches!(res, Err(StorageError::QueryError(_))));
    }

    #[tokio::test]
    async fn helix_alias_works() {
        let s = HelixStorage::new_in_memory();
        let k = StorageKey::new("alias-test");
        s.put(&k, sv(&[42])).await.unwrap();
        assert_eq!(s.get(&k).await.unwrap(), Some(sv(&[42])));
    }
}
