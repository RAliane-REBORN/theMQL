//! # themql-transport
//!
//! Cross-cutting transport specification for theMQL. Defines the
//! [`Bridge`] trait, the [`BridgeRoute`] routing type, and the
//! [`TransportError`] enum. Per-transport detail (MQTT, GraphQL, SSE)
//! lives in the subordinate adapter crates; this crate defines only the
//! cross-cutting contract that all transports must obey.
//!
//! Per `specs/transport.toml`, transports are **projections** of the
//! core `Message` model — they hold no business logic and define no
//! independent semantic systems. Transport-specific types must not
//! escape their adapter crate.
//!
//! See `specs/transport.toml` for the authoritative specification.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::future::Future;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use themql_core::{Error as CoreError, Message};
pub use themql_core::{Subject, SubjectPattern};

// Re-export themql-message types so consumers of the bridge can use the
// message routing layer without a direct dep on themql-message.
pub use themql_message::Serializer;

/// Identifies one of the three transports in theMQL.
///
/// Used by [`BridgeRoute`] to declare static routes between transports
/// and by [`Bridge::route`] to record the source and target transports
/// of a bridged message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    /// MQTT — pub/sub over topics.
    Mqtt,
    /// GraphQL — request/response + subscriptions.
    Graphql,
    /// SSE — server-sent events stream.
    Sse,
}

/// How a bridge rewrites the subject of a bridged message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubjectRewrite {
    /// Prepend a prefix to the original subject.
    Prefix(Subject),
    /// Replace the original subject entirely.
    Replace(Subject),
}

/// A static route: messages on `source_pattern` are forwarded from
/// `source` transport to `target` transport, with an optional subject
/// rewrite and optional explicit target subject.
///
/// `target_subject = None` means 'preserve the original subject'.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BridgeRoute {
    /// The transport the message originated on.
    pub source: TransportKind,
    /// The transport the message is being forwarded to.
    pub target: TransportKind,
    /// Pattern matching source subjects eligible for this route.
    pub source_pattern: SubjectPattern,
    /// Explicit target subject; `None` preserves the original subject.
    pub target_subject: Option<Subject>,
    /// Optional subject rewrite applied during forwarding.
    pub rewrite: Option<SubjectRewrite>,
}

/// Errors raised by transport bridging.
///
/// Maps to [`themql_core::Error`] via the [`From<TransportError>`]
/// implementation. Per `specs/transport.toml`, all `TransportError`
/// variants map to [`themql_core::Error::transport_error`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    /// The source transport has been closed; no more messages arrive.
    #[error("source transport closed")]
    SourceClosed,
    /// Publishing to a target transport failed.
    #[error("target publish failed: {0}")]
    TargetPublishFailed(String),
    /// A subject rewrite could not be applied (e.g. prefix would exceed
    /// subject grammar).
    #[error("subject rewrite failed: {0}")]
    SubjectRewriteFailed(String),
    /// Serialisation of the message for the target transport failed.
    #[error("serialization error: {0}")]
    SerializationError(String),
    /// A bridge attempted to route a message back to the transport it
    /// arrived on. Loopback is forbidden by `specs/transport.toml`.
    #[error("loopback forbidden: a bridge may not route back to the source transport")]
    LoopbackForbidden,
}

impl From<TransportError> for CoreError {
    fn from(e: TransportError) -> Self {
        Self::transport_error(e.to_string())
    }
}

/// A bridge re-publishes a `Message` received on one transport onto
/// another transport. Bridges hold no business logic; they translate
/// `Message` → transport-specific publish call. Routing is by
/// [`Subject`], never by adapter-specific types.
///
/// Async via `impl Future` return types (matching the
/// `themql_core::Resolver` pattern).
#[allow(async_fn_in_trait)]
pub trait Bridge: Send + Sync {
    /// Forward `msg` (which arrived on `source`) to each transport in
    /// `targets`. Implementations must reject loopback (a target equal
    /// to `source`) with [`TransportError::LoopbackForbidden`].
    ///
    /// # Errors
    /// Returns [`TransportError`] if any target publish fails, if
    /// loopback is attempted, or if serialisation fails.
    fn route(
        &self,
        msg: &Message,
        source: TransportKind,
        targets: &[TransportKind],
    ) -> impl Future<Output = Result<(), TransportError>>;
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::Subject;

    #[test]
    fn transport_kind_serialises_snake_case() {
        let json = serde_json::to_string(&TransportKind::Mqtt).unwrap();
        assert_eq!(json, "\"mqtt\"");
        let json = serde_json::to_string(&TransportKind::Graphql).unwrap();
        assert_eq!(json, "\"graphql\"");
        let json = serde_json::to_string(&TransportKind::Sse).unwrap();
        assert_eq!(json, "\"sse\"");
    }

    #[test]
    fn transport_kind_round_trips() {
        for kind in [
            TransportKind::Mqtt,
            TransportKind::Graphql,
            TransportKind::Sse,
        ] {
            let json = serde_json::to_string(&kind).unwrap();
            let back: TransportKind = serde_json::from_str(&json).unwrap();
            assert_eq!(back, kind);
        }
    }

    #[test]
    fn transport_kind_eq_and_hash() {
        assert_eq!(TransportKind::Mqtt, TransportKind::Mqtt);
        assert_ne!(TransportKind::Mqtt, TransportKind::Sse);
        let mut set = std::collections::HashSet::new();
        set.insert(TransportKind::Mqtt);
        set.insert(TransportKind::Mqtt);
        assert_eq!(set.len(), 1, "duplicates collapse");
    }

    #[test]
    fn bridge_route_constructs_with_defaults() {
        let route = BridgeRoute {
            source: TransportKind::Mqtt,
            target: TransportKind::Graphql,
            source_pattern: SubjectPattern::from_str("vehicle.#").unwrap(),
            target_subject: None,
            rewrite: None,
        };
        assert_eq!(route.source, TransportKind::Mqtt);
        assert_eq!(route.target, TransportKind::Graphql);
        assert!(route.target_subject.is_none());
        assert!(route.rewrite.is_none());
    }

    #[test]
    fn bridge_route_with_rewrite_prefix() {
        let route = BridgeRoute {
            source: TransportKind::Mqtt,
            target: TransportKind::Sse,
            source_pattern: SubjectPattern::from_str("vehicle.sensors.#").unwrap(),
            target_subject: None,
            rewrite: Some(SubjectRewrite::Prefix(Subject::from_str("bridge").unwrap())),
        };
        assert!(matches!(route.rewrite, Some(SubjectRewrite::Prefix(_))));
    }

    #[test]
    fn bridge_route_with_rewrite_replace() {
        let route = BridgeRoute {
            source: TransportKind::Graphql,
            target: TransportKind::Mqtt,
            source_pattern: SubjectPattern::from_str("query.#").unwrap(),
            target_subject: Some(Subject::from_str("bridge.query").unwrap()),
            rewrite: Some(SubjectRewrite::Replace(
                Subject::from_str("mqtt.forwarded").unwrap(),
            )),
        };
        assert!(matches!(route.rewrite, Some(SubjectRewrite::Replace(_))));
        assert_eq!(
            route.target_subject.as_ref().unwrap().as_str(),
            "bridge.query"
        );
    }

    #[test]
    fn bridge_route_round_trips_json() {
        let route = BridgeRoute {
            source: TransportKind::Mqtt,
            target: TransportKind::Sse,
            source_pattern: SubjectPattern::from_str("vehicle.#").unwrap(),
            target_subject: Some(Subject::from_str("sse.vehicle").unwrap()),
            rewrite: Some(SubjectRewrite::Prefix(Subject::from_str("bridge").unwrap())),
        };
        let json = serde_json::to_string(&route).unwrap();
        let back: BridgeRoute = serde_json::from_str(&json).unwrap();
        assert_eq!(back, route);
    }

    #[test]
    fn subject_rewrite_serialises() {
        let r = SubjectRewrite::Prefix(Subject::from_str("bridge").unwrap());
        let json = serde_json::to_string(&r).unwrap();
        let back: SubjectRewrite = serde_json::from_str(&json).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn transport_error_source_closed_constructs() {
        let e = TransportError::SourceClosed;
        assert_eq!(e.to_string(), "source transport closed");
    }

    #[test]
    fn transport_error_target_publish_failed_carries_message() {
        let e = TransportError::TargetPublishFailed("connection reset".to_owned());
        assert!(e.to_string().contains("connection reset"));
    }

    #[test]
    fn transport_error_loopback_forbidden_constructs() {
        let e = TransportError::LoopbackForbidden;
        assert!(e.to_string().contains("loopback"));
    }

    #[test]
    fn transport_error_converts_to_core_error_as_transport() {
        let e = TransportError::SourceClosed;
        let core: CoreError = e.into();
        assert_eq!(core.code, themql_core::ErrorCode::TransportError);
    }

    #[test]
    fn transport_error_serialization_maps_to_transport_error() {
        let e = TransportError::SerializationError("bad bytes".to_owned());
        let core: CoreError = e.into();
        assert_eq!(core.code, themql_core::ErrorCode::TransportError);
    }

    #[test]
    fn transport_error_subject_rewrite_maps_to_transport_error() {
        let e = TransportError::SubjectRewriteFailed("prefix too long".to_owned());
        let core: CoreError = e.into();
        assert_eq!(core.code, themql_core::ErrorCode::TransportError);
    }
}
