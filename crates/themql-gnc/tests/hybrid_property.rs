//! Property tests for `HybridController` bounded-output invariant.
//!
//! Covers `SPEC.toml [quality] property_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::cast_precision_loss)]

use proptest::prelude::*;
use themql_gnc::{
    ActuatorCommand, Controller, GncState, HybridController, HybridSwitchingPolicy, LqriController,
    LqriMatrices, PidController, PidGains, Setpoint,
};

use nalgebra::{UnitQuaternion, Vector3};

fn finite_vec3(max: f64) -> impl Strategy<Value = Vector3<f64>> {
    ((-max..max), (-max..max), (-max..max)).prop_map(|(x, y, z)| {
        Vector3::new(
            if x.is_finite() { x } else { 0.0 },
            if y.is_finite() { y } else { 0.0 },
            if z.is_finite() { z } else { 0.0 },
        )
    })
}

fn gnc_state() -> impl Strategy<Value = GncState> {
    (
        finite_vec3(100.0),
        finite_vec3(50.0),
        finite_vec3(10.0),
        finite_vec3(1.0),
    )
        .prop_map(|(pos, vel, angvel, accel_bias)| {
            let mut s = GncState::new();
            s.position = pos;
            s.velocity = vel;
            s.angular_velocity = angvel;
            s.accelerometer_bias = accel_bias;
            s
        })
}

fn setpoint() -> impl Strategy<Value = Setpoint> {
    (finite_vec3(100.0), finite_vec3(50.0), finite_vec3(10.0)).prop_map(|(tpos, tvel, tangvel)| {
        Setpoint {
            target_position: tpos,
            target_velocity: tvel,
            target_attitude: UnitQuaternion::identity(),
            target_angular_velocity: tangvel,
        }
    })
}

fn pid_gains() -> impl Strategy<Value = PidGains> {
    (0.0f64..10.0, 0.0f64..5.0, 0.0f64..2.0).prop_map(|(kp, ki, kd)| PidGains {
        kp: [kp; 3],
        ki: [ki; 3],
        kd: [kd; 3],
    })
}

fn dt_strategy() -> impl Strategy<Value = f64> {
    1.0e-3f64..1.0e-1f64
}

fn zero_lqri() -> LqriMatrices {
    LqriMatrices {
        K: nalgebra::SMatrix::zeros(),
        Ki: nalgebra::SMatrix::zeros(),
        Q: nalgebra::SMatrix::zeros(),
        R: nalgebra::SMatrix::zeros(),
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn hybrid_pid_step_produces_finite_output(
        state in gnc_state(),
        sp in setpoint(),
        gains in pid_gains(),
        dt in dt_strategy()
    ) {
        let ctrl = HybridController::new(
            HybridSwitchingPolicy::AlwaysPid,
            PidController::new(gains),
            LqriController::new(zero_lqri()),
        );
        let cmd: ActuatorCommand = ctrl.step(&state, &sp, dt).expect("step");
        for v in cmd.values.iter().copied() {
            prop_assert!(v.is_finite(), "non-finite actuator value: {v}");
        }
    }

    #[test]
    fn hybrid_step_rejects_nonpositive_dt(
        state in gnc_state(),
        sp in setpoint(),
        gains in pid_gains(),
        dt in -1.0f64..0.0f64
    ) {
        let ctrl = HybridController::new(
            HybridSwitchingPolicy::AlwaysPid,
            PidController::new(gains),
            LqriController::new(zero_lqri()),
        );
        prop_assert!(ctrl.step(&state, &sp, dt).is_err());
    }
}
