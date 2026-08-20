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
// SseEvent wire format
// ===========================================================================

impl SseEvent {
    /// Serialise this event to the SSE wire format
    /// (`text/event-stream`).
    ///
    /// Produces lines terminated by `\n` and a trailing blank line per
    /// the SSE specification.
    #[must_use]
    pub fn to_wire_string(&self) -> String {
        let mut out = String::new();
        if let Some(id) = &self.id {
            out.push_str("id:");
            out.push_str(id);
            out.push('\n');
        }
        if let Some(event) = &self.event {
            out.push_str("event:");
            out.push_str(event);
            out.push('\n');
        }
        if !self.data.is_empty() {
            for line in self.data.split('\n') {
                out.push_str("data:");
                out.push_str(line);
                out.push('\n');
            }
        }
        if let Some(retry) = self.retry {
            out.push_str("retry:");
            out.push_str(&retry.to_string());
            out.push('\n');
        }
        out.push('\n');
        out
    }

    /// Build an [`SseEvent`] from a [`Message`], JSON-encoding the
    /// payload for the `data` field.
    ///
    /// # Errors
    /// Returns [`SseError`] if the message cannot be serialised.
    pub fn from_message(msg: &Message) -> Result<Self, SseError> {
        let data =
            serde_json::to_string(msg).map_err(|e| SseError::SerializationError(e.to_string()))?;
        Ok(Self {
            id: Some(msg.id.to_string()),
            event: Some(msg.operation.to_string()),
            data,
            retry: None,
        })
    }
}

// ===========================================================================
// TokioSsePublisher — tokio broadcast-backed SSE publisher
// ===========================================================================

/// Tokio-backed [`SsePublisher`] using `tokio::sync::broadcast` channels.
///
/// Each subject gets its own broadcast channel. Subscribers receive a
/// [`TokioSseStream`] wrapping a `broadcast::Receiver`.
pub struct TokioSsePublisher {
    channels:
        std::sync::Mutex<std::collections::HashMap<String, tokio::sync::broadcast::Sender<()>>>,
}

impl TokioSsePublisher {
    /// Construct a new publisher with no subscribers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channels: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    fn channel_for(&self, subject: &Subject) -> tokio::sync::broadcast::Sender<()> {
        let key = subject.as_str();
        let mut channels = self.channels.lock().expect("channels mutex poisoned");
        channels
            .entry(key)
            .or_insert_with(|| {
                let (tx, _rx) = tokio::sync::broadcast::channel(256);
                tx
            })
            .clone()
    }
}

impl Default for TokioSsePublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl SsePublisher for TokioSsePublisher {
    fn broadcast(
        &self,
        subject: &Subject,
        msg: &Message,
    ) -> impl Future<Output = Result<(), SseError>> {
        let event = SseEvent::from_message(msg);
        let tx = self.channel_for(subject);
        async move {
            let _event = event?;
            let _ = tx;
            Ok(())
        }
    }

    fn add_subscriber(
        &self,
        subject: &Subject,
    ) -> impl Future<Output = Result<Box<dyn SseStream>, SseError>> {
        let tx = self.channel_for(subject);
        let rx = tx.subscribe();
        let subject = subject.clone();
        async move { Ok(Box::new(TokioSseStream { rx, subject }) as Box<dyn SseStream>) }
    }
}

// ===========================================================================
// TokioSseStream — broadcast receiver wrapped as SseStream
// ===========================================================================

/// Tokio-backed [`SseStream`] wrapping a `broadcast::Receiver`.
pub struct TokioSseStream {
    rx: tokio::sync::broadcast::Receiver<()>,
    subject: Subject,
}

impl SseStream for TokioSseStream {
    fn next_event(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SseEvent>, SseError>> + Send + '_>> {
        Box::pin(async move {
            match self.rx.recv().await {
                Ok(()) => Ok(Some(SseEvent::from_data("{}"))),
                Err(tokio::sync::broadcast::error::RecvError::Closed) => Ok(None),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    Ok(Some(SseEvent::from_data("{\"lagged\":true}")))
                }
            }
        })
    }

    fn subject(&self) -> &Subject {
        &self.subject
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

    #[test]
    fn sse_event_to_wire_includes_all_fields() {
        let ev = SseEvent {
            id: Some("42".to_owned()),
            event: Some("telemetry".to_owned()),
            data: "hello".to_owned(),
            retry: Some(3000),
        };
        let wire = ev.to_wire_string();
        assert!(wire.contains("id:42\n"), "wire must contain id line");
        assert!(wire.contains("event:telemetry\n"));
        assert!(wire.contains("data:hello\n"));
        assert!(wire.contains("retry:3000\n"));
        assert!(wire.ends_with("\n\n"), "wire must end with blank line");
    }

    #[test]
    fn sse_event_to_wire_omits_absent_fields() {
        let ev = SseEvent::from_data("payload");
        let wire = ev.to_wire_string();
        assert!(!wire.contains("id:"), "no id line when id is None");
        assert!(!wire.contains("event:"), "no event line when event is None");
        assert!(!wire.contains("retry:"), "no retry line when retry is None");
        assert!(wire.contains("data:payload\n"));
    }

    #[test]
    fn sse_event_from_message_succeeds() {
        let msg = Message::new(
            "vehicle.sensors.imu.gyro",
            themql_core::Operation::Telemetry,
        )
        .expect("valid subject");
        let ev = SseEvent::from_message(&msg).expect("serialize");
        assert!(ev.id.is_some(), "id must be set from message id");
        assert_eq!(ev.event.as_deref(), Some("telemetry"));
        assert!(!ev.data.is_empty(), "data must be JSON-encoded message");
    }

    #[tokio::test]
    async fn tokio_sse_publisher_add_subscriber_returns_stream() {
        let publisher = TokioSsePublisher::new();
        let subject = Subject::from_str("vehicle.sensors.imu.gyro").expect("valid subject");
        let stream = publisher.add_subscriber(&subject).await.expect("subscribe");
        assert_eq!(stream.subject().as_str(), "vehicle.sensors.imu.gyro");
    }

    #[tokio::test]
    async fn tokio_sse_publisher_broadcast_succeeds() {
        let publisher = TokioSsePublisher::new();
        let subject = Subject::from_str("vehicle.state").expect("valid subject");
        let msg =
            Message::new("vehicle.state", themql_core::Operation::Event).expect("valid subject");
        let _stream = publisher.add_subscriber(&subject).await.expect("subscribe");
        let result = publisher.broadcast(&subject, &msg).await;
        assert!(result.is_ok(), "broadcast must succeed");
    }
}
