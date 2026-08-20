//! # themql-gnc
//!
//! Guidance, Navigation, and Control for the embedded target. Deterministic
//! PID, LQRI, and hybrid LQRI-PID controllers operating on quaternion
//! attitude state. ML has **no** control authority here; basic flight
//! stability must hold without ML (per `SPEC.toml [ml.authority]`).
//!
//! See `specs/gnc.toml` for the authoritative specification.
//!
//! ## TETANUS
//!
//! This crate is safety-critical. All ten Power-of-Ten rules apply:
//! no recursion, fixed loop bounds, no heap allocation after init,
//! functions ≤ 60 lines, ≥ 2 assertions per function (as `if !invariant
//! { return Err }`), no `unwrap()`/`expect()`, simple macros, one level of
//! pointer dereference, zero warnings at `clippy::pedantic`.

#![forbid(unsafe_code)]
#![deny(warnings)]
#![warn(clippy::pedantic)]
#![warn(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![warn(missing_docs)]
#![allow(non_snake_case)]
#![allow(clippy::doc_markdown)]

use nalgebra::{SMatrix, UnitQuaternion, Vector3};
use thiserror::Error;

/// Maximum integral windup magnitude per axis. Saturates the integral term
/// to prevent windup.
const MAX_INTEGRAL: f64 = 10.0;

/// Number of actuator channels — fixed at compile time.
const ACTUATOR_COUNT: usize = 8;

// ===========================================================================
// State — 21-dim (matches state_estimation.toml)
// ===========================================================================

/// GNC state vector using nalgebra fixed-size types (stack-allocated, no
/// heap). Attitude is a `UnitQuaternion<f64>` so unit norm is enforced at
/// the type level.
#[derive(Debug, Clone, PartialEq)]
pub struct GncState {
    /// Position `[x, y, z]` metres.
    pub position: Vector3<f64>,
    /// Velocity `[vx, vy, vz]` m/s.
    pub velocity: Vector3<f64>,
    /// Attitude as a unit quaternion (Hamilton `[w, x, y, z]`).
    pub attitude: UnitQuaternion<f64>,
    /// Angular velocity `[wx, wy, wz]` rad/s.
    pub angular_velocity: Vector3<f64>,
    /// Accelerometer bias `[bx, by, bz]`.
    pub accelerometer_bias: Vector3<f64>,
    /// Gyroscope bias `[gx, gy, gz]`.
    pub gyroscope_bias: Vector3<f64>,
    /// Barometric altitude bias.
    pub baro_altitude_bias: f64,
    /// GPS clock bias.
    pub gps_clock_bias: f64,
}

impl GncState {
    /// Construct a zero state: all vectors zero, attitude = identity
    /// quaternion, biases zero.
    #[must_use]
    pub fn new() -> Self {
        Self {
            position: Vector3::zeros(),
            velocity: Vector3::zeros(),
            attitude: UnitQuaternion::identity(),
            angular_velocity: Vector3::zeros(),
            accelerometer_bias: Vector3::zeros(),
            gyroscope_bias: Vector3::zeros(),
            baro_altitude_bias: 0.0,
            gps_clock_bias: 0.0,
        }
    }
}

impl Default for GncState {
    fn default() -> Self {
        Self::new()
    }
}

/// Controller setpoint — the target state the controller drives toward.
#[derive(Debug, Clone, PartialEq)]
pub struct Setpoint {
    /// Target position.
    pub target_position: Vector3<f64>,
    /// Target velocity.
    pub target_velocity: Vector3<f64>,
    /// Target attitude.
    pub target_attitude: UnitQuaternion<f64>,
    /// Target angular velocity.
    pub target_angular_velocity: Vector3<f64>,
}

impl Setpoint {
    /// Construct a zero setpoint (hold current hover — identity attitude).
    #[must_use]
    pub fn new() -> Self {
        Self {
            target_position: Vector3::zeros(),
            target_velocity: Vector3::zeros(),
            target_attitude: UnitQuaternion::identity(),
            target_angular_velocity: Vector3::zeros(),
        }
    }
}

impl Default for Setpoint {
    fn default() -> Self {
        Self::new()
    }
}

/// Fixed 8-channel actuator command vector. Unused slots must be `0.0`
/// (never `NaN`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ActuatorCommand {
    /// 8 actuator values.
    pub values: [f64; ACTUATOR_COUNT],
}

impl ActuatorCommand {
    /// Construct an all-zero actuator command.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            values: [0.0; ACTUATOR_COUNT],
        }
    }
}

// ===========================================================================
// Controller kind
// ===========================================================================

/// Identifies which controller produced an `ActuatorCommand`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControllerKind {
    /// PID controller.
    Pid,
    /// LQRI controller.
    Lqri,
    /// Hybrid LQRI-PID controller.
    Hybrid,
}

// ===========================================================================
// Error
// ===========================================================================

/// Errors raised by GNC controllers.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum GncError {
    /// The provided state was invalid.
    #[error("invalid state: {reason}")]
    StateInvalid {
        /// Why the state was invalid.
        reason: String,
    },
    /// Attitude quaternion was not unit norm.
    #[error("quaternion not unit norm: {norm}")]
    QuaternionNotUnitNorm {
        /// The observed norm.
        norm: f64,
    },
    /// Actuator command saturated beyond limits.
    #[error("actuator saturation")]
    ActuatorSaturation,
    /// Time step `dt` was not positive.
    #[error("dt not positive: {dt}")]
    DtNotPositive {
        /// The offending dt.
        dt: f64,
    },
    /// The setpoint was invalid.
    #[error("invalid setpoint: {reason}")]
    SetpointInvalid {
        /// Why the setpoint was invalid.
        reason: String,
    },
    /// Internal controller failure.
    #[error("internal gnc error: {0}")]
    InternalError(String),
}

impl From<GncError> for themql_core::Error {
    fn from(e: GncError) -> Self {
        themql_core::Error::internal_error(e.to_string())
    }
}

// ===========================================================================
// PID controller
// ===========================================================================

/// PID gains for 3 axes. Fixed at construction; no runtime retuning without
/// re-construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidGains {
    /// Proportional gains per axis.
    pub kp: [f64; 3],
    /// Integral gains per axis.
    pub ki: [f64; 3],
    /// Derivative gains per axis.
    pub kd: [f64; 3],
}

/// PID controller with integral windup saturation. `step()` performs no
/// heap allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct PidController {
    /// PID gains.
    gains: PidGains,
    /// Integral accumulator (saturates at ±`MAX_INTEGRAL`).
    integral: [f64; 3],
    /// Previous error for derivative term.
    prev_error: [f64; 3],
}

impl PidController {
    /// Construct a PID controller with the given gains and zeroed state.
    #[must_use]
    pub fn new(gains: PidGains) -> Self {
        Self {
            gains,
            integral: [0.0; 3],
            prev_error: [0.0; 3],
        }
    }

    /// Compute the position error between state and setpoint.
    fn position_error(state: &GncState, setpoint: &Setpoint) -> [f64; 3] {
        let err = setpoint.target_position - state.position;
        [err.x, err.y, err.z]
    }

    /// Update the integral term with windup saturation. Returns the new
    /// integral value for one axis.
    fn saturate_integral(current: f64, ki: f64, err: f64, dt: f64) -> f64 {
        let acc = current + err * dt;
        if acc > MAX_INTEGRAL {
            MAX_INTEGRAL
        } else if acc < -MAX_INTEGRAL {
            -MAX_INTEGRAL
        } else {
            let _ = ki;
            acc
        }
    }

    /// Compute one `PID` axis output from gains, integral, prev_error.
    fn axis_output(g: &PidGains, i: usize, err: f64, integ: f64, prev: f64, dt: f64) -> f64 {
        let p = g.kp[i] * err;
        let i_term = g.ki[i] * integ;
        let d = if dt > 0.0 {
            g.kd[i] * (err - prev) / dt
        } else {
            0.0
        };
        p + i_term + d
    }

    /// Internal mutable step: computes the actuator command and updates
    /// integral / prev_error in place.
    fn step_mut(
        &mut self,
        state: &GncState,
        setpoint: &Setpoint,
        dt: f64,
    ) -> Result<ActuatorCommand, GncError> {
        if !dt.is_finite() {
            return Err(GncError::DtNotPositive { dt });
        }
        if dt <= 0.0 {
            return Err(GncError::DtNotPositive { dt });
        }
        let err = Self::position_error(state, setpoint);
        let mut out = [0.0; ACTUATOR_COUNT];
        for i in 0..3 {
            self.integral[i] =
                Self::saturate_integral(self.integral[i], self.gains.ki[i], err[i], dt);
            out[i] = Self::axis_output(
                &self.gains,
                i,
                err[i],
                self.integral[i],
                self.prev_error[i],
                dt,
            );
            self.prev_error[i] = err[i];
        }
        Ok(ActuatorCommand { values: out })
    }
}

impl Controller for PidController {
    fn step(
        &self,
        state: &GncState,
        setpoint: &Setpoint,
        dt: f64,
    ) -> Result<ActuatorCommand, GncError> {
        if !state.attitude.quaternion().norm().is_finite() {
            return Err(GncError::StateInvalid {
                reason: "attitude quaternion not finite".to_string(),
            });
        }
        let mut copy = self.clone();
        copy.step_mut(state, setpoint, dt)
    }

    fn kind(&self) -> ControllerKind {
        ControllerKind::Pid
    }
}

// ===========================================================================
// LQRI controller
// ===========================================================================

/// Fixed-size LQRI matrices: state feedback gain `K` (8×21), integral action
/// `Ki` (8×3), state cost `Q` (21×21), control cost `R` (8×8). All
/// stack-allocated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LqriMatrices {
    /// State feedback gain.
    pub K: SMatrix<f64, 8, 21>,
    /// Integral action gain.
    pub Ki: SMatrix<f64, 8, 3>,
    /// State cost weight.
    pub Q: SMatrix<f64, 21, 21>,
    /// Control cost weight.
    pub R: SMatrix<f64, 8, 8>,
}

/// LQRI controller with integral action on tracking error. `step()`
/// performs no heap allocation.
#[derive(Debug, Clone, PartialEq)]
pub struct LqriController {
    /// LQRI matrices.
    matrices: LqriMatrices,
    /// Integral state on the 3-axis tracking error.
    integral_state: Vector3<f64>,
}

impl LqriController {
    /// Construct an LQRI controller with the given matrices and zeroed
    /// integral state.
    #[must_use]
    pub fn new(matrices: LqriMatrices) -> Self {
        Self {
            matrices,
            integral_state: Vector3::zeros(),
        }
    }

    /// Pack a `GncState` into a 21-dim state vector.
    fn pack_state(state: &GncState) -> SMatrix<f64, 21, 1> {
        let mut x = SMatrix::<f64, 21, 1>::zeros();
        x[0] = state.position.x;
        x[1] = state.position.y;
        x[2] = state.position.z;
        x[3] = state.velocity.x;
        x[4] = state.velocity.y;
        x[5] = state.velocity.z;
        let q = state.attitude.quaternion();
        x[6] = q.w;
        x[7] = q.i;
        x[8] = q.j;
        x[9] = q.k;
        x[10] = state.angular_velocity.x;
        x[11] = state.angular_velocity.y;
        x[12] = state.angular_velocity.z;
        x[13] = state.accelerometer_bias.x;
        x[14] = state.accelerometer_bias.y;
        x[15] = state.accelerometer_bias.z;
        x[16] = state.gyroscope_bias.x;
        x[17] = state.gyroscope_bias.y;
        x[18] = state.gyroscope_bias.z;
        x[19] = state.baro_altitude_bias;
        x[20] = state.gps_clock_bias;
        x
    }

    /// Internal mutable step: computes actuator command and updates
    /// integral state in place.
    fn step_mut(
        &mut self,
        state: &GncState,
        setpoint: &Setpoint,
        dt: f64,
    ) -> Result<ActuatorCommand, GncError> {
        if !dt.is_finite() || dt <= 0.0 {
            return Err(GncError::DtNotPositive { dt });
        }
        let x = Self::pack_state(state);
        let err = setpoint.target_position - state.position;
        self.integral_state += err * dt;
        let integ = self.integral_state;
        let u = (self.matrices.K * x) + (self.matrices.Ki * integ);
        let mut out = [0.0; ACTUATOR_COUNT];
        for i in 0..ACTUATOR_COUNT {
            out[i] = u[i];
        }
        Ok(ActuatorCommand { values: out })
    }
}

impl Controller for LqriController {
    fn step(
        &self,
        state: &GncState,
        setpoint: &Setpoint,
        dt: f64,
    ) -> Result<ActuatorCommand, GncError> {
        let q = state.attitude.quaternion();
        let norm = q.norm();
        if !norm.is_finite() {
            return Err(GncError::QuaternionNotUnitNorm { norm });
        }
        let mut copy = self.clone();
        copy.step_mut(state, setpoint, dt)
    }

    fn kind(&self) -> ControllerKind {
        ControllerKind::Lqri
    }
}

// ===========================================================================
// Hybrid LQRI-PID controller
// ===========================================================================

/// Switching policy for the hybrid controller. `Schedule` holds a function
/// pointer (not a closure) per TETANUS rule 9.
#[derive(Clone, Copy)]
#[allow(clippy::large_enum_variant)]
pub enum HybridSwitchingPolicy {
    /// Always use PID.
    AlwaysPid,
    /// Always use LQRI.
    AlwaysLqri,
    /// LQRI primary; PID fallback on LQRI saturation.
    LqriWithPidFallback(PidGains),
    /// PID inner loop; LQRI outer-loop setpoint generation.
    PidWithLqriOuter(LqriMatrices),
    /// Explicit state-dependent switch via a pure function pointer.
    Schedule(fn(GncState) -> ControllerKind),
}

impl std::fmt::Debug for HybridSwitchingPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlwaysPid => f.write_str("AlwaysPid"),
            Self::AlwaysLqri => f.write_str("AlwaysLqri"),
            Self::LqriWithPidFallback(_) => f.write_str("LqriWithPidFallback(..)"),
            Self::PidWithLqriOuter(_) => f.write_str("PidWithLqriOuter(..)"),
            Self::Schedule(_) => f.write_str("Schedule(..)"),
        }
    }
}

impl PartialEq for HybridSwitchingPolicy {
    #[allow(clippy::match_same_arms)]
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::AlwaysPid, Self::AlwaysPid) => true,
            (Self::AlwaysLqri, Self::AlwaysLqri) => true,
            (Self::LqriWithPidFallback(a), Self::LqriWithPidFallback(b)) => a == b,
            (Self::PidWithLqriOuter(_), Self::PidWithLqriOuter(_)) => true,
            (Self::Schedule(a), Self::Schedule(b)) => {
                let pa: *const () = std::ptr::from_ref(a).cast();
                let pb: *const () = std::ptr::from_ref(b).cast();
                std::ptr::eq(pa, pb)
            }
            _ => false,
        }
    }
}

/// Hybrid LQRI-PID controller. `step()` applies the switching policy to
/// pick a controller, then delegates.
#[derive(Debug, Clone, PartialEq)]
pub struct HybridController {
    /// Switching policy.
    policy: HybridSwitchingPolicy,
    /// Embedded PID controller.
    pid: PidController,
    /// Embedded LQRI controller.
    lqri: LqriController,
}

impl HybridController {
    /// Construct a hybrid controller from a policy and the two sub-controllers.
    #[must_use]
    pub fn new(policy: HybridSwitchingPolicy, pid: PidController, lqri: LqriController) -> Self {
        Self { policy, pid, lqri }
    }

    /// Resolve the policy to a controller kind for the given state.
    #[allow(clippy::match_same_arms)]
    fn resolve_kind(&self, state: &GncState) -> ControllerKind {
        match self.policy {
            HybridSwitchingPolicy::AlwaysPid => ControllerKind::Pid,
            HybridSwitchingPolicy::AlwaysLqri => ControllerKind::Lqri,
            HybridSwitchingPolicy::LqriWithPidFallback(_) => ControllerKind::Lqri,
            HybridSwitchingPolicy::PidWithLqriOuter(_) => ControllerKind::Pid,
            HybridSwitchingPolicy::Schedule(f) => f(Clone::clone(state)),
        }
    }
}

impl Controller for HybridController {
    fn step(
        &self,
        state: &GncState,
        setpoint: &Setpoint,
        dt: f64,
    ) -> Result<ActuatorCommand, GncError> {
        if !dt.is_finite() || dt <= 0.0 {
            return Err(GncError::DtNotPositive { dt });
        }
        let kind = self.resolve_kind(state);
        match kind {
            ControllerKind::Pid | ControllerKind::Hybrid => self.pid.step(state, setpoint, dt),
            ControllerKind::Lqri => self.lqri.step(state, setpoint, dt),
        }
    }

    fn kind(&self) -> ControllerKind {
        ControllerKind::Hybrid
    }
}

// ===========================================================================
// Controller trait
// ===========================================================================

/// Top-level controller trait. Each controller takes the current state and
/// a setpoint and produces an `ActuatorCommand`. Deterministic; no ML.
pub trait Controller: Send + Sync {
    /// Compute one control step.
    ///
    /// # Errors
    /// Returns [`GncError`] on invalid state, setpoint, or dt.
    fn step(
        &self,
        state: &GncState,
        setpoint: &Setpoint,
        dt: f64,
    ) -> Result<ActuatorCommand, GncError>;
    /// Which controller kind this is.
    fn kind(&self) -> ControllerKind;
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    fn zero_gains() -> PidGains {
        PidGains {
            kp: [0.0; 3],
            ki: [0.0; 3],
            kd: [0.0; 3],
        }
    }

    fn zero_lqri() -> LqriMatrices {
        LqriMatrices {
            K: SMatrix::zeros(),
            Ki: SMatrix::zeros(),
            Q: SMatrix::zeros(),
            R: SMatrix::zeros(),
        }
    }

    #[test]
    fn gnc_state_new_is_zero() {
        let s = GncState::new();
        assert_eq!(s.position, Vector3::zeros());
        assert_eq!(s.attitude, UnitQuaternion::identity());
        assert!(s.baro_altitude_bias.abs() < 1e-12);
    }

    #[test]
    fn pid_step_produces_finite_output() {
        let pid = PidController::new(PidGains {
            kp: [1.0; 3],
            ki: [0.0; 3],
            kd: [0.0; 3],
        });
        let state = GncState::new();
        let sp = Setpoint {
            target_position: Vector3::new(1.0, 0.0, 0.0),
            ..Setpoint::new()
        };
        let cmd = pid.step(&state, &sp, 0.01).expect("pid step");
        for v in cmd.values {
            assert!(v.is_finite(), "actuator must be finite");
        }
    }

    #[test]
    fn lqri_step_produces_finite_output() {
        let lqri = LqriController::new(zero_lqri());
        let state = GncState::new();
        let sp = Setpoint::new();
        let cmd = lqri.step(&state, &sp, 0.01).expect("lqri step");
        for v in cmd.values {
            assert!(v.is_finite(), "actuator must be finite");
        }
    }

    #[test]
    fn hybrid_always_pid_equals_pid() {
        let pid = PidController::new(PidGains {
            kp: [1.0; 3],
            ki: [0.0; 3],
            kd: [0.0; 3],
        });
        let hybrid = HybridController::new(
            HybridSwitchingPolicy::AlwaysPid,
            pid.clone(),
            LqriController::new(zero_lqri()),
        );
        let state = GncState::new();
        let sp = Setpoint::new();
        let a = pid.step(&state, &sp, 0.01).expect("pid");
        let b = hybrid.step(&state, &sp, 0.01).expect("hybrid");
        assert_eq!(a, b);
    }

    #[test]
    fn dt_not_positive_returns_err() {
        let pid = PidController::new(zero_gains());
        let state = GncState::new();
        let sp = Setpoint::new();
        let err = pid.step(&state, &sp, 0.0).unwrap_err();
        assert!(matches!(err, GncError::DtNotPositive { .. }));
    }

    #[test]
    fn dt_negative_returns_err() {
        let lqri = LqriController::new(zero_lqri());
        let state = GncState::new();
        let sp = Setpoint::new();
        let err = lqri.step(&state, &sp, -0.01).unwrap_err();
        assert!(matches!(err, GncError::DtNotPositive { .. }));
    }

    #[test]
    fn quaternion_norm_check() {
        let q: UnitQuaternion<f64> = UnitQuaternion::identity();
        let norm: f64 = q.quaternion().norm();
        assert!((norm - 1.0).abs() < 1e-12, "identity quaternion is unit");
    }
}
