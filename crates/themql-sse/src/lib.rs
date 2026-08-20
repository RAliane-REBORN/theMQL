//! # themql-sse
//!
//! Server-Sent Events stream projection of theMQL's message model. SSE
//! is a one-way server-to-client stream used for telemetry, events, and
//! query subscription results. SSE-specific types must not escape this
//! crate.
//!
//! See `specs/sse.toml` for the authoritative specification. This crate
//! provides the public traits, supporting types, and a tokio-backed
//! concrete implementation (`TokioSsePublisher` / `TokioSseStream`)
//! plus an `axum`-based HTTP server (`serve_sse`).

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![deny(warnings)]
#![allow(clippy::module_name_repetitions)]
#![allow(async_fn_in_trait)]

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use themql_core::{Error, Message, Subject};
use thiserror::Error;
use tokio::sync::broadcast;

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
// SubjectState — per-subject channel + bounded replay log
// ===========================================================================

/// Maximum number of events retained per subject for Last-Event-ID
/// replay. Older events are evicted when the bound is exceeded.
pub const REPLAY_LOG_CAPACITY: usize = 256;

/// Per-subject state shared between the publisher and all subscribers:
/// a tokio `broadcast` channel for live events plus a bounded event log
/// used for Last-Event-ID replay.
struct SubjectState {
    tx: broadcast::Sender<Arc<SseEvent>>,
    log: VecDeque<Arc<SseEvent>>,
}

impl SubjectState {
    fn new() -> Self {
        let (tx, _rx) = broadcast::channel(256);
        Self {
            tx,
            log: VecDeque::with_capacity(REPLAY_LOG_CAPACITY),
        }
    }

    fn push(&mut self, event: Arc<SseEvent>) {
        if self.log.len() >= REPLAY_LOG_CAPACITY {
            self.log.pop_front();
        }
        self.log.push_back(event);
    }

    /// Returns events whose `id` is strictly greater than `last_id`,
    /// preserving publication order. If `last_id` is not found in the
    /// log, the entire log is returned (best-effort replay).
    fn replay_after(&self, last_id: &str) -> Vec<Arc<SseEvent>> {
        let pos = self
            .log
            .iter()
            .position(|e| e.id.as_deref() == Some(last_id));
        let start = pos.map_or(0, |p| p + 1);
        self.log.iter().skip(start).cloned().collect()
    }
}

// ===========================================================================
// TokioSsePublisher — tokio broadcast-backed SSE publisher
// ===========================================================================

/// Tokio-backed [`SsePublisher`] using `tokio::sync::broadcast` channels.
///
/// Each subject gets its own broadcast channel and bounded event log.
/// Subscribers receive a [`TokioSseStream`] wrapping a
/// `broadcast::Receiver`. The event log supports Last-Event-ID replay
/// via [`TokioSsePublisher::replay_after`].
pub struct TokioSsePublisher {
    subjects: Mutex<HashMap<String, Arc<Mutex<SubjectState>>>>,
}

impl TokioSsePublisher {
    /// Construct a new publisher with no subscribers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            subjects: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the [`SubjectState`] handle for `subject`, creating it
    /// on first use.
    fn state_for(&self, subject: &Subject) -> Arc<Mutex<SubjectState>> {
        let key = subject.as_str();
        let mut subjects = self
            .subjects
            .lock()
            .expect("sse publisher subjects mutex poisoned");
        subjects
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(SubjectState::new())))
            .clone()
    }

    /// Returns the events published to `subject` whose `id` is strictly
    /// greater than `last_id`, in publication order. If `last_id` is not
    /// present in the log, the full retained log is returned
    /// (best-effort replay per `specs/sse.toml`).
    ///
    /// # Errors
    /// Returns [`SseError::InternalError`] if the per-subject lock is
    /// poisoned.
    pub fn replay_after(
        &self,
        subject: &Subject,
        last_id: &str,
    ) -> Result<Vec<Arc<SseEvent>>, SseError> {
        let state = self.state_for(subject);
        let guard = state
            .lock()
            .map_err(|_| SseError::InternalError("subject state poisoned".to_owned()))?;
        Ok(guard.replay_after(last_id))
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
        let state = self.state_for(subject);
        let event_result = SseEvent::from_message(msg);
        async move {
            let event = event_result?;
            let event = Arc::new(event);
            {
                let mut guard = state
                    .lock()
                    .map_err(|_| SseError::InternalError("subject state poisoned".to_owned()))?;
                guard.push(Arc::clone(&event));
            }
            let tx = {
                let guard = state
                    .lock()
                    .map_err(|_| SseError::InternalError("subject state poisoned".to_owned()))?;
                guard.tx.clone()
            };
            if tx.receiver_count() > 0 {
                let _ = tx.send(event);
            }
            Ok(())
        }
    }

    fn add_subscriber(
        &self,
        subject: &Subject,
    ) -> impl Future<Output = Result<Box<dyn SseStream>, SseError>> {
        let state = self.state_for(subject);
        let subject = subject.clone();
        async move {
            let rx = {
                let guard = state
                    .lock()
                    .map_err(|_| SseError::InternalError("subject state poisoned".to_owned()))?;
                guard.tx.subscribe()
            };
            Ok(Box::new(TokioSseStream { rx, subject }) as Box<dyn SseStream>)
        }
    }
}

// ===========================================================================
// TokioSseStream — broadcast receiver wrapped as SseStream
// ===========================================================================

/// Tokio-backed [`SseStream`] wrapping a `broadcast::Receiver`.
pub struct TokioSseStream {
    rx: broadcast::Receiver<Arc<SseEvent>>,
    subject: Subject,
}

impl SseStream for TokioSseStream {
    fn next_event(
        &mut self,
    ) -> Pin<Box<dyn Future<Output = Result<Option<SseEvent>, SseError>> + Send + '_>> {
        Box::pin(async move {
            match self.rx.recv().await {
                Ok(event) => Ok(Some((*event).clone())),
                Err(broadcast::error::RecvError::Closed) => Ok(None),
                Err(broadcast::error::RecvError::Lagged(skipped)) => Ok(Some(SseEvent::from_data(
                    format!("{{\"lagged\":{skipped}}}"),
                ))),
            }
        })
    }

    fn subject(&self) -> &Subject {
        &self.subject
    }
}

// ===========================================================================
// axum HTTP server — GET /events -> text/event-stream
// ===========================================================================

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event as AxumEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use futures_util::stream::{self, Stream};
use std::convert::Infallible;
use std::task::{Context, Poll};

/// Shared application state for the axum router: the publisher and the
/// subject the `/events` endpoint is bound to.
#[derive(Clone)]
pub struct SseAppState {
    publisher: Arc<TokioSsePublisher>,
    subject: Subject,
}

/// Builds an `axum::Router` exposing a `GET /events` route that streams
/// events from `publisher` for `subject` as `text/event-stream`.
///
/// The route honours the `Last-Event-ID` request header: when present,
/// the server first replays all retained events whose `id` is greater
/// than the header value, then continues with the live stream.
pub fn serve_sse(publisher: TokioSsePublisher, subject: Subject) -> Router {
    let state = SseAppState {
        publisher: Arc::new(publisher),
        subject,
    };
    Router::new()
        .route("/events", get(events_handler))
        .with_state(state)
}

/// Returns the event stream for the configured subject.
///
/// On `Last-Event-ID` presence, retained events with a greater `id`
/// are replayed first (best-effort), followed by the live stream.
async fn events_handler(State(state): State<SseAppState>, headers: HeaderMap) -> Response {
    let last_event_id = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);

    let replay = match &last_event_id {
        Some(id) => state
            .publisher
            .replay_after(&state.subject, id)
            .unwrap_or_default(),
        None => Vec::new(),
    };

    let stream = match state.publisher.add_subscriber(&state.subject).await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("sse subscribe failed: {e}"),
            )
                .into_response();
        }
    };

    let replay_iter = replay.into_iter();
    let live_stream: Pin<Box<dyn Stream<Item = Result<AxumEvent, Infallible>> + Send>> =
        Box::pin(stream::unfold(stream, |mut stream| async move {
            match stream.next_event().await {
                Ok(Some(ev)) => Some((Ok(to_axum_event(&ev)), stream)),
                Ok(None) | Err(_) => None,
            }
        }));
    let body = EventBody {
        replay: replay_iter,
        live: live_stream,
    };
    Sse::new(body)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Convert an [`SseEvent`] to an `axum::response::sse::Event`.
fn to_axum_event(ev: &SseEvent) -> AxumEvent {
    let mut builder = AxumEvent::default();
    if let Some(id) = &ev.id {
        builder = builder.id(id);
    }
    if let Some(event) = &ev.event {
        builder = builder.event(event);
    }
    if !ev.data.is_empty() {
        builder = builder.data(ev.data.clone());
    }
    if let Some(retry) = ev.retry {
        builder = builder.retry(std::time::Duration::from_millis(u64::from(retry)));
    }
    builder
}

/// `Stream` that yields replayed events then live events as
/// `Result<AxumEvent, Infallible>`.
struct EventBody {
    replay: std::vec::IntoIter<Arc<SseEvent>>,
    live: Pin<Box<dyn Stream<Item = Result<AxumEvent, Infallible>> + Send>>,
}

impl Stream for EventBody {
    type Item = Result<AxumEvent, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(ev) = self.replay.next() {
            return Poll::Ready(Some(Ok(to_axum_event(&ev))));
        }
        self.live.as_mut().poll_next(cx)
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

    #[tokio::test]
    async fn tokio_sse_publisher_broadcast_delivers_real_event() {
        let publisher = TokioSsePublisher::new();
        let subject = Subject::from_str("vehicle.state").expect("valid subject");
        let msg =
            Message::new("vehicle.state", themql_core::Operation::Event).expect("valid subject");
        let expected = SseEvent::from_message(&msg).expect("serialize");
        let mut stream = publisher.add_subscriber(&subject).await.expect("subscribe");
        publisher
            .broadcast(&subject, &msg)
            .await
            .expect("broadcast");
        let received = stream
            .next_event()
            .await
            .expect("receive")
            .expect("event present");
        assert_eq!(received.id, expected.id);
        assert_eq!(received.event, expected.event);
        assert_eq!(received.data, expected.data);
    }

    #[tokio::test]
    async fn tokio_sse_publisher_multi_subscriber_broadcast() {
        let publisher = TokioSsePublisher::new();
        let subject = Subject::from_str("vehicle.state").expect("valid subject");
        let msg = Message::new("vehicle.state", themql_core::Operation::Telemetry)
            .expect("valid subject");
        let mut stream_a = publisher
            .add_subscriber(&subject)
            .await
            .expect("subscribe a");
        let mut stream_b = publisher
            .add_subscriber(&subject)
            .await
            .expect("subscribe b");
        publisher
            .broadcast(&subject, &msg)
            .await
            .expect("broadcast");
        let ev_a = stream_a.next_event().await.expect("recv a").expect("ev a");
        let ev_b = stream_b.next_event().await.expect("recv b").expect("ev b");
        assert_eq!(ev_a.id, ev_b.id, "both subscribers get same event id");
        assert_eq!(ev_a.data, ev_b.data, "both subscribers get same data");
    }

    #[tokio::test]
    async fn tokio_sse_publisher_last_event_id_replay() {
        let publisher = TokioSsePublisher::new();
        let subject = Subject::from_str("vehicle.state").expect("valid subject");
        let msg_a =
            Message::new("vehicle.state", themql_core::Operation::Event).expect("valid subject");
        let msg_b = Message::new("vehicle.state", themql_core::Operation::Telemetry)
            .expect("valid subject");
        publisher
            .broadcast(&subject, &msg_a)
            .await
            .expect("broadcast a");
        publisher
            .broadcast(&subject, &msg_b)
            .await
            .expect("broadcast b");
        let id_a = SseEvent::from_message(&msg_a)
            .expect("serialize a")
            .id
            .expect("id a");
        let replay = publisher.replay_after(&subject, &id_a).expect("replay");
        assert_eq!(replay.len(), 1, "exactly one event after id_a");
        let replayed = replay.first().expect("one event");
        assert_eq!(replayed.event.as_deref(), Some("telemetry"));
    }

    #[tokio::test]
    async fn tokio_sse_publisher_replay_after_unknown_id_returns_full_log() {
        let publisher = TokioSsePublisher::new();
        let subject = Subject::from_str("vehicle.state").expect("valid subject");
        let msg_a =
            Message::new("vehicle.state", themql_core::Operation::Event).expect("valid subject");
        publisher
            .broadcast(&subject, &msg_a)
            .await
            .expect("broadcast");
        let replay = publisher
            .replay_after(&subject, "does-not-exist")
            .expect("replay");
        assert_eq!(replay.len(), 1, "unknown id -> full log replayed");
    }
}
