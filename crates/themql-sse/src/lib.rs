//! # themql-sse
//!
//! Server-Sent Events stream projection of theMQL's message model. SSE
//! is a one-way server-to-client stream used for telemetry, events, and
//! query subscription results. SSE-specific types must not escape this
//! crate.
//!
//! See `specs/sse.toml` for the authoritative specification. This v0.1
//! crate declares only the public traits and supporting types; concrete
//! tokio-backed implementations are added in a later phase.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(async_fn_in_trait)]

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};
use themql_core::{Error, Message, Subject};
use thiserror::Error;

// ===========================================================================
// SseEvent — one SSE frame
// ===========================================================================

/// One Server-Sent Event frame. Carries a `themql-core` `Message`
/// payload as JSON.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SseEvent {
    /// SSE `id` field; carries `Message.id` for Last-Event-ID replay.
    pub id: Option<String>,
    /// SSE `event` field; carries `Operation` as a lowercase string.
    pub event: Option<String>,
    /// SSE `data` field; JSON-encoded `Message` payload.
    pub data: String,
    /// SSE `retry` field; milliseconds before the client reconnects.
    pub retry: Option<u32>,
}

impl SseEvent {
    /// Construct a minimal `SseEvent` with only the `data` field set.
    #[must_use]
    pub fn from_data(data: impl Into<String>) -> Self {
        Self {
            id: None,
            event: None,
            data: data.into(),
            retry: None,
        }
    }

    /// Set the `id` field.
    #[must_use]
    pub fn with_id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Set the `event` field.
    #[must_use]
    pub fn with_event(mut self, event: impl Into<String>) -> Self {
        self.event = Some(event.into());
        self
    }

    /// Set the `retry` field.
    #[must_use]
    pub fn with_retry(mut self, retry: u32) -> Self {
        self.retry = Some(retry);
        self
    }
}

// ===========================================================================
// SseStream — live stream of SSE events
// ===========================================================================

/// A live SSE stream from a subject. Implementations are produced by
/// [`SsePublisher::add_subscriber`].
pub trait SseStream: Send {
    /// Pull the next event from the stream.
    ///
    /// Returns `Ok(None)` when the stream has terminated cleanly. Errors
    /// indicate a transport-level failure; the stream must be considered
    /// closed afterwards.
    ///
    /// # Errors
    /// Returns [`SseError`] if the stream fails.
    fn next_event(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SseEvent>, SseError>> + Send + '_>>;

    /// The subject this stream is subscribed to.
    fn subject(&self) -> &Subject;
}

// ===========================================================================
// SsePublisher — server-side push to connected SSE clients
// ===========================================================================

/// Server-side publisher that pushes `Message`s to connected SSE
/// clients.
pub trait SsePublisher: Send + Sync {
    /// Broadcast a `Message` to all clients subscribed to `subject`.
    ///
    /// # Errors
    /// Returns [`SseError`] if the broadcast fails.
    fn broadcast(
        &self,
        subject: &Subject,
        msg: &Message,
    ) -> impl Future<Output = Result<(), SseError>>;

    /// Add a new subscriber for `subject`, returning the resulting
    /// stream.
    ///
    /// # Errors
    /// Returns [`SseError`] if a new stream cannot be established.
    fn add_subscriber(
        &self,
        subject: &Subject,
    ) -> impl Future<Output = Result<Box<dyn SseStream>, SseError>>;
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by the SSE transport adapter.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum SseError {
    /// The connection was closed cleanly.
    #[error("sse connection closed")]
    ConnectionClosed,
    /// A message could not be serialised for the wire.
    #[error("sse serialization error: {0}")]
    SerializationError(String),
    /// An operation timed out.
    #[error("sse timeout")]
    Timeout,
    /// A client disconnected unexpectedly.
    #[error("sse client disconnected")]
    ClientDisconnected,
    /// An internal failure occurred.
    #[error("sse internal error: {0}")]
    InternalError(String),
}

impl From<SseError> for Error {
    fn from(e: SseError) -> Self {
        Error::transport_error(e.to_string())
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::ErrorCode;

    #[test]
    fn sse_event_from_data_sets_only_data() {
        let ev = SseEvent::from_data("{}");
        assert_eq!(ev.data, "{}");
        assert_eq!(ev.id, None);
        assert_eq!(ev.event, None);
        assert_eq!(ev.retry, None);
    }

    #[test]
    fn sse_event_builder_chains() {
        let ev = SseEvent::from_data("payload")
            .with_id("42")
            .with_event("telemetry")
            .with_retry(5000);
        assert_eq!(ev.id.as_deref(), Some("42"));
        assert_eq!(ev.event.as_deref(), Some("telemetry"));
        assert_eq!(ev.data, "payload");
        assert_eq!(ev.retry, Some(5000));
    }

    #[test]
    fn sse_event_round_trips_json() {
        let ev = SseEvent {
            id: Some("abc".to_owned()),
            event: Some("response".to_owned()),
            data: "{\"k\":1}".to_owned(),
            retry: Some(3000),
        };
        let json = serde_json::to_string(&ev).expect("serialize");
        let back: SseEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(ev, back);
    }

    #[test]
    fn sse_error_connection_closed_maps_to_transport_error() {
        let e: Error = SseError::ConnectionClosed.into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn sse_error_serialization_maps_to_transport_error() {
        let e: Error = SseError::SerializationError("bad json".to_owned()).into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn sse_error_timeout_maps_to_transport_error() {
        let e: Error = SseError::Timeout.into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn sse_error_client_disconnected_maps_to_transport_error() {
        let e: Error = SseError::ClientDisconnected.into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn sse_error_internal_maps_to_transport_error() {
        let e: Error = SseError::InternalError("boom".to_owned()).into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }
}
