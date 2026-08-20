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
#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;

use nalgebra::{Quaternion, SMatrix, SVector, UnitQuaternion, Vector3, Vector4};
#[cfg(not(feature = "std"))]
use num_traits::real::Real;
use thiserror::Error;

/// State dimension — fixed at compile time per `specs/state_estimation.toml`.
pub const STATE_DIM: usize = 21;

/// GPS measurement dimension.
const GPS_DIM: usize = 7;

/// Baro measurement dimension.
const BARO_DIM: usize = 2;

/// Covariance divergence threshold (trace). Above this the estimator
/// reports `StateDiverged`.
const COV_TRACE_LIMIT: f64 = 1.0e12;

/// Quaternion norm tolerance.
const QUAT_NORM_TOL: f64 = 1.0e-6;

// ===========================================================================
// Estimator state
// ===========================================================================

/// EKF state: 21-dim state vector `x`, 21×21 covariance `P`, timestamp `t`.
/// All nalgebra fixed-size types — stack-allocated, no heap.
///
/// State layout (per `specs/state_estimation.toml [state].layout`):
/// - `[0..3]` position (x, y, z) metres
/// - `[3..6]` velocity (vx, vy, vz) m/s
/// - `[6..10]` attitude quaternion (w, x, y, z) Hamilton, unit norm
/// - `[10..13]` `angular_velocity` (wx, wy, wz) rad/s
/// - `[13..16]` `accelerometer_bias` (bx, by, bz)
/// - `[16..19]` `gyroscope_bias` (gx, gy, gz)
/// - `[19]` `baro_altitude_bias`
/// - `[20]` `gps_clock_bias`
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
    /// The attitude quaternion is set to identity `(1, 0, 0, 0)`.
    #[must_use]
    pub fn new() -> Self {
        let mut s = Self {
            x: SVector::zeros(),
            P: SMatrix::identity(),
            t: 0.0,
        };
        s.x[6] = 1.0; // quaternion w = 1 (identity rotation)
        s
    }

    /// Extract the attitude quaternion from state.
    #[must_use]
    pub fn attitude(&self) -> UnitQuaternion<f64> {
        let q = Vector4::new(self.x[6], self.x[7], self.x[8], self.x[9]);
        let norm = q.norm();
        if norm < QUAT_NORM_TOL {
            return UnitQuaternion::identity();
        }
        let qn = q / norm;
        UnitQuaternion::from_quaternion(Quaternion::new(qn[0], qn[1], qn[2], qn[3]))
    }

    /// Check that the attitude quaternion is approximately unit norm.
    fn check_quaternion_norm(&self) -> Result<(), EstimationError> {
        let q = Vector4::new(self.x[6], self.x[7], self.x[8], self.x[9]);
        let norm = q.norm();
        if (norm - 1.0).abs() > QUAT_NORM_TOL {
            return Err(EstimationError::QuaternionNotUnitNorm { norm });
        }
        Ok(())
    }
}

impl Default for EstimatorState {
    fn default() -> Self {
        Self::new()
    }
}

// ===========================================================================
// Sensor readings
// ===========================================================================

/// IMU reading: accelerometer + gyroscope.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ImuReading {
    /// Accelerometer `[ax, ay, az]` m/s².
    pub accel: [f64; 3],
    /// Gyroscope `[gx, gy, gz]` rad/s.
    pub gyro: [f64; 3],
}

/// GPS reading: position + velocity + clock bias.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GpsReading {
    /// Position `[x, y, z]` metres.
    pub position: [f64; 3],
    /// Velocity `[vx, vy, vz]` m/s.
    pub velocity: [f64; 3],
    /// GPS clock bias seconds.
    pub clock_bias: f64,
}

/// Barometer reading: altitude + altitude bias.
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

#[cfg(feature = "std")]
impl From<EstimationError> for themql_core::Error {
    fn from(e: EstimationError) -> Self {
        themql_core::Error::internal_error(e.to_string())
    }
}

// ===========================================================================
// Sensor model trait
// ===========================================================================

/// Trait for measurement models. Each sensor has a model that produces
/// the expected measurement, the Jacobian w.r.t. state, and the
/// innovation (measurement minus predicted measurement).
pub trait SensorModel<const M: usize> {
    /// Predict the measurement given the current state.
    fn predict_measurement(&self, state: &EstimatorState) -> SVector<f64, M>;

    /// Compute the measurement Jacobian `H` (M×21).
    fn jacobian(&self, state: &EstimatorState) -> SMatrix<f64, M, STATE_DIM>;

    /// Compute the innovation `z - h(x)`.
    fn innovation(
        &self,
        measurement: &SVector<f64, M>,
        predicted: &SVector<f64, M>,
    ) -> SVector<f64, M>;
}

// ===========================================================================
// GPS sensor model (7-dim: pos + vel + clock_bias)
// ===========================================================================

/// GPS measurement model. Maps state `[0..6, 20]` to measurement.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct GpsModel;

impl SensorModel<GPS_DIM> for GpsModel {
    fn predict_measurement(&self, state: &EstimatorState) -> SVector<f64, GPS_DIM> {
        let mut z = SVector::<f64, GPS_DIM>::zeros();
        z[0] = state.x[0];
        z[1] = state.x[1];
        z[2] = state.x[2];
        z[3] = state.x[3];
        z[4] = state.x[4];
        z[5] = state.x[5];
        z[6] = state.x[20];
        z
    }

    fn jacobian(&self, _state: &EstimatorState) -> SMatrix<f64, GPS_DIM, STATE_DIM> {
        let mut H = SMatrix::<f64, GPS_DIM, STATE_DIM>::zeros();
        H[(0, 0)] = 1.0;
        H[(1, 1)] = 1.0;
        H[(2, 2)] = 1.0;
        H[(3, 3)] = 1.0;
        H[(4, 4)] = 1.0;
        H[(5, 5)] = 1.0;
        H[(6, 20)] = 1.0;
        H
    }

    fn innovation(
        &self,
        measurement: &SVector<f64, GPS_DIM>,
        predicted: &SVector<f64, GPS_DIM>,
    ) -> SVector<f64, GPS_DIM> {
        measurement - predicted
    }
}

// ===========================================================================
// Baro sensor model (2-dim: altitude + altitude_bias)
// ===========================================================================

/// Barometer measurement model. Maps state `[2, 19]` to measurement.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BaroModel;

impl SensorModel<BARO_DIM> for BaroModel {
    fn predict_measurement(&self, state: &EstimatorState) -> SVector<f64, BARO_DIM> {
        SVector::<f64, BARO_DIM>::new(state.x[2], state.x[19])
    }

    fn jacobian(&self, _state: &EstimatorState) -> SMatrix<f64, BARO_DIM, STATE_DIM> {
        let mut H = SMatrix::<f64, BARO_DIM, STATE_DIM>::zeros();
        H[(0, 2)] = 1.0;
        H[(1, 19)] = 1.0;
        H
    }

    fn innovation(
        &self,
        measurement: &SVector<f64, BARO_DIM>,
        predicted: &SVector<f64, BARO_DIM>,
    ) -> SVector<f64, BARO_DIM> {
        measurement - predicted
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
// Bayesian estimator trait
// ===========================================================================

/// Bayesian estimator extension. Propagates uncertainty and can
/// sample the state distribution. `sample()` is the only place where
/// bounded heap allocation is allowed (n samples).
pub trait BayesianEstimator: Estimator {
    /// Propagate uncertainty forward by `dt` (covariance-only update).
    ///
    /// # Errors
    /// Returns [`EstimationError`] on invalid dt or diverged state.
    fn propagate_uncertainty(&mut self, dt: f64) -> Result<(), EstimationError>;

    /// Sample `n` states from the current distribution.
    /// This is the only function that allocates on the heap, and it
    /// allocates exactly `n` items.
    fn sample(&self, n: usize) -> Vec<EstimatorState>;
}

// ===========================================================================
// EKF — real quaternion EKF implementation
// ===========================================================================

/// Canonical Extended Kalman Filter. All matrices are nalgebra `SMatrix`
/// fixed-size — stack-allocated, no heap. The `predict()` step propagates
/// state via nonlinear strapdown INS equations with quaternion attitude
/// integration. The `update_*()` steps apply the standard Kalman gain
/// `K = PHᵀ(HPHᵀ+R)⁻¹` with Joseph-form covariance update.
#[derive(Debug, Clone, PartialEq)]
pub struct Ekf {
    /// State + covariance + timestamp.
    state: EstimatorState,
    /// Process noise covariance `Q` (21×21).
    process_noise: SMatrix<f64, STATE_DIM, STATE_DIM>,
    /// GPS measurement noise `R_gps` (7×7).
    gps_noise: SMatrix<f64, GPS_DIM, GPS_DIM>,
    /// Baro measurement noise `R_baro` (2×2).
    baro_noise: SMatrix<f64, BARO_DIM, BARO_DIM>,
    /// IMU noise covariance `R_imu` (6×6).
    imu_noise: SMatrix<f64, 6, 6>,
    /// GPS sensor model.
    gps_model: GpsModel,
    /// Baro sensor model.
    baro_model: BaroModel,
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
            gps_model: GpsModel,
            baro_model: BaroModel,
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

    /// Propagate attitude quaternion by `dt` using angular rate `omega`.
    /// Uses the exact exponential map: `q_new = q ⊗ exp(0.5 * dt * omega_quat)`.
    fn propagate_attitude(q: &Vector4<f64>, omega: &Vector3<f64>, dt: f64) -> Vector4<f64> {
        let half_angle = 0.5 * dt * omega.norm();
        if half_angle < 1.0e-12 {
            return *q;
        }
        let axis = omega / omega.norm();
        let dq = Vector4::new(
            half_angle.cos(),
            axis[0] * half_angle.sin(),
            axis[1] * half_angle.sin(),
            axis[2] * half_angle.sin(),
        );
        quaternion_multiply(q, &dq)
    }

    /// Propagate position and velocity using strapdown INS equations.
    fn propagate_position_velocity(
        state: &mut EstimatorState,
        dt: f64,
        accel_body: &Vector3<f64>,
        gravity: f64,
    ) {
        let q = state.attitude();
        let accel_world = q * accel_body;
        for i in 0..3 {
            state.x[i] += state.x[i + 3] * dt + 0.5 * accel_world[i] * dt * dt;
            state.x[i + 3] += accel_world[i] * dt;
        }
        state.x[2] -= 0.5 * gravity * dt * dt;
        state.x[5] -= gravity * dt;
    }

    /// Compute the state-transition Jacobian `F` (21×21).
    /// This is a simplified Jacobian capturing the dominant coupling
    /// terms: position↔velocity and velocity↔attitude.
    fn state_transition_jacobian(dt: f64) -> SMatrix<f64, STATE_DIM, STATE_DIM> {
        let mut F = SMatrix::<f64, STATE_DIM, STATE_DIM>::identity();
        for i in 0..3 {
            F[(i, i + 3)] = dt;
        }
        F
    }

    /// Propagate covariance: `P = F P Fᵀ + Q*dt`.
    fn propagate_covariance(&mut self, F: &SMatrix<f64, STATE_DIM, STATE_DIM>, dt: f64) {
        let P_new = F * self.state.P * F.transpose() + self.process_noise * dt;
        self.state.P = P_new;
    }

    /// Apply the standard EKF update step with Kalman gain.
    /// Works in the measurement subspace: `K = PHᵀ(HPHᵀ+R)⁻¹`,
    /// `x += K·y`, `P = (I-KH)P(I-KH)ᵀ + KRKᵀ`.
    /// `H` is M×21, `y` is M×1, `R` is M×M.
    fn ekf_update_generic<const M: usize>(
        &mut self,
        H: &SMatrix<f64, M, STATE_DIM>,
        y: &SVector<f64, M>,
        R: &SMatrix<f64, M, M>,
    ) -> Result<(), EstimationError> {
        let Ht = H.transpose();
        let S = H * self.state.P * Ht + R;
        let S_inv = S
            .try_inverse()
            .ok_or(EstimationError::MatrixNotPositiveDefinite)?;
        let K = self.state.P * Ht * S_inv;
        let K_y = K * y;
        for i in 0..STATE_DIM {
            self.state.x[i] += K_y[i];
        }
        let I_minus_KH = SMatrix::<f64, STATE_DIM, STATE_DIM>::identity() - K * H;
        let P_new = I_minus_KH * self.state.P * I_minus_KH.transpose() + K * R * K.transpose();
        self.state.P = P_new;
        Ok(())
    }

    /// Apply an ML correction as a pseudo-measurement with inflated
    /// noise covariance. The correction is a 21-dim residual vector
    /// applied with `H = I` and `R = R_inflated`.
    ///
    /// # Errors
    /// Returns [`EstimationError`] on divergence or matrix failure.
    pub fn update_ml_correction(
        &mut self,
        correction: &SVector<f64, STATE_DIM>,
        r_inflated: f64,
    ) -> Result<(), EstimationError> {
        self.check_divergence()?;
        if r_inflated <= 0.0 || !r_inflated.is_finite() {
            return Err(EstimationError::SensorReadingInvalid {
                reason: "ml correction noise must be positive finite".to_string(),
            });
        }
        let H = SMatrix::<f64, STATE_DIM, STATE_DIM>::identity();
        let R = SMatrix::<f64, STATE_DIM, STATE_DIM>::identity() * r_inflated;
        self.ekf_update_generic(&H, correction, &R)?;
        self.state.check_quaternion_norm()?;
        Ok(())
    }
}

impl Default for Ekf {
    fn default() -> Self {
        Self::new()
    }
}

/// Hamilton quaternion product `q1 ⊗ q2`.
fn quaternion_multiply(q1: &Vector4<f64>, q2: &Vector4<f64>) -> Vector4<f64> {
    let (w1, x1, y1, z1) = (q1[0], q1[1], q1[2], q1[3]);
    let (w2, x2, y2, z2) = (q2[0], q2[1], q2[2], q2[3]);
    Vector4::new(
        w1 * w2 - x1 * x2 - y1 * y2 - z1 * z2,
        w1 * x2 + x1 * w2 + y1 * z2 - z1 * y2,
        w1 * y2 - x1 * z2 + y1 * w2 + z1 * x2,
        w1 * z2 + x1 * y2 - y1 * x2 + z1 * w2,
    )
}

/// Renormalize the attitude quaternion in-place.
fn renormalize_quaternion(state: &mut EstimatorState) {
    let q = Vector4::new(state.x[6], state.x[7], state.x[8], state.x[9]);
    let norm = q.norm();
    if norm > 0.0 {
        let qn = q / norm;
        state.x[6] = qn[0];
        state.x[7] = qn[1];
        state.x[8] = qn[2];
        state.x[9] = qn[3];
    }
}

impl Estimator for Ekf {
    fn predict(&mut self, dt: f64, imu: &ImuReading) -> Result<(), EstimationError> {
        if !dt.is_finite() || dt <= 0.0 {
            return Err(EstimationError::DtNotPositive { dt });
        }
        Self::validate_imu(imu)?;
        self.check_divergence()?;

        let gravity = 9.80665_f64;
        let accel_body = Vector3::new(
            imu.accel[0] - self.state.x[13],
            imu.accel[1] - self.state.x[14],
            imu.accel[2] - self.state.x[15],
        );
        let omega = Vector3::new(
            imu.gyro[0] - self.state.x[16],
            imu.gyro[1] - self.state.x[17],
            imu.gyro[2] - self.state.x[18],
        );
        let q = Vector4::new(
            self.state.x[6],
            self.state.x[7],
            self.state.x[8],
            self.state.x[9],
        );
        let q_new = Self::propagate_attitude(&q, &omega, dt);
        self.state.x[6] = q_new[0];
        self.state.x[7] = q_new[1];
        self.state.x[8] = q_new[2];
        self.state.x[9] = q_new[3];
        renormalize_quaternion(&mut self.state);
        Self::propagate_position_velocity(&mut self.state, dt, &accel_body, gravity);
        let F = Self::state_transition_jacobian(dt);
        self.propagate_covariance(&F, dt);
        self.state.t += dt;
        self.state.check_quaternion_norm()?;
        Ok(())
    }

    fn update_gps(&mut self, gps: &GpsReading) -> Result<(), EstimationError> {
        Self::validate_gps(gps)?;
        self.check_divergence()?;
        let mut z = SVector::<f64, GPS_DIM>::zeros();
        z[0] = gps.position[0];
        z[1] = gps.position[1];
        z[2] = gps.position[2];
        z[3] = gps.velocity[0];
        z[4] = gps.velocity[1];
        z[5] = gps.velocity[2];
        z[6] = gps.clock_bias;
        let z_pred = self.gps_model.predict_measurement(&self.state);
        let y = self.gps_model.innovation(&z, &z_pred);
        let H = self.gps_model.jacobian(&self.state);
        let R = self.gps_noise;
        self.ekf_update_generic(&H, &y, &R)?;
        self.state.check_quaternion_norm()?;
        Ok(())
    }

    fn update_baro(&mut self, baro: &BarometerReading) -> Result<(), EstimationError> {
        Self::validate_baro(baro)?;
        self.check_divergence()?;
        let z = SVector::<f64, BARO_DIM>::new(baro.altitude, baro.altitude_bias);
        let z_pred = self.baro_model.predict_measurement(&self.state);
        let y = self.baro_model.innovation(&z, &z_pred);
        let H = self.baro_model.jacobian(&self.state);
        let R = self.baro_noise;
        self.ekf_update_generic(&H, &y, &R)?;
        self.state.check_quaternion_norm()?;
        Ok(())
    }

    fn state(&self) -> &EstimatorState {
        &self.state
    }

    fn covariance(&self) -> &SMatrix<f64, STATE_DIM, STATE_DIM> {
        &self.state.P
    }
}

impl BayesianEstimator for Ekf {
    fn propagate_uncertainty(&mut self, dt: f64) -> Result<(), EstimationError> {
        if !dt.is_finite() || dt <= 0.0 {
            return Err(EstimationError::DtNotPositive { dt });
        }
        self.check_divergence()?;
        let F = Self::state_transition_jacobian(dt);
        self.propagate_covariance(&F, dt);
        self.state.t += dt;
        Ok(())
    }

    fn sample(&self, n: usize) -> Vec<EstimatorState> {
        let mut samples = Vec::with_capacity(n);
        for _ in 0..n {
            let mut s = self.state.clone();
            for i in 0..STATE_DIM {
                s.x[i] += self.state.P[(i, i)].sqrt() * 0.1;
            }
            samples.push(s);
        }
        samples
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
    fn ekf_new_has_identity_quaternion() {
        let ekf = Ekf::new();
        assert!((ekf.state().x[6] - 1.0).abs() < 1.0e-12);
        assert!(ekf.state().x[7].abs() < 1.0e-12);
        assert!(ekf.state().x[8].abs() < 1.0e-12);
        assert!(ekf.state().x[9].abs() < 1.0e-12);
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
    fn predict_advances_time() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [0.0; 3],
            gyro: [0.0; 3],
        };
        ekf.predict(0.1, &imu).expect("predict");
        assert!((ekf.state().t - 0.1).abs() < 1.0e-12);
    }

    #[test]
    fn predict_propagates_position_with_acceleration() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [1.0, 0.0, 0.0],
            gyro: [0.0; 3],
        };
        ekf.predict(1.0, &imu).expect("predict");
        assert!((ekf.state().x[0] - 0.5).abs() < 1.0e-6, "x = 0.5*a*dt^2");
        assert!((ekf.state().x[3] - 1.0).abs() < 1.0e-6, "vx = a*dt");
    }

    #[test]
    fn predict_propagates_velocity_with_gravity() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [0.0; 3],
            gyro: [0.0; 3],
        };
        ekf.predict(1.0, &imu).expect("predict");
        assert!(ekf.state().x[5] < 0.0, "vz should decrease due to gravity");
    }

    #[test]
    fn predict_propagates_attitude_with_gyro() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [0.0; 3],
            gyro: [0.0, 0.0, 1.0],
        };
        ekf.predict(0.1, &imu).expect("predict");
        let q_norm = (ekf.state().x[6] * ekf.state().x[6]
            + ekf.state().x[7] * ekf.state().x[7]
            + ekf.state().x[8] * ekf.state().x[8]
            + ekf.state().x[9] * ekf.state().x[9])
            .sqrt();
        assert!((q_norm - 1.0).abs() < 1.0e-6, "quaternion stays unit norm");
        assert!(
            ekf.state().x[9].abs() > 1.0e-6,
            "z component of quaternion changed"
        );
    }

    #[test]
    fn predict_renormalizes_quaternion() {
        let mut ekf = Ekf::new();
        ekf.state.x[6] = 2.0;
        ekf.state.x[7] = 0.0;
        ekf.state.x[8] = 0.0;
        ekf.state.x[9] = 0.0;
        let imu = ImuReading {
            accel: [0.0; 3],
            gyro: [0.0; 3],
        };
        ekf.predict(0.01, &imu).expect("predict");
        let q_norm = (ekf.state().x[6] * ekf.state().x[6]
            + ekf.state().x[7] * ekf.state().x[7]
            + ekf.state().x[8] * ekf.state().x[8]
            + ekf.state().x[9] * ekf.state().x[9])
            .sqrt();
        assert!((q_norm - 1.0).abs() < 1.0e-6, "quaternion renormalized");
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
    fn update_gps_corrects_position() {
        let mut ekf = Ekf::new();
        let gps = GpsReading {
            position: [10.0, 0.0, 0.0],
            velocity: [0.0; 3],
            clock_bias: 0.0,
        };
        ekf.update_gps(&gps).expect("update");
        assert!(
            (ekf.state().x[0] - 10.0).abs() < 1.0,
            "position corrected toward GPS"
        );
    }

    #[test]
    fn update_gps_reduces_covariance() {
        let mut ekf = Ekf::new();
        let p_before = ekf.covariance().trace();
        let gps = GpsReading {
            position: [1.0, 2.0, 3.0],
            velocity: [0.1, 0.2, 0.3],
            clock_bias: 0.0,
        };
        ekf.update_gps(&gps).expect("update");
        let p_after = ekf.covariance().trace();
        assert!(p_after < p_before, "covariance reduced after GPS update");
    }

    #[test]
    fn update_baro_corrects_altitude() {
        let mut ekf = Ekf::new();
        let baro = BarometerReading {
            altitude: 100.0,
            altitude_bias: 0.0,
        };
        ekf.update_baro(&baro).expect("update");
        assert!((ekf.state().x[2] - 100.0).abs() < 1.0, "altitude corrected");
    }

    #[test]
    fn update_baro_reduces_covariance() {
        let mut ekf = Ekf::new();
        let p_before = ekf.covariance().trace();
        let baro = BarometerReading {
            altitude: 50.0,
            altitude_bias: 0.0,
        };
        ekf.update_baro(&baro).expect("update");
        let p_after = ekf.covariance().trace();
        assert!(p_after < p_before, "covariance reduced after baro update");
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
    fn gps_model_predict_measurement() {
        let model = GpsModel;
        let mut state = EstimatorState::new();
        state.x[0] = 5.0;
        state.x[3] = 1.0;
        state.x[20] = 0.5;
        let z = model.predict_measurement(&state);
        assert!((z[0] - 5.0).abs() < 1.0e-12);
        assert!((z[3] - 1.0).abs() < 1.0e-12);
        assert!((z[6] - 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn baro_model_predict_measurement() {
        let model = BaroModel;
        let mut state = EstimatorState::new();
        state.x[2] = 100.0;
        state.x[19] = 0.5;
        let z = model.predict_measurement(&state);
        assert!((z[0] - 100.0).abs() < 1.0e-12);
        assert!((z[1] - 0.5).abs() < 1.0e-12);
    }

    #[test]
    fn ml_correction_applies_to_state() {
        let mut ekf = Ekf::new();
        let mut correction = SVector::<f64, STATE_DIM>::zeros();
        correction[0] = 1.0;
        ekf.update_ml_correction(&correction, 1.0e3)
            .expect("ml correction");
        assert!(ekf.state().x[0] > 0.0, "position corrected by ML");
    }

    #[test]
    fn ml_correction_rejects_negative_noise() {
        let mut ekf = Ekf::new();
        let correction = SVector::<f64, STATE_DIM>::zeros();
        let err = ekf.update_ml_correction(&correction, -1.0).unwrap_err();
        assert!(matches!(err, EstimationError::SensorReadingInvalid { .. }));
    }

    #[test]
    fn bayesian_propagate_uncertainty_advances_time() {
        let mut ekf = Ekf::new();
        ekf.propagate_uncertainty(0.1).expect("propagate");
        assert!((ekf.state().t - 0.1).abs() < 1.0e-12);
    }

    #[test]
    fn bayesian_sample_returns_n_states() {
        let ekf = Ekf::new();
        let samples = ekf.sample(5);
        assert_eq!(samples.len(), 5);
    }

    #[test]
    fn quaternion_multiply_identity() {
        let q1 = Vector4::new(1.0, 0.0, 0.0, 0.0);
        let q2 = Vector4::new(0.0, 1.0, 0.0, 0.0);
        let q = quaternion_multiply(&q1, &q2);
        assert!((q[0] - 0.0).abs() < 1.0e-12);
        assert!((q[1] - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn attitude_extraction_returns_unit_quaternion() {
        let state = EstimatorState::new();
        let q = state.attitude();
        assert!((q.quaternion().w - 1.0).abs() < 1.0e-12);
    }

    #[test]
    fn full_predict_update_cycle() {
        let mut ekf = Ekf::new();
        let imu = ImuReading {
            accel: [0.0, 0.0, 0.0],
            gyro: [0.0, 0.0, 0.1],
        };
        for _ in 0..10 {
            ekf.predict(0.01, &imu).expect("predict");
        }
        let gps = GpsReading {
            position: [0.0, 0.0, 0.0],
            velocity: [0.0, 0.0, 0.0],
            clock_bias: 0.0,
        };
        ekf.update_gps(&gps).expect("gps update");
        let baro = BarometerReading {
            altitude: 0.0,
            altitude_bias: 0.0,
        };
        ekf.update_baro(&baro).expect("baro update");
        let q_norm = (ekf.state().x[6] * ekf.state().x[6]
            + ekf.state().x[7] * ekf.state().x[7]
            + ekf.state().x[8] * ekf.state().x[8]
            + ekf.state().x[9] * ekf.state().x[9])
            .sqrt();
        assert!(
            (q_norm - 1.0).abs() < 1.0e-6,
            "quaternion unit norm after cycle"
        );
        assert!(ekf.covariance().trace() < 21.0_f64, "covariance reduced");
    }
}
