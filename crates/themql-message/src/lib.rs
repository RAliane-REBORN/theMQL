//! # themql-message
//!
//! Message routing, subject hierarchy, subject parsing/matching, and
//! serialisation specialisation. The canonical `Message` type and its
//! fields are defined in [`themql_core`]; this crate specialises how
//! Messages are routed (subject grammar) and serialised (format
//! dispatch).
//!
//! See `specs/message.toml` for the authoritative specification.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

// Re-export the subject types from core so consumers can depend on
// themql-message alone for routing concerns.
pub use themql_core::{
    CausationId, CorrelationId, FormatTag, Message, MessageId, Metadata, Operation, PatternSegment,
    Payload, Principal, Subject, SubjectError, SubjectPattern, SubjectSegment, TraceId,
};

use thiserror::Error;

// ---------------------------------------------------------------------------
// Serializer — format dispatch over a Message payload
// ---------------------------------------------------------------------------

/// Format dispatch over a `Message` payload. themql-message owns the
/// trait so transport adapters can delegate to a single serializer
/// instead of each implementing format-specific logic.
pub trait Serializer: Send + Sync {
    /// Serialize a message into bytes per the given format.
    ///
    /// # Errors
    /// Returns [`MessageError`] if serialisation fails.
    fn serialize(&self, msg: &Message, format: FormatTag) -> Result<Vec<u8>, MessageError>;

    /// Deserialize bytes into a message per the given format.
    ///
    /// # Errors
    /// Returns [`MessageError`] if deserialisation fails.
    fn deserialize(&self, bytes: &[u8], format: FormatTag) -> Result<Message, MessageError>;
}

/// JSON serializer implementation. Uses `serde_json` for the text format.
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonSerializer;

impl Serializer for JsonSerializer {
    fn serialize(&self, msg: &Message, _format: FormatTag) -> Result<Vec<u8>, MessageError> {
        serde_json::to_vec(msg).map_err(|e| MessageError::SerializationError(e.to_string()))
    }

    fn deserialize(&self, bytes: &[u8], _format: FormatTag) -> Result<Message, MessageError> {
        serde_json::from_slice(bytes).map_err(|e| MessageError::DeserializationError(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// MessageError
// ---------------------------------------------------------------------------

/// Errors raised by the message subsystem (serialisation, format).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MessageError {
    /// Serialisation failed.
    #[error("serialization error: {0}")]
    SerializationError(String),
    /// Deserialisation failed.
    #[error("deserialization error: {0}")]
    DeserializationError(String),
    /// The requested format is not supported by this serializer.
    #[error("unsupported format: {0}")]
    UnsupportedFormat(FormatTag),
    /// The payload did not match the expected shape.
    #[error("payload mismatch")]
    PayloadMismatch,
}

impl From<MessageError> for themql_core::Error {
    fn from(e: MessageError) -> Self {
        Self::transport_error(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::Subject;

    #[test]
    fn json_serializer_round_trip() {
        let msg = Message::new("vehicle.sensors.imu.gyro", Operation::Telemetry).unwrap();
        let ser = JsonSerializer;
        let bytes = ser.serialize(&msg, FormatTag::Json).unwrap();
        let back = ser.deserialize(&bytes, FormatTag::Json).unwrap();
        assert_eq!(msg, back, "JSON round-trip must preserve the message");
    }

    #[test]
    fn json_serializer_rejects_garbage() {
        let ser = JsonSerializer;
        let result = ser.deserialize(b"not json", FormatTag::Json);
        assert!(matches!(result, Err(MessageError::DeserializationError(_))));
    }

    #[test]
    fn message_error_converts_to_core_error() {
        let e: themql_core::Error = MessageError::PayloadMismatch.into();
        assert_eq!(e.code, themql_core::ErrorCode::TransportError);
    }

    #[test]
    fn subject_re_exported_from_core() {
        let s = Subject::from_str("vehicle.sensors.imu.gyro").unwrap();
        assert_eq!(s.segments().len(), 4);
    }
}
