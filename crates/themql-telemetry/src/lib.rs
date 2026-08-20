//! # themql-telemetry
//!
//! First-class telemetry message schema. Telemetry is a `Message` with
//! `operation = Operation::Telemetry` and a payload carrying a
//! [`TelemetryMessage`]. This crate owns the canonical sensor-reading
//! structs, the schema version, the sequence-number and timestamp types,
//! and the per-sensor reading types. Transports (mqtt, sse, graphql),
//! analysis (polars), and storage (helix-db) consume these types; they
//! must not redefine them.
//!
//! See `specs/telemetry.toml` for the authoritative specification.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use serde::{Deserialize, Serialize};
use serde_big_array::BigArray;
use themql_core::{Error, Subject, Timestamp};
use thiserror::Error;

// ===========================================================================
// TelemetryMessage — top-level envelope
// ===========================================================================

/// Top-level telemetry envelope. Serialised as the payload of a
/// `Message` with `operation = Operation::Telemetry`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TelemetryMessage {
    /// Schema version of this telemetry message. Current version is 1.
    pub schema_version: u16,
    /// Monotonic per-source sequence number.
    pub sequence_number: u64,
    /// Timestamp of the reading (canonical themql-core type).
    pub timestamp: Timestamp,
    /// Source subject identifying the sensor or derived source.
    pub source: Subject,
    /// The reading payload, one of the per-sensor / derived structs.
    pub reading: SensorReading,
}

// ===========================================================================
// SensorReading — enum over per-sensor structs
// ===========================================================================

/// Reading payload of a [`TelemetryMessage`]. One variant per raw sensor
/// or derived source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SensorReading {
    /// NEO-6M GPS reading.
    Gps(GpsReading),
    /// BME280 barometer reading.
    Barometer(BarometerReading),
    /// GY-LSM6DS3 IMU reading.
    Imu(ImuReading),
    /// Snapshot of the current EKF state estimate.
    StateEstimate(StateEstimateReading),
    /// State estimate covariance (21x21, flat row-major).
    Covariance(Box<CovarianceReading>),
    /// Snapshot of the active controller's state.
    ControllerState(Box<ControllerStateReading>),
    /// Snapshot of the currently-active ML model.
    ModelState(ModelStateReading),
    /// Diagnostics message.
    Diagnostics(DiagnosticsReading),
}

/// Tag for raw sensor kinds. Used for source tagging and sensor-fusion
/// dispatch; derived sources ([`StateEstimate`] etc.) are not raw sensor
/// reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorKind {
    /// GPS receiver.
    Gps,
    /// Barometer.
    Barometer,
    /// IMU.
    Imu,
}

// ===========================================================================
// Per-sensor structs — NEO-6M GPS
// ===========================================================================

/// Reading from a NEO-6M GPS. Fix quality flagged; hdop/vdop for
/// uncertainty.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpsReading {
    /// Fix quality.
    pub fix: GpsFix,
    /// Latitude in degrees.
    pub latitude_deg: f64,
    /// Longitude in degrees.
    pub longitude_deg: f64,
    /// Altitude in metres.
    pub altitude_m: f64,
    /// Velocity north in metres per second.
    pub velocity_north_mps: f64,
    /// Velocity east in metres per second.
    pub velocity_east_mps: f64,
    /// Velocity down in metres per second.
    pub velocity_down_mps: f64,
    /// Horizontal dilution of precision.
    pub hdop: f32,
    /// Vertical dilution of precision.
    pub vdop: f32,
    /// Number of satellites used for this fix.
    pub satellites_used: u8,
}

/// GPS fix quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpsFix {
    /// No fix.
    NoFix,
    /// 2D fix (position only).
    TwoD,
    /// 3D fix (position + altitude).
    ThreeD,
    /// Differential GPS fix.
    Dgps,
    /// RTK fixed-integer solution.
    RtkFixed,
    /// RTK float solution.
    RtkFloat,
}

// ===========================================================================
// Per-sensor structs — BME280 barometer
// ===========================================================================

/// Reading from a BME280 barometer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BarometerReading {
    /// Pressure in pascals.
    pub pressure_pa: f64,
    /// Temperature in degrees Celsius.
    pub temperature_c: f64,
    /// Relative humidity in percent (BME280 only; BMP280 leaves this 0).
    pub humidity_pct: f32,
    /// Altitude in metres, derived from pressure via standard atmosphere.
    pub altitude_m: f64,
}

// ===========================================================================
// Per-sensor structs — GY-LSM6DS3 IMU
// ===========================================================================

/// Reading from a GY-LSM6DS3 IMU (accelerometer + gyroscope in one
/// package).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImuReading {
    /// Accelerometer X axis in metres per second squared.
    pub accel_x_mps2: f64,
    /// Accelerometer Y axis in metres per second squared.
    pub accel_y_mps2: f64,
    /// Accelerometer Z axis in metres per second squared.
    pub accel_z_mps2: f64,
    /// Gyroscope X axis in radians per second.
    pub gyro_x_radps: f64,
    /// Gyroscope Y axis in radians per second.
    pub gyro_y_radps: f64,
    /// Gyroscope Z axis in radians per second.
    pub gyro_z_radps: f64,
    /// Die temperature in degrees Celsius.
    pub temperature_c: f32,
}

// ===========================================================================
// Derived — state estimate (from themql-estimation)
// ===========================================================================

/// Snapshot of the current EKF state estimate. Produced by
/// `themql-estimation`; serialised here so consumers don't depend on
/// estimation internals. Quaternion is `[w, x, y, z]` (Hamilton
/// convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateEstimateReading {
    /// Position `[x, y, z]` in metres.
    pub position: [f64; 3],
    /// Velocity `[vx, vy, vz]` in metres per second.
    pub velocity: [f64; 3],
    /// Attitude quaternion `[w, x, y, z]` (unit norm, Hamilton).
    pub attitude_quaternion: [f64; 4],
    /// Angular velocity `[wx, wy, wz]` in radians per second.
    pub angular_velocity: [f64; 3],
    /// Accelerometer bias `[bx, by, bz]`.
    pub accelerometer_bias: [f64; 3],
    /// Gyroscope bias `[bx, by, bz]`.
    pub gyroscope_bias: [f64; 3],
    /// Barometric altitude bias.
    pub baro_altitude_bias: f64,
    /// GPS clock bias.
    pub gps_clock_bias: f64,
}

/// State estimate covariance (21x21). Serialised as a flat `[f64; 441]`
/// array in row-major order, plus the dimension for forward compat.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CovarianceReading {
    /// Matrix dimension. Always 21 for schema version 1.
    pub dimension: u16,
    /// Flat row-major covariance values (dimension * dimension entries).
    #[serde(with = "BigArray")]
    pub values: [f64; 441],
}

// ===========================================================================
// Derived — controller state (from themql-gnc)
// ===========================================================================

/// Snapshot of the active controller's state and latest actuator
/// commands.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ControllerStateReading {
    /// Which controller is active.
    pub active_controller: ControllerKind,
    /// PID controller state, if the active controller is PID.
    pub pid_state: Option<PidState>,
    /// LQRI controller state, if the active controller is LQRI.
    pub lqri_state: Option<LqriState>,
    /// Fixed-size actuator vector; unused slots are `0.0`.
    pub actuator_commands: [f64; 8],
}

/// Kind of active controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControllerKind {
    /// PID controller.
    Pid,
    /// LQRI controller.
    Lqri,
    /// Hybrid PID + LQRI controller.
    Hybrid,
}

/// PID controller state snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PidState {
    /// Setpoint vector.
    pub setpoint: [f64; 3],
    /// Measured vector.
    pub measured: [f64; 3],
    /// Error vector.
    pub error: [f64; 3],
    /// Integral term.
    pub integral: [f64; 3],
    /// Derivative term.
    pub derivative: [f64; 3],
    /// Controller output.
    pub output: [f64; 3],
}

/// LQRI controller state snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LqriState {
    /// State vector (copy of `StateEstimateReading` for controller input).
    pub state: [f64; 21],
    /// Control vector (actuator commands).
    pub control: [f64; 8],
}

// ===========================================================================
// Derived — model state (from themql-inference)
// ===========================================================================

/// Snapshot of the currently-active ML model and inference budget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelStateReading {
    /// Model identifier.
    pub model_id: String,
    /// Model version string.
    pub model_version: String,
    /// Whether this model is currently active.
    pub active: bool,
    /// Resource budget summary.
    pub resource_budget: ResourceBudgetSummary,
    /// Latency of the most recent inference call in nanoseconds.
    pub last_inference_latency_ns: u64,
    /// Whether a rollback to a previous model has been engaged.
    pub rollback_engaged: bool,
}

/// Summary of the resource budget granted to a model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBudgetSummary {
    /// CPU budget in percent.
    pub cpu_budget_pct: u8,
    /// Memory budget in bytes.
    pub memory_budget_bytes: u64,
    /// Inference deadline in milliseconds.
    pub inference_deadline_ms: u16,
}

// ===========================================================================
// Diagnostics
// ===========================================================================

/// Diagnostics reading. Carries a level, a stable code, a human-readable
/// message, and optional structured details.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiagnosticsReading {
    /// Severity level.
    pub level: DiagnosticLevel,
    /// Stable diagnostic code.
    pub code: String,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source subject.
    pub source: Subject,
    /// Optional structured details.
    pub details: Option<serde_json::Value>,
}

/// Severity level of a diagnostics reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    /// Informational.
    Info,
    /// Warning.
    Warn,
    /// Error.
    Error,
    /// Critical — system safety may be compromised.
    Critical,
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by telemetry schema validation and (de)serialisation.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TelemetryError {
    /// Schema version is not supported by this crate.
    #[error("unsupported telemetry schema version: got {got}, supported {supported}")]
    SchemaVersionUnsupported {
        /// The schema version found in the message.
        got: u16,
        /// The maximum schema version supported by this crate.
        supported: u16,
    },
    /// Sequence number overflowed its monotonic range.
    #[error("telemetry sequence number overflowed")]
    SequenceOverflow,
    /// Serialisation or deserialisation failed.
    #[error("telemetry serialisation error: {0}")]
    SerializationError(String),
    /// An unknown sensor kind was encountered during deserialisation.
    #[error("unknown sensor kind: {0}")]
    UnknownSensorKind(String),
}

impl From<TelemetryError> for Error {
    fn from(e: TelemetryError) -> Self {
        match e {
            TelemetryError::SchemaVersionUnsupported { .. }
            | TelemetryError::UnknownSensorKind(_) => Error::validation_error(e.to_string()),
            TelemetryError::SequenceOverflow | TelemetryError::SerializationError(_) => {
                Error::internal_error(e.to_string())
            }
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use themql_core::{ErrorCode, Subject, Timestamp, TimestampKind};

    fn sample_subject() -> Subject {
        Subject::from_str("vehicle.sensors.imu.gyro").expect("valid subject")
    }

    fn sample_timestamp() -> Timestamp {
        Timestamp::new(TimestampKind::Monotonic, 1_000_000_000)
    }

    #[test]
    fn telemetry_message_round_trips_json() {
        let msg = TelemetryMessage {
            schema_version: 1,
            sequence_number: 42,
            timestamp: sample_timestamp(),
            source: sample_subject(),
            reading: SensorReading::Imu(ImuReading {
                accel_x_mps2: 0.1,
                accel_y_mps2: 0.2,
                accel_z_mps2: 9.8,
                gyro_x_radps: 0.0,
                gyro_y_radps: 0.0,
                gyro_z_radps: 0.0,
                temperature_c: 25.0,
            }),
        };
        let json = serde_json::to_string(&msg).expect("serialize");
        let back: TelemetryMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(msg, back);
    }

    #[test]
    fn sensor_reading_gps_variant_round_trips() {
        let reading = SensorReading::Gps(GpsReading {
            fix: GpsFix::ThreeD,
            latitude_deg: 37.0,
            longitude_deg: -122.0,
            altitude_m: 100.0,
            velocity_north_mps: 1.0,
            velocity_east_mps: 2.0,
            velocity_down_mps: 0.0,
            hdop: 1.2,
            vdop: 2.0,
            satellites_used: 9,
        });
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_reading_barometer_variant_round_trips() {
        let reading = SensorReading::Barometer(BarometerReading {
            pressure_pa: 101_325.0,
            temperature_c: 20.0,
            humidity_pct: 50.0,
            altitude_m: 0.0,
        });
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_reading_state_estimate_variant_round_trips() {
        let reading = SensorReading::StateEstimate(StateEstimateReading {
            position: [1.0, 2.0, 3.0],
            velocity: [0.1, 0.2, 0.3],
            attitude_quaternion: [1.0, 0.0, 0.0, 0.0],
            angular_velocity: [0.0; 3],
            accelerometer_bias: [0.0; 3],
            gyroscope_bias: [0.0; 3],
            baro_altitude_bias: 0.0,
            gps_clock_bias: 0.0,
        });
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_reading_covariance_variant_round_trips() {
        let mut values = [0.0_f64; 441];
        for (i, slot) in values.iter_mut().enumerate() {
            *slot = if i % 22 == 0 { 1.0 } else { 0.0 };
        }
        let reading = SensorReading::Covariance(Box::new(CovarianceReading {
            dimension: 21,
            values,
        }));
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_reading_controller_state_variant_round_trips() {
        let reading = SensorReading::ControllerState(Box::new(ControllerStateReading {
            active_controller: ControllerKind::Pid,
            pid_state: Some(PidState {
                setpoint: [1.0, 0.0, 0.0],
                measured: [0.9, 0.0, 0.0],
                error: [0.1, 0.0, 0.0],
                integral: [0.0; 3],
                derivative: [0.0; 3],
                output: [0.05, 0.0, 0.0],
            }),
            lqri_state: None,
            actuator_commands: [0.0; 8],
        }));
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_reading_model_state_variant_round_trips() {
        let reading = SensorReading::ModelState(ModelStateReading {
            model_id: "anomaly-detector".to_owned(),
            model_version: "0.1.0".to_owned(),
            active: true,
            resource_budget: ResourceBudgetSummary {
                cpu_budget_pct: 20,
                memory_budget_bytes: 1024 * 1024,
                inference_deadline_ms: 50,
            },
            last_inference_latency_ns: 1_000_000,
            rollback_engaged: false,
        });
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_reading_diagnostics_variant_round_trips() {
        let reading = SensorReading::Diagnostics(DiagnosticsReading {
            level: DiagnosticLevel::Warn,
            code: "GPS_FIX_DEGRADED".to_owned(),
            message: "GPS fix dropped to 2D".to_owned(),
            source: sample_subject(),
            details: Some(serde_json::json!({"satellites": 3})),
        });
        let json = serde_json::to_string(&reading).expect("serialize");
        let back: SensorReading = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(reading, back);
    }

    #[test]
    fn sensor_kind_serialises_snake_case() {
        let json = serde_json::to_string(&SensorKind::Gps).expect("serialize");
        assert_eq!(json, "\"gps\"");
        let back: SensorKind = serde_json::from_str("\"barometer\"").expect("deserialize");
        assert_eq!(back, SensorKind::Barometer);
    }

    #[test]
    fn gps_fix_serialises_snake_case() {
        let json = serde_json::to_string(&GpsFix::RtkFixed).expect("serialize");
        assert_eq!(json, "\"rtk_fixed\"");
        let back: GpsFix = serde_json::from_str("\"no_fix\"").expect("deserialize");
        assert_eq!(back, GpsFix::NoFix);
    }

    #[test]
    fn telemetry_error_schema_version_maps_to_validation_error() {
        let e: Error = TelemetryError::SchemaVersionUnsupported {
            got: 99,
            supported: 1,
        }
        .into();
        assert_eq!(e.code, ErrorCode::ValidationError);
    }

    #[test]
    fn telemetry_error_unknown_sensor_kind_maps_to_validation_error() {
        let e: Error = TelemetryError::UnknownSensorKind("radar".to_owned()).into();
        assert_eq!(e.code, ErrorCode::ValidationError);
    }

    #[test]
    fn telemetry_error_sequence_overflow_maps_to_internal_error() {
        let e: Error = TelemetryError::SequenceOverflow.into();
        assert_eq!(e.code, ErrorCode::InternalError);
    }

    #[test]
    fn telemetry_error_serialization_maps_to_internal_error() {
        let e: Error = TelemetryError::SerializationError("boom".to_owned()).into();
        assert_eq!(e.code, ErrorCode::InternalError);
    }
}
