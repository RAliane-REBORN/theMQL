//! Benchmark: EKF predict + update cycle (hot path).
//!
//! Covers `SPEC.toml [quality] benchmark_hot_paths = true`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use themql_estimation::{BarometerReading, Ekf, Estimator, GpsReading, ImuReading};

fn bench_ekf_predict(c: &mut Criterion) {
    c.bench_function("ekf_predict", |b| {
        b.iter(|| {
            let mut ekf = Ekf::new();
            let imu = ImuReading {
                accel: [0.1, -0.2, 9.8],
                gyro: [0.01, -0.02, 0.003],
            };
            let _ = black_box(ekf.predict(0.005, &imu));
        });
    });
}

fn bench_ekf_predict_then_gps_update(c: &mut Criterion) {
    c.bench_function("ekf_predict_then_gps_update", |b| {
        b.iter(|| {
            let mut ekf = Ekf::new();
            let imu = ImuReading {
                accel: [0.1, -0.2, 9.8],
                gyro: [0.01, -0.02, 0.003],
            };
            ekf.predict(0.005, &imu).expect("predict");
            let gps = GpsReading {
                position: [1.0, 2.0, 3.0],
                velocity: [0.1, 0.2, 0.3],
                clock_bias: 1e-8,
            };
            let _ = black_box(ekf.update_gps(&gps));
        });
    });
}

fn bench_ekf_predict_then_baro_update(c: &mut Criterion) {
    c.bench_function("ekf_predict_then_baro_update", |b| {
        b.iter(|| {
            let mut ekf = Ekf::new();
            let imu = ImuReading {
                accel: [0.1, -0.2, 9.8],
                gyro: [0.01, -0.02, 0.003],
            };
            ekf.predict(0.005, &imu).expect("predict");
            let baro = BarometerReading {
                altitude: 100.0,
                altitude_bias: 0.5,
            };
            let _ = black_box(ekf.update_baro(&baro));
        });
    });
}

criterion_group!(
    benches,
    bench_ekf_predict,
    bench_ekf_predict_then_gps_update,
    bench_ekf_predict_then_baro_update
);
criterion_main!(benches);
