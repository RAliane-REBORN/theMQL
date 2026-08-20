//! # themql-estimation
//!
//! Embedded state estimation. The EKF is the primary estimator with Bayesian
//! updating and uncertainty propagation. ML is allowed only as augmentative
//! state correction, never as the primary estimator. Quaternion attitude
//! is canonical (per `SPEC.toml` and the nalgebra amendment).
//!
//! See `specs/state_estimation.toml` for the authoritative specification.
//!
//! ## TETANUS
//!
//! Safety-critical: no recursion, fixed loop bounds, no heap allocation
//! after `new()`, functions ≤ 60 lines, ≥ 2 assertions per function (as
//! `if !invariant { return Err }`), no `unwrap()`/`expect()`.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(missing_docs)]
#![allow(non_snake_case)]

use nalgebra::{SMatrix, SVector};
use thiserror::Error;

/// State dimension — fixed at compile time per `specs/state_estimation.toml`.
pub const STATE_DIM: usize = 21;

/// Covariance divergence threshold (trace). Above this the estimator
/// reports `StateDiverged`.
const COV_TRACE_LIMIT: f64 = 1.0e12;

// ===========================================================================
// Estimator state
// ===========================================================================

/// EKF state: 21-dim state vector `x`, 21×21 covariance `P`, timestamp `t`.
/// All nalgebra fixed-size types — stack-allocated, no heap.
#[derive(Debug, Clone, PartialEq)]
pub struct EstimatorState {
    /// State vector.
    pub x: SVector<f64, STATE_DIM>,
    /// Covariance matrix.
    pub P: SMatrix<f64, STATE_DIM, STATE_DIM>,
    /// Timestamp in seconds.
    pub t: f64,
}

impl EstimatorState {
    /// Construct a zero state with identity covariance at `t = 0`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            x: SVector::zeros(),
            P: SMatrix::identity(),
            t: 0.0,
        }
    }
}

impl Default for EstimatorState {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Sensor readings — minimal local stubs matching telemetry schema
// ===========================================================================

/// IMU reading: accelerometer + gyroscope. Minimal local stub matching
/// `specs/telemetry.toml` shapes; the canonical types live in
/// `themql-telemetry` when implemented.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImuReading {
    /// Accelerometer `[ax, ay, az]` m/s².
    pub accel: [f64; 3],
    /// Gyroscope `[gx, gy, gz]` rad/s.
    pub gyro: [f64; 3],
}

/// GPS reading: position + velocity + clock bias. Minimal local stub.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpsReading {
    /// Position `[x, y, z]` metres.
    pub position: [f64; 3],
    /// Velocity `[vx, vy, vz]` m/s.
    pub velocity: [f64; 3],
    /// GPS clock bias seconds.
    pub clock_bias: f64,
}

/// Barometer reading: altitude + altitude bias. Minimal local stub.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BarometerReading {
    /// Altitude metres.
    pub altitude: f64,
    /// Altitude bias metres.
    pub altitude_bias: f64,
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by state estimators.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EstimationError {
    /// Covariance diverged beyond the trace limit.
    #[error("state diverged: covariance trace {covariance_trace}")]
    StateDiverged {
        /// The observed covariance trace.
        covariance_trace: f64,
    },
    /// Attitude quaternion was not unit norm.
    #[error("quaternion not unit norm: {norm}")]
    QuaternionNotUnitNorm {
        /// The observed norm.
        norm: f64,
    },
    /// A sensor reading was invalid.
    #[error("invalid sensor reading: {reason}")]
    SensorReadingInvalid {
        /// Why the reading was invalid.
        reason: String,
    },
    /// A matrix was not positive definite.
    #[error("matrix not positive definite")]
    MatrixNotPositiveDefinite,
    /// Time step `dt` was not positive.
    #[error("dt not positive: {dt}")]
    DtNotPositive {
        /// The offending dt.
        dt: f64,
    },
    /// Internal estimator failure.
    #[error("internal estimation error: {0}")]
    InternalError(String),
}

impl From<EstimationError> for themql_core::Error {
    fn from(e: EstimationError) -> Self {
        themql_core::Error::internal_error(e.to_string())
    }
}

// ===========================================================================
// Estimator trait
// ===========================================================================

/// Primary estimator trait. Predict and update are separate steps so the
/// runtime can interleave them at the correct rates (IMU predict high-rate,
/// GPS/baro update lower-rate).
pub trait Estimator: Send + Sync {
    /// Propagate state forward by `dt` using the IMU reading.
    ///
    /// # Errors
    /// Returns [`EstimationError`] on invalid dt or diverged state.
    fn predict(&mut self, dt: f64, imu: &ImuReading) -> Result<(), EstimationError>;
    /// Update with a GPS reading.
    ///
    /// # Errors
    /// Returns [`EstimationError`] on invalid reading or diverged state.
    fn update_gps(&mut self, gps: &GpsReading) -> Result<(), EstimationError>;
    /// Update with a barometer reading.
    ///
    /// # Errors
    /// Returns [`EstimationError`] on invalid reading or diverged state.
    fn update_baro(&mut self, baro: &BarometerReading) -> Result<(), EstimationError>;
    /// Borrow the current state.
    fn state(&self) -> &EstimatorState;
    /// Borrow the current covariance.
    fn covariance(&self) -> &SMatrix<f64, STATE_DIM, STATE_DIM>;
}

// ===========================================================================
// EKF — v0.1 simplified implementation
// ===========================================================================

/// Canonical Extended Kalman Filter. All matrices are nalgebra `SMatrix`
/// fixed-size — stack-allocated, no heap. The v0.1 implementation uses a
/// simplified predict (identity propagation + process-noise add) and
/// simplified identity-gain updates; full nonlinear quaternion dynamics
/// and Jacobian-based Kalman gain are a future task.
#[derive(Debug, Clone, PartialEq)]
pub struct Ekf {
    /// State + covariance + timestamp.
    state: EstimatorState,
    /// Process noise covariance `Q` (21×21).
    process_noise: SMatrix<f64, STATE_DIM, STATE_DIM>,
    /// GPS measurement noise `R_gps` (7×7).
    gps_noise: SMatrix<f64, 7, 7>,
    /// Baro measurement noise `R_baro` (2×2).
    baro_noise: SMatrix<f64, 2, 2>,
    /// IMU noise covariance `R_imu` (6×6).
    imu_noise: SMatrix<f64, 6, 6>,
}

impl Ekf {
    /// Construct an EKF with zero state, identity covariance, and small
    /// diagonal noise matrices. No heap allocation occurs after this.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: EstimatorState::new(),
            process_noise: SMatrix::identity() * 1.0e-6,
            gps_noise: SMatrix::identity() * 1.0e-3,
            baro_noise: SMatrix::identity() * 1.0e-2,
            imu_noise: SMatrix::identity() * 1.0e-4,
        }
    }

    /// Check the covariance trace for divergence.
    fn check_divergence(&self) -> Result<(), EstimationError> {
        let trace = self.state.P.trace();
        if !trace.is_finite() {
            return Err(EstimationError::StateDiverged {
                covariance_trace: trace,
            });
        }
        if trace > COV_TRACE_LIMIT {
            return Err(EstimationError::StateDiverged {
                covariance_trace: trace,
            });
        }
        Ok(())
    }

    /// Validate an IMU reading: all components finite.
    fn validate_imu(imu: &ImuReading) -> Result<(), EstimationError> {
        for i in 0..3 {
            if !imu.accel[i].is_finite() || !imu.gyro[i].is_finite() {
                return Err(EstimationError::SensorReadingInvalid {
                    reason: "imu reading not finite".to_string(),
                });
            }
        }
        Ok(())
    }

    /// Validate a GPS reading: all components finite.
    fn validate_gps(gps: &GpsReading) -> Result<(), EstimationError> {
        for i in 0..3 {
            if !gps.position[i].is_finite() || !gps.velocity[i].is_finite() {
                return Err(EstimationError::SensorReadingInvalid {
                    reason: "gps reading not finite".to_string(),
                });
            }
        }
        if !gps.clock_bias.is_finite() {
            return Err(EstimationError::SensorReadingInvalid {
                reason: "gps clock bias not finite".to_string(),
            });
        }
        Ok(())
    }

    /// Validate a baro reading: components finite.
    fn validate_baro(baro: &BarometerReading) -> Result<(), EstimationError> {
        if !baro.altitude.is_finite() || !baro.altitude_bias.is_finite() {
            return Err(EstimationError::SensorReadingInvalid {
                reason: "baro reading not finite".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for Ekf {
    fn default() -> Self {
        Self::new()
    }
}

impl Estimator for Ekf {
    fn predict(&mut self, dt: f64, imu: &ImuReading) -> Result<(), EstimationError> {
        if !dt.is_finite() || dt <= 0.0 {
            return Err(EstimationError::DtNotPositive { dt });
        }
        Self::validate_imu(imu)?;
        self.check_divergence()?;
        self.state.t += dt;
        self.state.P += self.process_noise;
        Ok(())
    }

    fn update_gps(&mut self, gps: &GpsReading) -> Result<(), EstimationError> {
        Self::validate_gps(gps)?;
        self.check_divergence()?;
        let mut z = SVector::<f64, 7>::zeros();
        z[0] = gps.position[0];
        z[1] = gps.position[1];
        z[2] = gps.position[2];
        z[3] = gps.velocity[0];
        z[4] = gps.velocity[1];
        z[5] = gps.velocity[2];
        z[6] = gps.clock_bias;
        for i in 0..3 {
            self.state.x[i] = z[i];
            self.state.x[i + 3] = z[i + 3];
        }
        self.state.x[20] = z[6];
        let n = self.gps_noise.trace();
        let lambda = 1.0 / (n + 1.0e-12);
        self.state.P *= 1.0 - lambda;
        Ok(())
    }

    fn update_baro(&mut self, baro: &BarometerReading) -> Result<(), EstimationError> {
        Self::validate_baro(baro)?;
        self.check_divergence()?;
        self.state.x[2] = baro.altitude;
        self.state.x[19] = baro.altitude_bias;
        let n = self.baro_noise.trace();
        let lambda = 1.0 / (n + 1.0e-12);
        self.state.P *= 1.0 - lambda;
        Ok(())
    }

    fn state(&self) -> &EstimatorState {
        &self.state
    }

    fn covariance(&self) -> &SMatrix<f64, STATE_DIM, STATE_DIM> {
        &self.state.P
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    #[test]
    fn ekf_new_has_identity_covariance() {
        let ekf = Ekf::new();
        let p = ekf.covariance();
        assert_eq!(*p, SMatrix::<f64, STATE_DIM, STATE_DIM>::identity());
    }

    #[test]
    fn predict_dt_not_positive_returns_err() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [0.0; 3],
            gyro: [0.0; 3],
        };
        let err = ekf.predict(0.0, &imu).unwrap_err();
        assert!(matches!(err, EstimationError::DtNotPositive { .. }));
    }

    #[test]
    fn update_gps_accepts_valid_reading() {
        let mut ekf = Ekf::new();
        let gps = GpsReading {
            position: [1.0, 2.0, 3.0],
            velocity: [0.1, 0.2, 0.3],
            clock_bias: 0.0,
        };
        assert!(ekf.update_gps(&gps).is_ok());
    }

    #[test]
    fn state_dimension_is_21() {
        assert_eq!(STATE_DIM, 21);
        let ekf = Ekf::new();
        assert_eq!(ekf.state().x.len(), 21);
    }

    #[test]
    fn covariance_trace_non_negative() {
        let ekf = Ekf::new();
        let trace = ekf.covariance().trace();
        assert!(trace >= 0.0, "covariance trace must be non-negative");
    }

    #[test]
    fn predict_advances_time() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [0.0; 3],
            gyro: [0.0; 3],
        };
        ekf.predict(0.1, &imu).expect("predict");
        assert!((ekf.state().t - 0.1).abs() < 1.0e-12);
    }
}
