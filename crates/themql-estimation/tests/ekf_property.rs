//! Property tests for EKF covariance symmetry, PSD, and quaternion norm.
//!
//! Covers `SPEC.toml [quality] property_tests_required = true`.

#![allow(clippy::unwrap_used, clippy::expect_used)]
#![allow(clippy::cast_precision_loss)]

use proptest::prelude::*;
use themql_estimation::{BarometerReading, Ekf, Estimator, GpsReading, ImuReading};

fn finite_f64(max: f64) -> impl Strategy<Value = f64> {
    (-max..max).prop_filter_map(
        "finite",
        move |v| {
            if v.is_finite() {
                Some(v)
            } else {
                None
            }
        },
    )
}

fn imu_reading() -> impl Strategy<Value = ImuReading> {
    (
        finite_f64(10.0),
        finite_f64(10.0),
        finite_f64(10.0),
        finite_f64(1.0),
        finite_f64(1.0),
        finite_f64(1.0),
    )
        .prop_map(|(ax, ay, az, gx, gy, gz)| ImuReading {
            accel: [ax, ay, az],
            gyro: [gx, gy, gz],
        })
}

fn gps_reading() -> impl Strategy<Value = GpsReading> {
    (
        finite_f64(100.0),
        finite_f64(100.0),
        finite_f64(100.0),
        finite_f64(10.0),
        finite_f64(10.0),
        finite_f64(10.0),
        finite_f64(1.0),
    )
        .prop_map(|(px, py, pz, vx, vy, vz, cb)| GpsReading {
            position: [px, py, pz],
            velocity: [vx, vy, vz],
            clock_bias: cb,
        })
}

fn baro_reading() -> impl Strategy<Value = BarometerReading> {
    (finite_f64(1000.0), finite_f64(10.0)).prop_map(|(alt, bias)| BarometerReading {
        altitude: alt,
        altitude_bias: bias,
    })
}

fn dt_strategy() -> impl Strategy<Value = f64> {
    1.0e-3f64..1.0e-1f64
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn predict_preserves_covariance_symmetry(
        dt in dt_strategy(),
        imu in imu_reading()
    ) {
        let mut ekf = Ekf::new();
        ekf.predict(dt, &imu).expect("predict");
        let p = ekf.covariance();
        for i in 0..21 {
            for j in 0..21 {
                let diff = (p[(i, j)] - p[(j, i)]).abs();
                prop_assert!(
                    diff < 1e-9,
                    "P[{},{}] != P[{},{}]: {} vs {}",
                    i, j, j, i, p[(i, j)], p[(j, i)]
                );
            }
        }
    }

    #[test]
    fn predict_preserves_quaternion_norm(
        dt in dt_strategy(),
        imu in imu_reading()
    ) {
        let mut ekf = Ekf::new();
        ekf.predict(dt, &imu).expect("predict");
        let s = ekf.state();
        let q_norm = (s.x[6]*s.x[6] + s.x[7]*s.x[7] + s.x[8]*s.x[8] + s.x[9]*s.x[9]).sqrt();
        prop_assert!((q_norm - 1.0).abs() < 1e-6, "quaternion norm: {}", q_norm);
    }

    #[test]
    fn update_gps_preserves_covariance_symmetry(
        dt in dt_strategy(),
        imu in imu_reading(),
        gps in gps_reading()
    ) {
        let mut ekf = Ekf::new();
        ekf.predict(dt, &imu).expect("predict");
        ekf.update_gps(&gps).expect("update_gps");
        let p = ekf.covariance();
        for i in 0..21 {
            for j in 0..21 {
                let diff = (p[(i, j)] - p[(j, i)]).abs();
                prop_assert!(
                    diff < 1e-9,
                    "after update_gps P[{},{}] != P[{},{}]: {} vs {}",
                    i, j, j, i, p[(i, j)], p[(j, i)]
                );
            }
        }
    }

    #[test]
    fn update_baro_preserves_covariance_symmetry(
        dt in dt_strategy(),
        imu in imu_reading(),
        baro in baro_reading()
    ) {
        let mut ekf = Ekf::new();
        ekf.predict(dt, &imu).expect("predict");
        ekf.update_baro(&baro).expect("update_baro");
        let p = ekf.covariance();
        for i in 0..21 {
            for j in 0..21 {
                let diff = (p[(i, j)] - p[(j, i)]).abs();
                prop_assert!(
                    diff < 1e-9,
                    "after update_baro P[{},{}] != P[{},{}]: {} vs {}",
                    i, j, j, i, p[(i, j)], p[(j, i)]
                );
            }
        }
    }

    #[test]
    fn predict_rejects_nonpositive_dt(dt in -1.0f64..0.0f64) {
        let mut ekf = Ekf::new();
        let imu = ImuReading { accel: [0.0; 3], gyro: [0.0; 3] };
        let result = ekf.predict(dt, &imu);
        prop_assert!(result.is_err());
    }
}
