//! # themql-mqtt
//!
//! MQTT transport adapter for theMQL. Uses embassy for embedded MQTT and
//! delegates to tokio-based MQTT clients on desktop when needed. MQTT is
//! a transport, not a semantic owner. MQTT-specific types must not escape
//! this crate.
//!
//! See `specs/mqtt.toml` for the authoritative specification. This v0.1
//! crate declares only the public traits and supporting types; concrete
//! embassy / tokio-backed implementations are added in a later phase.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]
#![allow(async_fn_in_trait)]

use std::future::Future;

use serde::{Deserialize, Serialize};
use themql_core::{Error, Message, MessageHandler, Subject};
use thiserror::Error;

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
    /// # Errors
    /// Returns [`MqttError`] if the subscription cannot be established.
    fn subscribe(
        &self,
        topic: &Subject,
        handler: impl MessageHandler,
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
}
