//! # themql-query
//!
//! Query resolution, caching, and transport-projection specialisation.
//!
//! The canonical `Query`, `Context`, `Response`, and `Error` types are
//! defined in [`themql_core`] (see `specs/core.toml`). This crate
//! specialises how queries are resolved, cached, and batched around the
//! core [`themql_core::Resolver`] and [`themql_core::QueryExecutor`]
//! traits; it does not redefine the canonical types.
//!
//! ## Authority
//!
//! See `specs/query.toml` for the authoritative specification. The
//! `QueryExecutor` trait is declared in `themql_core` so transport
//! adapters can depend on the trait without a hard dep on this crate;
//! implementations live here. `CacheKeyer`, `CacheKey`, `Batcher`, and
//! `QueryError` are declared in this crate per `specs/query.toml [api]`.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::fmt;
use std::future::Future;

use serde::{Deserialize, Serialize};

// Re-exports — canonical types live in themql_core; re-exported here so
// downstream code can depend on themql-query alone for query concerns.
// The `QueryExecutor` trait is declared in themql_core per spec (so
// transport adapters can depend on the trait without a hard dep on this
// crate); implementations live here.
pub use themql_core::{
    CachePolicy, CacheTier, Context, Error, ErrorCode, Query, QueryExecutor, Resolver, Response,
    SubjectPattern,
};

// ===========================================================================
// CacheKey — opaque 32-byte hash derived from a Query
// ===========================================================================

/// Opaque, hash-stable cache key derived from a [`Query`].
///
/// The key is a 32-byte blake3 digest over the canonical serialisation of
/// `(resource.subject, selection, arguments)`. Per `specs/query.toml
/// [api.CacheKey.derivation]`, the projection is **not** part of the key
/// — the same selection may be projected differently by different
/// callers without splitting the cache.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey {
    /// 32-byte blake3 digest of the canonical key material.
    hash: [u8; 32],
}

impl CacheKey {
    /// Construct a `CacheKey` from a raw 32-byte digest.
    ///
    /// This is intended for use by [`CacheKeyer`] implementations. Most
    /// callers should obtain a key via [`CacheKeyer::key`].
    #[must_use]
    pub fn from_hash(hash: [u8; 32]) -> Self {
        Self { hash }
    }

    /// Construct a `CacheKey` by hashing the given input with BLAKE3.
    ///
    /// Convenience method for cache tier implementations that need to
    /// derive a key from raw bytes without going through a
    /// [`CacheKeyer`].
    #[must_use]
    pub fn hash_of(input: &[u8]) -> Self {
        Self {
            hash: blake3::hash(input).into(),
        }
    }

    /// The raw 32-byte digest.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.hash
    }
}

impl fmt::Display for CacheKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut hex = [0u8; 64];
        for (i, byte) in self.hash.iter().enumerate() {
            hex[i * 2] = HEX[usize::from(byte >> 4)];
            hex[i * 2 + 1] = HEX[usize::from(byte & 0x0f)];
        }
        f.write_str(std::str::from_utf8(&hex).expect("hex is valid utf-8"))
    }
}

// ===========================================================================
// CacheKeyer — trait + DefaultCacheKeyer implementation
// ===========================================================================

/// Derives a deterministic, hash-stable [`CacheKey`] from a [`Query`].
///
/// Used by `themql-cache` (L1–L4) so all tiers share the same key
/// derivation. Implementations must be deterministic: the same [`Query`]
/// (by value of `resource.subject`, `selection`, and `arguments`) must
/// always produce the same [`CacheKey`].
///
/// Per `specs/query.toml [api.CacheKey.derivation]`, the key material is:
///
/// - `resource.subject` (canonical dot-joined string),
/// - `selection` (serialised deterministically),
/// - `arguments` (if present, serialised deterministically — `serde_json`
///   canonical form).
///
/// The projection is deliberately **not** part of the key.
pub trait CacheKeyer: Send + Sync {
    /// Derive the cache key for `query`.
    ///
    /// Must be deterministic and hash-stable for equal key material.
    fn key(&self, query: &Query) -> CacheKey;
}

/// Default [`CacheKeyer`] using blake3 over the canonical JSON of
/// `(subject, selection, arguments)`.
///
/// Serialisation is via `serde_json::to_string`, which is deterministic
/// for equal `serde_json::Value` trees (object keys are held in a
/// `BTreeMap` where relevant, and enum variants serialise to a stable
/// tag). The projection is intentionally excluded from the key material
/// per the spec.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultCacheKeyer;

impl DefaultCacheKeyer {
    /// Construct a `DefaultCacheKeyer`.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl CacheKeyer for DefaultCacheKeyer {
    fn key(&self, query: &Query) -> CacheKey {
        let subject = query.resource.subject.as_str();
        let selection = serde_json::to_string(&query.selection).unwrap_or_default();
        let arguments = query
            .arguments
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default())
            .unwrap_or_default();

        let mut hasher = blake3::Hasher::new();
        hasher.update(subject.as_bytes());
        hasher.update(&[0]);
        hasher.update(selection.as_bytes());
        hasher.update(&[0]);
        hasher.update(arguments.as_bytes());

        let hash: [u8; 32] = hasher.finalize().into();
        CacheKey::from_hash(hash)
    }
}

// ===========================================================================
// QueryError — themql-query-owned error (internal to QueryExecutor impls)
// ===========================================================================

/// Errors raised by `themql-query` orchestration. Per `specs/query.toml
/// [api.QueryError]`, a `QueryExecutor` always returns
/// [`themql_core::Error`] to callers; `QueryError` is the internal
/// representation that maps onto the canonical error.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum QueryError {
    /// The deadline in [`Context`] expired before dispatch could
    /// complete. Maps to [`ErrorCode::Timeout`].
    #[error("deadline expired")]
    DeadlineExpired,
    /// The [`Context`] cancellation signal was set before dispatch could
    /// complete. Maps to [`ErrorCode::Timeout`].
    #[error("cancelled")]
    Cancelled,
    /// A cache tier lookup failed unexpectedly (not a miss — a fault).
    /// Maps to [`ErrorCode::InternalError`].
    #[error("cache lookup failed")]
    CacheLookupFailed,
    /// Batch dispatch failed. Maps to [`ErrorCode::InternalError`].
    #[error("batch dispatch failed")]
    BatchDispatchFailed,
    /// The underlying [`themql_core::Resolver`] returned an error.
    /// Propagated as-is.
    #[error("resolver returned: {0}")]
    ResolverReturned(#[from] Error),
}

impl From<QueryError> for Error {
    fn from(e: QueryError) -> Self {
        match e {
            QueryError::DeadlineExpired | QueryError::Cancelled => Error::timeout(e.to_string()),
            QueryError::CacheLookupFailed | QueryError::BatchDispatchFailed => {
                Error::internal_error(e.to_string())
            }
            QueryError::ResolverReturned(err) => err,
        }
    }
}

// ===========================================================================
// Batcher — groups concurrent queries for batch dispatch
// ===========================================================================

/// Groups concurrent queries to the same resource for batch dispatch,
/// when the underlying [`themql_core::Resolver`] supports batching.
///
/// Per `specs/query.toml [api.Batcher]`, batching is optional: if a
/// `QueryExecutor` is constructed without a `Batcher`, queries are
/// dispatched one-at-a-time.
#[allow(async_fn_in_trait)]
pub trait Batcher: Send + Sync {
    /// Dispatch `queries` as a batch under the shared [`Context`].
    ///
    /// The returned `Vec` has the same length and order as `queries`.
    ///
    /// # Errors
    /// Each element is `Result<Response, Error>`; a single query in the
    /// batch may fail without failing the whole batch.
    fn batch(
        &self,
        queries: Vec<Query>,
        ctx: &Context,
    ) -> impl Future<Output = Vec<Result<Response, Error>>>;
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::{Comparison, FieldPath, Predicate, Resource, Selection};

    fn query_with_subject(subject: &str) -> Query {
        Query::new(Resource::from_str(subject).expect("valid subject"))
    }

    fn query_filter(subject: &str, path: &str, value: serde_json::Value) -> Query {
        Query::new(Resource::from_str(subject).expect("valid subject")).with_selection(
            Selection::filter(Predicate::Field {
                path: FieldPath::from_str(path).expect("valid path"),
                op: Comparison::Eq,
                value,
            }),
        )
    }

    #[test]
    fn cache_key_is_deterministic_for_same_query() {
        let keyer = DefaultCacheKeyer::new();
        let q = query_with_subject("vehicle.sensors.imu.gyro");
        let k1 = keyer.key(&q);
        let k2 = keyer.key(&q);
        assert_eq!(k1, k2, "same query must produce same cache key");
    }

    #[test]
    fn cache_key_differs_when_selection_differs() {
        let keyer = DefaultCacheKeyer::new();
        let all = query_with_subject("vehicle.sensors.imu.gyro");
        let filtered = query_filter(
            "vehicle.sensors.imu.gyro",
            "temperature",
            serde_json::json!(42),
        );
        let k_all = keyer.key(&all);
        let k_filtered = keyer.key(&filtered);
        assert_ne!(
            k_all, k_filtered,
            "different selections must produce different cache keys"
        );
    }

    #[test]
    fn cache_key_differs_when_arguments_differ() {
        let keyer = DefaultCacheKeyer::new();
        let q1 = Query::new(Resource::from_str("vehicle.sensors.imu.gyro").expect("valid"))
            .with_arguments(serde_json::json!({"limit": 10}));
        let q2 = Query::new(Resource::from_str("vehicle.sensors.imu.gyro").expect("valid"))
            .with_arguments(serde_json::json!({"limit": 100}));
        let k1 = keyer.key(&q1);
        let k2 = keyer.key(&q2);
        assert_ne!(
            k1, k2,
            "different arguments must produce different cache keys"
        );
    }

    #[test]
    fn cache_key_is_stable_across_keyer_instances() {
        let k1 = DefaultCacheKeyer::new();
        let k2 = DefaultCacheKeyer::new();
        let q = query_with_subject("vehicle.sensors.imu.gyro");
        assert_eq!(
            k1.key(&q),
            k2.key(&q),
            "different keyer instances must agree"
        );
    }

    #[test]
    fn cache_key_excludes_projection() {
        let keyer = DefaultCacheKeyer::new();
        let base = Query::new(Resource::from_str("vehicle.sensors.imu.gyro").expect("valid"));
        let with_proj = Query::new(Resource::from_str("vehicle.sensors.imu.gyro").expect("valid"))
            .with_projection(
                themql_core::Projection::new()
                    .include(FieldPath::from_str("position.x").expect("valid")),
            );
        assert_eq!(
            keyer.key(&base),
            keyer.key(&with_proj),
            "projection must not be part of the cache key"
        );
    }

    #[test]
    fn cache_key_display_is_hex() {
        let keyer = DefaultCacheKeyer::new();
        let key = keyer.key(&query_with_subject("vehicle.sensors.imu.gyro"));
        let s = key.to_string();
        assert_eq!(s.len(), 64, "blake3 hex must be 64 chars");
        assert!(s.chars().all(|c| c.is_ascii_hexdigit()), "must be hex");
    }

    #[test]
    fn query_error_deadline_maps_to_timeout() {
        let e: Error = QueryError::DeadlineExpired.into();
        assert_eq!(e.code, ErrorCode::Timeout);
    }

    #[test]
    fn query_error_cancelled_maps_to_timeout() {
        let e: Error = QueryError::Cancelled.into();
        assert_eq!(e.code, ErrorCode::Timeout);
    }

    #[test]
    fn query_error_cache_lookup_maps_to_internal() {
        let e: Error = QueryError::CacheLookupFailed.into();
        assert_eq!(e.code, ErrorCode::InternalError);
    }

    #[test]
    fn query_error_batch_maps_to_internal() {
        let e: Error = QueryError::BatchDispatchFailed.into();
        assert_eq!(e.code, ErrorCode::InternalError);
    }

    #[test]
    fn query_error_resolver_returned_propagates_inner() {
        let inner = Error::resolver_error("boom");
        let e: Error = QueryError::ResolverReturned(inner.clone()).into();
        assert_eq!(e.code, ErrorCode::ResolverError);
        assert_eq!(e.message, "boom");
    }
}
