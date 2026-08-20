//! # themql-embedded
//!
//! Embedded binary stub. Runs on the flight controller
//! (`thumbv7em` or `riscv32imc`). Per `specs/embedded.toml`, this binary
//! composes `themql-runtime` (embassy), `themql-mqtt`, `themql-gnc`,
//! `themql-estimation`, `themql-inference`, `themql-artifact`, and
//! `themql-telemetry`, and must not depend on desktop infrastructure.
//!
//! This is a stub that compiles on the host target. Embassy is deferred
//! (it does not compile for the x86 host target). The
//! [`SensorDriver`] trait, [`SensorError`] enum, and stub driver impls
//! are real and unit-tested.
//!
//! ## TETANUS compliance
//!
//! Per `specs/embedded.toml [tetanus]` and `TETANUS.md`: no `unsafe`,
//! no recursion, no dynamic allocation after init in hot loops, no
//! `unwrap()`/`expect()`, all loops have fixed bounds, functions ≤ 60
//! lines, `clippy::pedantic` + `deny(warnings)` clean.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_docs)]
#![allow(clippy::module_name_repetitions)]

use std::fmt;

// ===========================================================================
// SensorKind + SensorReading + SensorError
// ===========================================================================

/// Kind of sensor a driver reads. Mirrors `specs/embedded.toml [sensors]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SensorKind {
    /// GPS receiver (e.g. NEO-6M).
    Gps,
    /// Barometer (e.g. BME280).
    Barometer,
    /// IMU (e.g. GY-LSM6DS3).
    Imu,
}

/// A single reading from a sensor. Minimal stand-in: the canonical
/// reading carries typed payloads per sensor kind; this stub holds the
/// kind and a raw byte buffer of fixed maximum length so no dynamic
/// allocation occurs in the hot path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SensorReading {
    /// Source sensor kind.
    pub kind: SensorKind,
    /// Raw reading bytes (fixed-capacity, ≤ 32 bytes per TETANUS).
    pub raw: [u8; 32],
    /// Number of valid bytes in `raw`.
    pub len: u8,
}

/// Errors raised by a sensor driver. Mirrors
/// `specs/embedded.toml [api.SensorError]`. Manual `Display` impl keeps
/// the binary free of `thiserror` (TETANUS: minimal deps).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SensorError {
    /// Bus-level I/O failure (I2C/SPI/UART), with a short descriptor.
    BusError(String),
    /// The read did not complete within the deadline.
    Timeout,
    /// The sensor did not respond at all.
    SensorNotResponding,
    /// The sensor returned a reading that failed validation.
    InvalidReading(String),
    /// The sensor requires calibration before use.
    CalibrationRequired,
}

impl fmt::Display for SensorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BusError(s) => write!(f, "bus error: {s}"),
            Self::Timeout => f.write_str("sensor read timeout"),
            Self::SensorNotResponding => f.write_str("sensor not responding"),
            Self::InvalidReading(s) => write!(f, "invalid reading: {s}"),
            Self::CalibrationRequired => f.write_str("calibration required"),
        }
    }
}

impl std::error::Error for SensorError {}

// ===========================================================================
// SensorDriver trait
// ===========================================================================

/// Trait for sensor drivers. Each driver reads its sensor on a fixed
/// schedule and produces a [`SensorReading`]. Mirrors
/// `specs/embedded.toml [api.SensorDriver]`.
///
/// The canonical form is `async` (embassy tasks); this stub uses a
/// synchronous `read` so the binary compiles on the host without
/// embassy. The trait surface is otherwise identical.
pub trait SensorDriver: Send + Sync {
    /// Read one sample from the sensor.
    ///
    /// # Errors
    /// Returns [`SensorError`] on any read failure.
    fn read(&mut self) -> Result<SensorReading, SensorError>;

    /// Returns the kind of sensor this driver reads.
    fn kind(&self) -> SensorKind;

    /// Returns the configured read rate in hertz.
    fn rate_hz(&self) -> u32;
}

// ===========================================================================
// Stub driver impls
// ===========================================================================

/// Stub GPS driver. Real impl holds a UART peripheral handle.
#[derive(Debug, Clone, Copy)]
pub struct GpsDriver {
    /// Configured read rate (Hz).
    pub rate_hz: u32,
}

impl GpsDriver {
    /// Construct a stub GPS driver with the given rate.
    #[must_use]
    pub fn new(rate_hz: u32) -> Self {
        Self { rate_hz }
    }
}

impl SensorDriver for GpsDriver {
    fn read(&mut self) -> Result<SensorReading, SensorError> {
        Ok(SensorReading {
            kind: SensorKind::Gps,
            raw: [0; 32],
            len: 0,
        })
    }

    fn kind(&self) -> SensorKind {
        SensorKind::Gps
    }

    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
}

/// Stub barometer driver. Real impl holds an I2C peripheral handle.
#[derive(Debug, Clone, Copy)]
pub struct BaroDriver {
    /// Configured read rate (Hz).
    pub rate_hz: u32,
}

impl BaroDriver {
    /// Construct a stub barometer driver with the given rate.
    #[must_use]
    pub fn new(rate_hz: u32) -> Self {
        Self { rate_hz }
    }
}

impl SensorDriver for BaroDriver {
    fn read(&mut self) -> Result<SensorReading, SensorError> {
        Ok(SensorReading {
            kind: SensorKind::Barometer,
            raw: [0; 32],
            len: 0,
        })
    }

    fn kind(&self) -> SensorKind {
        SensorKind::Barometer
    }

    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
}

/// Stub IMU driver. Real impl holds an SPI/I2C peripheral handle.
#[derive(Debug, Clone, Copy)]
pub struct ImuDriver {
    /// Configured read rate (Hz).
    pub rate_hz: u32,
}

impl ImuDriver {
    /// Construct a stub IMU driver with the given rate.
    #[must_use]
    pub fn new(rate_hz: u32) -> Self {
        Self { rate_hz }
    }
}

impl SensorDriver for ImuDriver {
    fn read(&mut self) -> Result<SensorReading, SensorError> {
        Ok(SensorReading {
            kind: SensorKind::Imu,
            raw: [0; 32],
            len: 0,
        })
    }

    fn kind(&self) -> SensorKind {
        SensorKind::Imu
    }

    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
}

// ===========================================================================
// Embassy task topology (stub main)
// ===========================================================================

/// Embedded entry point (stub).
///
/// Per `specs/embedded.toml [entry]`, the canonical form is an
/// `#[embassy_executor::main]` `async fn` that spawns sensor tasks
/// (gps, baro, imu), an estimator task (EKF), a controller task
/// (`HybridController`), a telemetry task (MQTT publisher at 10 Hz on
/// `vehicle.state_estimate`, `vehicle.covariance`,
/// `vehicle.controller_state`, `vehicle.diagnostics`), an inference
/// task (ML state correction at 5 Hz with budget enforcement + rollback
/// ready), and a command task (subscribes to command subject, dispatches
/// to controller setpoint), then runs the main loop at IMU rate.
///
/// Embassy does not compile for the x86 host target, so this stub
/// prints a banner and returns. The real embassy main is a future task.
#[allow(clippy::unnecessary_wraps)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("themql-embedded: stub");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensor_error_variants_display() {
        assert_eq!(
            SensorError::BusError("nak".to_owned()).to_string(),
            "bus error: nak"
        );
        assert_eq!(SensorError::Timeout.to_string(), "sensor read timeout");
        assert_eq!(
            SensorError::SensorNotResponding.to_string(),
            "sensor not responding"
        );
        assert_eq!(
            SensorError::InvalidReading("bad crc".to_owned()).to_string(),
            "invalid reading: bad crc"
        );
        assert_eq!(
            SensorError::CalibrationRequired.to_string(),
            "calibration required"
        );
    }

    #[test]
    fn sensor_driver_trait_object_construction() {
        let mut drivers: Vec<Box<dyn SensorDriver>> = vec![
            Box::new(GpsDriver::new(10)),
            Box::new(BaroDriver::new(50)),
            Box::new(ImuDriver::new(200)),
        ];
        assert_eq!(drivers.len(), 3);
        for d in &mut drivers {
            let _ = d.read().unwrap();
        }
        assert_eq!(drivers[0].kind(), SensorKind::Gps);
        assert_eq!(drivers[1].kind(), SensorKind::Barometer);
        assert_eq!(drivers[2].kind(), SensorKind::Imu);
        assert_eq!(drivers[0].rate_hz(), 10);
        assert_eq!(drivers[1].rate_hz(), 50);
        assert_eq!(drivers[2].rate_hz(), 200);
    }

    #[test]
    fn gps_driver_reads_gps_kind() {
        let mut d = GpsDriver::new(10);
        let r = d.read().unwrap();
        assert_eq!(r.kind, SensorKind::Gps);
    }

    #[test]
    fn sensor_reading_raw_is_fixed_capacity() {
        let r = SensorReading {
            kind: SensorKind::Imu,
            raw: [0; 32],
            len: 0,
        };
        assert_eq!(r.raw.len(), 32);
        assert_eq!(r.len, 0);
    }
}
