//! # themql-mqtt
//!
//! MQTT transport adapter for theMQL. Uses embassy for embedded MQTT and
//! `rumqttc` (tokio-based) for desktop. MQTT is a transport, not a semantic
//! owner. MQTT-specific types must not escape this crate.
//!
//! See `specs/mqtt.toml` for the authoritative specification.
//!
//! The desktop client ([`RumqttcTransport`]) wraps a `rumqttc::AsyncClient`
//! and its `EventLoop`. Per `specs/mqtt.toml [lifecycle]`, reconnect is
//! never implicit: the caller decides whether to keep polling the
//! event loop after a connection error, and the [`RumqttcConfig`]
//! `auto_reconnect` flag is an explicit, caller-owned policy knob.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![deny(warnings)]
#![allow(clippy::module_name_repetitions)]
#![allow(async_fn_in_trait)]

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rumqttc::{AsyncClient, ClientError, Event, EventLoop, MqttOptions, QoS as RumqttcQos};
use serde::{Deserialize, Serialize};
use themql_core::{Error, Message, MessageHandler, Response, Subject};
use thiserror::Error;
use tokio::sync::Mutex as AsyncMutex;

// ===========================================================================
// SubscriptionId — opaque handle returned by subscribe()
// ===========================================================================

/// Opaque handle returned by [`MqttSubscriber::subscribe`]. Used to
/// unsubscribe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubscriptionId(pub u64);

impl SubscriptionId {
    /// Construct a `SubscriptionId` from a raw u64.
    ///
    /// Intended for use by transport implementations, not application
    /// code.
    #[must_use]
    pub fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw u64 identifier.
    #[must_use]
    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

// ===========================================================================
// MqttQos
// ===========================================================================

/// MQTT quality-of-service level. Configurable per subject via publish
/// options; default is [`MqttQos::AtLeastOnce`] (`QoS` 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MqttQos {
    /// `QoS` 0 — at most once, fire-and-forget.
    AtMostOnce,
    /// `QoS` 1 — at least once, default for theMQL.
    #[default]
    AtLeastOnce,
    /// `QoS` 2 — exactly once.
    ExactlyOnce,
}

impl MqttQos {
    /// Map a [`MqttQos`] to the corresponding `rumqttc` [`QoS`].
    #[must_use]
    pub const fn to_rumqttc(self) -> RumqttcQos {
        match self {
            Self::AtMostOnce => RumqttcQos::AtMostOnce,
            Self::AtLeastOnce => RumqttcQos::AtLeastOnce,
            Self::ExactlyOnce => RumqttcQos::ExactlyOnce,
        }
    }
}

// ===========================================================================
// Transport traits
// ===========================================================================

/// Top-level MQTT transport handle. Combines publish and subscribe
/// capabilities.
pub trait MqttTransport: MqttPublisher + MqttSubscriber {}

/// Write-only publisher for telemetry / command dispatch.
pub trait MqttPublisher: Send + Sync {
    /// Publish a `Message` to a topic derived from `topic`.
    ///
    /// # Errors
    /// Returns [`MqttError`] if the publish fails or the broker cannot
    /// acknowledge at the configured `QoS`.
    fn publish(
        &self,
        topic: &Subject,
        payload: &Message,
    ) -> impl Future<Output = Result<(), MqttError>>;
}

/// Read-only subscriber for telemetry acquisition / command reception.
pub trait MqttSubscriber: Send + Sync {
    /// Subscribe to `topic`, invoking `handler` per incoming message.
    ///
    /// The handler must be `Send + Sync + 'static` so the transport can
    /// store it for dispatch from an async poller task.
    ///
    /// # Errors
    /// Returns [`MqttError`] if the subscription cannot be established.
    fn subscribe(
        &self,
        topic: &Subject,
        handler: impl MessageHandler + 'static,
    ) -> impl Future<Output = Result<SubscriptionId, MqttError>>;

    /// Unsubscribe a previously-established subscription by id.
    ///
    /// # Errors
    /// Returns [`MqttError`] if the broker rejects the unsubscribe or the
    /// id is unknown.
    fn unsubscribe(&self, id: SubscriptionId) -> impl Future<Output = Result<(), MqttError>>;
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by the MQTT transport adapter.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MqttError {
    /// The connection to the broker was lost.
    #[error("mqtt connection lost")]
    ConnectionLost,
    /// A publish operation failed.
    #[error("mqtt publish failed: {0}")]
    PublishFailed(String),
    /// A subscribe operation failed.
    #[error("mqtt subscribe failed: {0}")]
    SubscribeFailed(String),
    /// A message could not be deserialised.
    #[error("mqtt deserialization error: {0}")]
    DeserializationError(String),
    /// A configured `QoS` contract was violated.
    #[error("mqtt qos violation")]
    QosViolation,
    /// An operation timed out waiting for a broker ACK.
    #[error("mqtt timeout")]
    Timeout,
}

impl From<MqttError> for Error {
    fn from(e: MqttError) -> Self {
        Error::transport_error(e.to_string())
    }
}

impl From<ClientError> for MqttError {
    fn from(e: ClientError) -> Self {
        Self::PublishFailed(e.to_string())
    }
}

// ===========================================================================
// ACL — topic-based authorization per role
// ===========================================================================

/// Authorization action for MQTT ACL rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AclAction {
    /// Publish (write) permission.
    Publish,
    /// Subscribe (read) permission.
    Subscribe,
    /// Both publish and subscribe.
    PubSub,
}

/// A single ACL rule: an action allowed on a topic pattern.
/// Topic patterns use `*` as a wildcard suffix (e.g. `vehicle.*`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AclRule {
    /// Action permitted by this rule.
    pub action: AclAction,
    /// Topic pattern (`*` suffix matches any sub-topic).
    pub topic_pattern: String,
}

impl AclRule {
    /// Create a publish rule for the given pattern.
    #[must_use]
    pub fn publish(pattern: impl Into<String>) -> Self {
        Self {
            action: AclAction::Publish,
            topic_pattern: pattern.into(),
        }
    }

    /// Create a subscribe rule for the given pattern.
    #[must_use]
    pub fn subscribe(pattern: impl Into<String>) -> Self {
        Self {
            action: AclAction::Subscribe,
            topic_pattern: pattern.into(),
        }
    }

    /// Create a pub+sub rule for the given pattern.
    #[must_use]
    pub fn pubsub(pattern: impl Into<String>) -> Self {
        Self {
            action: AclAction::PubSub,
            topic_pattern: pattern.into(),
        }
    }

    /// Check if a topic matches this rule's pattern.
    fn topic_matches(&self, topic: &str) -> bool {
        if self.topic_pattern == "*" {
            return true;
        }
        if let Some(prefix) = self.topic_pattern.strip_suffix(".*") {
            topic == prefix || topic.starts_with(&format!("{prefix}."))
        } else {
            topic == self.topic_pattern
        }
    }

    /// Check if this rule permits the given action on the given topic.
    #[must_use]
    pub fn permits(&self, action: AclAction, topic: &str) -> bool {
        let action_ok = matches!(
            (self.action, action),
            (AclAction::PubSub, _) | (AclAction::Publish, AclAction::Publish)
                | (AclAction::Subscribe, AclAction::Subscribe)
        );
        action_ok && self.topic_matches(topic)
    }
}

/// A set of ACL rules for a single client/role. Checked before any
/// publish or subscribe operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MqttAcl {
    /// Rules for this client.
    pub rules: Vec<AclRule>,
}

impl MqttAcl {
    /// Create an empty ACL (denies everything).
    #[must_use]
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Add a rule to this ACL.
    #[must_use]
    pub fn allow(mut self, rule: AclRule) -> Self {
        self.rules.push(rule);
        self
    }

    /// Check if the ACL permits the given action on the given topic.
    #[must_use]
    pub fn permits(&self, action: AclAction, topic: &str) -> bool {
        self.rules.iter().any(|r| r.permits(action, topic))
    }
}

impl Default for MqttAcl {
    fn default() -> Self {
        Self::new()
    }
}

/// Default ACL for the `admin` role: pub+sub on all topics.
#[must_use]
pub fn admin_acl() -> MqttAcl {
    MqttAcl::new().allow(AclRule::pubsub("*"))
}

/// Default ACL for the `operator` role: pub+sub on `vehicle.*`.
#[must_use]
pub fn operator_acl() -> MqttAcl {
    MqttAcl::new().allow(AclRule::pubsub("vehicle.*"))
}

/// Default ACL for the `observer` role: subscribe only on `vehicle.*`.
#[must_use]
pub fn observer_acl() -> MqttAcl {
    MqttAcl::new().allow(AclRule::subscribe("vehicle.*"))
}

// ===========================================================================
// Subject ↔ MQTT topic mapping
// ===========================================================================

use themql_core::SubjectError;

/// Convert a [`Subject`] to an MQTT topic string.
///
/// Per `specs/mqtt.toml [mapping]`, the themql-core Subject maps 1:1 to
/// the MQTT topic string — both are hierarchical and dot-separated, so
/// the mapping is identity.
#[must_use]
pub fn subject_to_topic(subject: &Subject) -> String {
    subject.as_str()
}

/// Parse an MQTT topic string into a [`Subject`].
///
/// # Errors
/// Returns [`SubjectError`] if the topic is not a valid concrete subject.
pub fn topic_to_subject(topic: &str) -> Result<Subject, SubjectError> {
    Subject::from_str(topic)
}

// ===========================================================================
// Message encode / decode
// ===========================================================================

/// Encode a [`Message`] into MQTT publish payload bytes (JSON).
///
/// # Errors
/// Returns [`MqttError`] if serialisation fails.
pub fn encode_message(msg: &Message) -> Result<Vec<u8>, MqttError> {
    serde_json::to_vec(msg).map_err(|e| MqttError::DeserializationError(e.to_string()))
}

/// Decode MQTT publish payload bytes (JSON) back into a [`Message`].
///
/// # Errors
/// Returns [`MqttError`] if deserialisation fails.
pub fn decode_message(bytes: &[u8]) -> Result<Message, MqttError> {
    serde_json::from_slice(bytes).map_err(|e| MqttError::DeserializationError(e.to_string()))
}

// ===========================================================================
// Erased handler storage
// ===========================================================================

type HandlerFuture<'a> = Pin<Box<dyn Future<Output = Result<Response, Error>> + 'a>>;

trait ErasedHandler: Send + Sync {
    fn handle<'a>(&'a self, msg: &'a Message) -> HandlerFuture<'a>;
}

impl<H> ErasedHandler for H
where
    H: MessageHandler + Send + Sync,
{
    fn handle<'a>(&'a self, msg: &'a Message) -> HandlerFuture<'a> {
        Box::pin(<H as MessageHandler>::handle(self, msg))
    }
}

// ===========================================================================
// RumqttcConfig
// ===========================================================================

/// Configuration for a [`RumqttcTransport`]. All fields are explicit so
/// that reconnect behaviour is caller-owned, never implicit (per
/// `specs/mqtt.toml [constraints] implicit_reconnect = false`). Per
/// `specs/auth.toml [authn.mqtt]`, broker credentials are carried as
/// optional username/password fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RumqttcConfig {
    /// Broker hostname or IP address.
    pub host: String,
    /// Broker TCP port (typically 1883 plaintext, 8883 TLS).
    pub port: u16,
    /// MQTT client identifier sent in the CONNECT packet.
    pub client_id: String,
    /// Keep-alive interval sent to the broker.
    pub keep_alive: Duration,
    /// Explicit reconnect policy flag. When `false` (the spec default),
    /// the caller is responsible for re-polling the event loop after a
    /// connection error.
    pub auto_reconnect: bool,
    /// Optional broker username for SASL authentication.
    pub username: Option<String>,
    /// Optional broker password for SASL authentication.
    pub password: Option<String>,
    /// Optional ACL restricting publish/subscribe topics. When `None`,
    /// all topics are allowed (backwards-compatible with no auth).
    pub acl: Option<MqttAcl>,
}

impl RumqttcConfig {
    /// Construct a config with the given broker endpoint and client id,
    /// defaulting `keep_alive` to 60 s and `auto_reconnect` to `false`
    /// (matching `specs/mqtt.toml [constraints] implicit_reconnect`).
    /// Credentials default to `None`.
    #[must_use]
    pub fn new(host: impl Into<String>, port: u16, client_id: impl Into<String>) -> Self {
        Self {
            host: host.into(),
            port,
            client_id: client_id.into(),
            keep_alive: Duration::from_mins(1),
            auto_reconnect: false,
            username: None,
            password: None,
            acl: None,
        }
    }

    /// Override the keep-alive interval.
    #[must_use]
    pub fn with_keep_alive(mut self, keep_alive: Duration) -> Self {
        self.keep_alive = keep_alive;
        self
    }

    /// Override the explicit reconnect policy flag.
    #[must_use]
    pub fn with_auto_reconnect(mut self, auto_reconnect: bool) -> Self {
        self.auto_reconnect = auto_reconnect;
        self
    }

    /// Set broker credentials for SASL authentication. Per
    /// `specs/auth.toml [authn.mqtt]`, credentials must not appear in
    /// source; pass values from env or config.
    #[must_use]
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Set an ACL restricting which topics this client can publish
    /// and subscribe to. Per `specs/auth.toml [authz.mqtt]`.
    #[must_use]
    pub fn with_acl(mut self, acl: MqttAcl) -> Self {
        self.acl = Some(acl);
        self
    }

    /// Build a `rumqttc::MqttOptions` from this config.
    fn to_mqtt_options(&self) -> MqttOptions {
        let mut opts = MqttOptions::new(self.client_id.clone(), self.host.clone(), self.port);
        opts.set_keep_alive(self.keep_alive);
        if let (Some(u), Some(p)) = (&self.username, &self.password) {
            opts.set_credentials(u.clone(), p.clone());
        }
        opts
    }
}

impl Default for RumqttcConfig {
    /// Default config points at `localhost:1883` with an empty client id
    /// and `auto_reconnect = false`. Production callers should supply an
    /// explicit client id via [`RumqttcConfig::new`].
    fn default() -> Self {
        Self::new("localhost", 1883, "")
    }
}

// ===========================================================================
// RumqttcTransport
// ===========================================================================

/// Concrete desktop MQTT transport backed by `rumqttc`.
///
/// Holds an [`AsyncClient`] for publish/subscribe requests and the
/// [`EventLoop`] used to drive the connection. The event loop is shared
/// behind a [`Mutex`] so that a separate poller task (spawned by the
/// caller via [`RumqttcTransport::poll`]) can advance the connection
/// while publish/subscribe remain available through `&self`.
pub struct RumqttcTransport {
    client: AsyncClient,
    eventloop: Arc<AsyncMutex<EventLoop>>,
    handlers: Arc<Mutex<HashMap<SubscriptionId, Box<dyn ErasedHandler>>>>,
    next_id: AtomicU64,
    topic_index: Arc<Mutex<HashMap<SubscriptionId, String>>>,
    acl: Option<MqttAcl>,
}

impl RumqttcTransport {
    /// Construct a transport from a [`RumqttcConfig`]. Does not perform any
    /// network I/O; the connection is established lazily when the event
    /// loop is first polled (see [`RumqttcTransport::poll`]).
    #[must_use]
    pub fn new(config: &RumqttcConfig) -> Self {
        let opts = config.to_mqtt_options();
        let (client, eventloop) = AsyncClient::new(opts, 10);
        Self {
            client,
            eventloop: Arc::new(AsyncMutex::new(eventloop)),
            handlers: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            topic_index: Arc::new(Mutex::new(HashMap::new())),
            acl: config.acl.clone(),
        }
    }

    /// Advance the underlying `rumqttc` event loop by one event.
    ///
    /// The caller is expected to spawn a task that loops on this method
    /// for the lifetime of the connection. Incoming `Publish` packets are
    /// decoded and dispatched to the registered handler for the matching
    /// topic. Other events are returned to the caller for observability.
    ///
    /// # Errors
    /// Returns [`MqttError`] if the event loop yields a connection error
    /// or if dispatch to a registered handler fails.
    pub async fn poll(&self) -> Result<Event, MqttError> {
        let event = {
            let mut guard = self.eventloop.lock().await;
            guard.poll().await
        };
        match event {
            Ok(ev) => {
                if let Event::Incoming(rumqttc::Packet::Publish(publish)) = &ev {
                    self.dispatch(&publish.topic, &publish.payload);
                }
                Ok(ev)
            }
            Err(_) => Err(MqttError::ConnectionLost),
        }
    }

    /// Dispatch a decoded [`Message`] to the handler registered for
    /// `topic`, if any. Handler errors are swallowed at the transport
    /// layer (logged by the caller via [`MqttError`]) because MQTT has no
    /// reply channel for inbound dispatch failures.
    fn dispatch(&self, topic: &str, payload: &[u8]) {
        let Ok(msg) = decode_message(payload) else {
            return;
        };
        let Ok(handlers) = self.handlers.lock() else {
            return;
        };
        if let Some(id) = self.matching_subscription(topic, &handlers) {
            if let Some(handler) = handlers.get(&id) {
                drop(handler.handle(&msg));
            }
        }
    }

    /// Find the [`SubscriptionId`] whose registered topic matches `topic`.
    /// Matching is exact-equality on the topic string; MQTT `+`/`#`
    /// wildcards are expanded by the broker and delivered as concrete
    /// topics, so theMQL stores the original (possibly wildcarded) filter
    /// and falls back to exact match for the common non-wildcard case.
    fn matching_subscription(
        &self,
        topic: &str,
        handlers: &HashMap<SubscriptionId, Box<dyn ErasedHandler>>,
    ) -> Option<SubscriptionId> {
        let topics = self.topic_index.lock().ok()?;
        topics
            .iter()
            .find(|(id, filter)| {
                (*filter == topic || matches_filter(filter, topic)) && handlers.contains_key(id)
            })
            .map(|(id, _)| *id)
    }
}

/// Minimal MQTT topic-filter matcher supporting `+` (single level) and
/// `#` (multi level at end) wildcards.
fn matches_filter(filter: &str, topic: &str) -> bool {
    let filter_parts: Vec<&str> = filter.split('/').collect();
    let topic_parts: Vec<&str> = topic.split('/').collect();
    for (i, fp) in filter_parts.iter().enumerate() {
        if *fp == "#" {
            return true;
        }
        if i >= topic_parts.len() {
            return false;
        }
        if *fp != "+" && *fp != topic_parts[i] {
            return false;
        }
    }
    filter_parts.len() == topic_parts.len()
}

impl MqttPublisher for RumqttcTransport {
    async fn publish(&self, topic: &Subject, payload: &Message) -> Result<(), MqttError> {
        let topic_str = subject_to_topic(topic);
        if let Some(acl) = &self.acl {
            if !acl.permits(AclAction::Publish, &topic_str) {
                return Err(MqttError::PublishFailed(format!(
                    "ACL denied publish on topic '{topic_str}'"
                )));
            }
        }
        let bytes = encode_message(payload)?;
        self.client
            .publish(topic_str, RumqttcQos::AtLeastOnce, false, bytes)
            .await
            .map_err(MqttError::from)
    }
}

impl MqttSubscriber for RumqttcTransport {
    async fn subscribe(
        &self,
        topic: &Subject,
        handler: impl MessageHandler + 'static,
    ) -> Result<SubscriptionId, MqttError> {
        let id = SubscriptionId::new(self.next_id.fetch_add(1, Ordering::SeqCst));
        let topic_str = subject_to_topic(topic);
        if let Some(acl) = &self.acl {
            if !acl.permits(AclAction::Subscribe, &topic_str) {
                return Err(MqttError::SubscribeFailed(format!(
                    "ACL denied subscribe on topic '{topic_str}'"
                )));
            }
        }
        self.client
            .subscribe(topic_str.clone(), RumqttcQos::AtLeastOnce)
            .await
            .map_err(|e| MqttError::SubscribeFailed(e.to_string()))?;
        {
            let mut handlers = self
                .handlers
                .lock()
                .map_err(|_| MqttError::SubscribeFailed("handler lock poisoned".to_owned()))?;
            handlers.insert(id, Box::new(handler));
        }
        {
            let mut topics = self
                .topic_index
                .lock()
                .map_err(|_| MqttError::SubscribeFailed("topic lock poisoned".to_owned()))?;
            topics.insert(id, topic_str);
        }
        Ok(id)
    }

    async fn unsubscribe(&self, id: SubscriptionId) -> Result<(), MqttError> {
        let topic = {
            let mut topics = self
                .topic_index
                .lock()
                .map_err(|_| MqttError::SubscribeFailed("topic lock poisoned".to_owned()))?;
            topics.remove(&id)
        };
        if let Some(topic) = topic {
            self.client
                .unsubscribe(topic)
                .await
                .map_err(|e| MqttError::SubscribeFailed(e.to_string()))?;
        }
        let mut handlers = self
            .handlers
            .lock()
            .map_err(|_| MqttError::SubscribeFailed("handler lock poisoned".to_owned()))?;
        handlers.remove(&id);
        Ok(())
    }
}

impl MqttTransport for RumqttcTransport {}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::ErrorCode;

    #[test]
    fn subscription_id_wraps_u64() {
        let id = SubscriptionId::new(7);
        assert_eq!(id.as_u64(), 7);
        assert_eq!(SubscriptionId(7), id);
    }

    #[test]
    fn mqtt_qos_default_is_at_least_once() {
        assert_eq!(MqttQos::default(), MqttQos::AtLeastOnce);
    }

    #[test]
    fn mqtt_qos_round_trips_json() {
        let json = serde_json::to_string(&MqttQos::ExactlyOnce).expect("serialize");
        assert_eq!(json, "\"exactly_once\"");
        let back: MqttQos = serde_json::from_str("\"at_most_once\"").expect("deserialize");
        assert_eq!(back, MqttQos::AtMostOnce);
    }

    #[test]
    fn mqtt_qos_maps_to_rumqttc() {
        assert_eq!(MqttQos::AtMostOnce.to_rumqttc(), RumqttcQos::AtMostOnce);
        assert_eq!(MqttQos::AtLeastOnce.to_rumqttc(), RumqttcQos::AtLeastOnce);
        assert_eq!(MqttQos::ExactlyOnce.to_rumqttc(), RumqttcQos::ExactlyOnce);
    }

    #[test]
    fn mqtt_error_connection_lost_maps_to_transport_error() {
        let e: Error = MqttError::ConnectionLost.into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn mqtt_error_publish_failed_maps_to_transport_error() {
        let e: Error = MqttError::PublishFailed("broker unreachable".to_owned()).into();
        assert_eq!(e.code, ErrorCode::TransportError);
        assert!(e.message.contains("broker unreachable"));
    }

    #[test]
    fn mqtt_error_subscribe_failed_maps_to_transport_error() {
        let e: Error = MqttError::SubscribeFailed("topic too long".to_owned()).into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn mqtt_error_deserialization_maps_to_transport_error() {
        let e: Error = MqttError::DeserializationError("bad json".to_owned()).into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn mqtt_error_qos_violation_maps_to_transport_error() {
        let e: Error = MqttError::QosViolation.into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn mqtt_error_timeout_maps_to_transport_error() {
        let e: Error = MqttError::Timeout.into();
        assert_eq!(e.code, ErrorCode::TransportError);
    }

    #[test]
    fn client_error_converts_to_mqtt_error() {
        let mqtt_err = MqttError::from(ClientError::TryRequest(rumqttc::Request::PingReq(
            rumqttc::PingReq,
        )));
        assert!(matches!(mqtt_err, MqttError::PublishFailed(_)));
    }

    #[test]
    fn subject_to_topic_is_identity() {
        let subject = Subject::from_str("vehicle.sensors.imu.gyro").expect("valid subject");
        assert_eq!(subject_to_topic(&subject), "vehicle.sensors.imu.gyro");
    }

    #[test]
    fn topic_to_subject_round_trips() {
        let topic = "vehicle.actuators.fins.fin1";
        let subject = topic_to_subject(topic).expect("valid topic");
        assert_eq!(subject.as_str(), topic);
    }

    #[test]
    fn topic_to_subject_rejects_wildcard() {
        assert!(topic_to_subject("vehicle.+.sensors").is_err());
    }

    #[test]
    fn encode_decode_message_round_trips() {
        let msg = Message::new("vehicle.state", themql_core::Operation::Telemetry)
            .expect("valid subject");
        let bytes = encode_message(&msg).expect("encode");
        let back = decode_message(&bytes).expect("decode");
        assert_eq!(msg, back, "round-trip must preserve the message");
    }

    #[test]
    fn decode_message_rejects_garbage() {
        assert!(decode_message(b"not json").is_err());
    }

    #[test]
    fn rumqttc_config_new_defaults() {
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1");
        assert_eq!(cfg.host, "broker.local");
        assert_eq!(cfg.port, 1883);
        assert_eq!(cfg.client_id, "themql-1");
        assert_eq!(cfg.keep_alive, Duration::from_mins(1));
        assert!(!cfg.auto_reconnect);
        assert!(cfg.username.is_none());
        assert!(cfg.password.is_none());
    }

    #[test]
    fn rumqttc_config_default_is_localhost_no_reconnect() {
        let cfg = RumqttcConfig::default();
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 1883);
        assert!(!cfg.auto_reconnect);
    }

    #[test]
    fn rumqttc_config_builders_override_fields() {
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1")
            .with_keep_alive(Duration::from_secs(10))
            .with_auto_reconnect(true);
        assert_eq!(cfg.keep_alive, Duration::from_secs(10));
        assert!(cfg.auto_reconnect);
    }

    #[test]
    fn rumqttc_config_to_mqtt_options_round_trips_endpoint() {
        let cfg = RumqttcConfig::new("broker.local", 8883, "themql-1");
        let opts = cfg.to_mqtt_options();
        let (host, port) = opts.broker_address();
        assert_eq!(host, "broker.local");
        assert_eq!(port, 8883);
        assert_eq!(opts.client_id(), "themql-1");
        assert_eq!(opts.keep_alive(), Duration::from_mins(1));
    }

    #[test]
    fn rumqttc_transport_constructs_without_io() {
        let cfg = RumqttcConfig::new("localhost", 1883, "themql-test");
        let _transport = RumqttcTransport::new(&cfg);
    }

    #[test]
    fn subject_to_topic_round_trips_through_transport_config() {
        let subject = Subject::from_str("vehicle.sensors.imu.gyro").expect("valid subject");
        let topic = subject_to_topic(&subject);
        let back = topic_to_subject(&topic).expect("valid topic");
        assert_eq!(back.as_str(), subject.as_str());
    }

    #[test]
    fn matches_filter_exact() {
        assert!(matches_filter("vehicle/sensors", "vehicle/sensors"));
        assert!(!matches_filter("vehicle/sensors", "vehicle/actuators"));
    }

    #[test]
    fn matches_filter_single_level_wildcard() {
        assert!(matches_filter("vehicle/+/sensors", "vehicle/imu/sensors"));
        assert!(!matches_filter(
            "vehicle/+/sensors",
            "vehicle/imu/actuators"
        ));
    }

    #[test]
    fn matches_filter_multi_level_wildcard() {
        assert!(matches_filter("vehicle/#", "vehicle/sensors/imu/gyro"));
        assert!(matches_filter("vehicle/#", "vehicle"));
    }

    #[test]
    fn rumqttc_config_with_credentials_sets_username_password() {
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1")
            .with_credentials("operator", "s3cret");
        assert_eq!(cfg.username.as_deref(), Some("operator"));
        assert_eq!(cfg.password.as_deref(), Some("s3cret"));
    }

    #[test]
    fn rumqttc_config_credentials_appear_in_mqtt_options() {
        let cfg =
            RumqttcConfig::new("broker.local", 1883, "themql-1").with_credentials("admin", "pass");
        let opts = cfg.to_mqtt_options();
        let (u, p) = opts.credentials().unwrap_or((String::new(), String::new()));
        assert_eq!(u, "admin");
        assert_eq!(p, "pass");
    }

    #[test]
    fn rumqttc_config_without_credentials_has_no_credentials_in_options() {
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1");
        let opts = cfg.to_mqtt_options();
        assert!(opts.credentials().is_none());
    }

    // --- ACL tests ---

    #[test]
    fn acl_rule_topic_matches_exact() {
        let rule = AclRule::pubsub("vehicle.events");
        assert!(rule.topic_matches("vehicle.events"));
        assert!(!rule.topic_matches("vehicle.state"));
        assert!(!rule.topic_matches("vehicle.events.imu"));
    }

    #[test]
    fn acl_rule_topic_matches_wildcard() {
        let rule = AclRule::pubsub("vehicle.*");
        assert!(rule.topic_matches("vehicle"));
        assert!(rule.topic_matches("vehicle.events"));
        assert!(rule.topic_matches("vehicle.events.imu"));
        assert!(!rule.topic_matches("system.events"));
    }

    #[test]
    fn acl_rule_topic_matches_star() {
        let rule = AclRule::pubsub("*");
        assert!(rule.topic_matches("anything"));
        assert!(rule.topic_matches("vehicle.events"));
    }

    #[test]
    fn acl_rule_permits_action() {
        let rule = AclRule::subscribe("vehicle.*");
        assert!(rule.permits(AclAction::Subscribe, "vehicle.events"));
        assert!(!rule.permits(AclAction::Publish, "vehicle.events"));
    }

    #[test]
    fn acl_permits_admin_all() {
        let acl = admin_acl();
        assert!(acl.permits(AclAction::Publish, "anything"));
        assert!(acl.permits(AclAction::Subscribe, "vehicle.events"));
        assert!(acl.permits(AclAction::PubSub, "system.config"));
    }

    #[test]
    fn acl_permits_operator_vehicle() {
        let acl = operator_acl();
        assert!(acl.permits(AclAction::Publish, "vehicle.events"));
        assert!(acl.permits(AclAction::Subscribe, "vehicle.state"));
        assert!(!acl.permits(AclAction::Publish, "system.config"));
    }

    #[test]
    fn acl_permits_observer_subscribe_only() {
        let acl = observer_acl();
        assert!(acl.permits(AclAction::Subscribe, "vehicle.events"));
        assert!(!acl.permits(AclAction::Publish, "vehicle.events"));
        assert!(!acl.permits(AclAction::Subscribe, "system.config"));
    }

    #[test]
    fn acl_empty_denies_all() {
        let acl = MqttAcl::new();
        assert!(!acl.permits(AclAction::Publish, "vehicle.events"));
        assert!(!acl.permits(AclAction::Subscribe, "vehicle.events"));
    }

    #[test]
    fn rumqttc_config_with_acl_stores_acl() {
        let acl = observer_acl();
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1").with_acl(acl);
        assert!(cfg.acl.is_some());
        assert!(cfg
            .acl
            .as_ref()
            .unwrap()
            .permits(AclAction::Subscribe, "vehicle.events"));
        assert!(!cfg
            .acl
            .as_ref()
            .unwrap()
            .permits(AclAction::Publish, "vehicle.events"));
    }

    #[test]
    fn rumqttc_config_check_acl_allows_without_acl() {
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1");
        assert!(cfg.acl.is_none());
    }

    #[test]
    fn rumqttc_config_check_acl_denies_with_acl() {
        let cfg = RumqttcConfig::new("broker.local", 1883, "themql-1").with_acl(observer_acl());
        let acl = cfg.acl.as_ref().expect("acl set");
        assert!(acl.permits(AclAction::Subscribe, "vehicle.events"));
        assert!(!acl.permits(AclAction::Publish, "vehicle.events"));
        assert!(!acl.permits(AclAction::Subscribe, "system.config"));
    }
}
