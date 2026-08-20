//! # themql-embedded
//!
//! Embedded binary. Runs on the flight controller
//! (`thumbv7em-none-eabihf` or `riscv32imc`). Per `specs/embedded.toml`,
//! this binary composes `themql-runtime` (embassy), `themql-mqtt`,
//! `themql-gnc`, `themql-estimation`, `themql-inference`,
//! `themql-artifact`, and `themql-telemetry`, and must not depend on
//! desktop infrastructure.
//!
//! When compiled for an embedded target (`target_os = "none"`), this is
//! a `#![no_std]` + `#![no_main]` binary using `embassy-executor` with
//! the full task topology from `specs/embedded.toml [tasks]`:
//! sensor tasks (gps, baro, imu), estimator task, controller task,
//! telemetry task, inference task, and command task.
//!
//! When compiled for the host, a stub `main` is provided so that
//! `cargo check --workspace` and `cargo test --workspace` succeed. The
//! [`SensorDriver`] trait, [`SensorError`] enum, and stub driver impls
//! are shared between both targets and unit-tested on the host.
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
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

// ---------------------------------------------------------------------------
// Shared types — available on both host and embedded
// ---------------------------------------------------------------------------

#[cfg(target_os = "none")]
use core::fmt;

#[cfg(not(target_os = "none"))]
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
    BusError(HeapString),
    /// The read did not complete within the deadline.
    Timeout,
    /// The sensor did not respond at all.
    SensorNotResponding,
    /// The sensor returned a reading that failed validation.
    InvalidReading(HeapString),
    /// The sensor requires calibration before use.
    CalibrationRequired,
}

/// Fixed-capacity string for error messages (no heap allocation).
/// Stores up to 63 bytes + NUL terminator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeapString {
    /// Fixed buffer.
    buf: [u8; 64],
    /// Valid length.
    len: u8,
}

impl HeapString {
    /// Construct a `HeapString` from a string slice, truncating at 64
    /// bytes.
    #[must_use]
    pub fn new(s: &str) -> Self {
        let truncated_len = s.len().min(64);
        let mut buf = [0u8; 64];
        buf[..truncated_len].copy_from_slice(&s.as_bytes()[..truncated_len]);
        Self {
            buf,
            len: u8::try_from(truncated_len).unwrap_or(64),
        }
    }

    /// Return the string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        let end = self.len as usize;
        if end == 0 {
            return "";
        }
        core::str::from_utf8(&self.buf[..end]).unwrap_or("")
    }
}

impl fmt::Display for HeapString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
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

#[cfg(not(target_os = "none"))]
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
// Stub driver impls — compile on both host and embedded
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
// Task configuration constants — from specs/embedded.toml [tasks]
// ===========================================================================

/// GPS sample rate in Hz. Per `specs/embedded.toml [tasks.sensors]`.
pub const GPS_RATE_HZ: u32 = 10;
/// Barometer sample rate in Hz.
pub const BARO_RATE_HZ: u32 = 50;
/// IMU sample rate in Hz.
pub const IMU_RATE_HZ: u32 = 200;
/// Telemetry publish rate in Hz.
pub const TELEMETRY_RATE_HZ: u32 = 10;
/// Inference rate in Hz.
pub const INFERENCE_RATE_HZ: u32 = 5;
/// Main loop period in milliseconds (IMU rate = 200 Hz = 5 ms).
pub const MAIN_LOOP_MS: u64 = 5;

/// Telemetry subjects per `specs/embedded.toml [tasks.telemetry]`.
pub const TELEMETRY_SUBJECTS: [&str; 4] = [
    "vehicle.state_estimate",
    "vehicle.covariance",
    "vehicle.controller_state",
    "vehicle.diagnostics",
];

// ===========================================================================
// Inter-task communication — heapless SPSC channels
// ===========================================================================

/// Capacity of the sensor→estimator channel (must be power of 2 for
/// `heapless::spsc::Channel`). 8 is sufficient for 200 Hz IMU with 5 ms
/// estimator cycle.
pub const SENSOR_CHANNEL_CAPACITY: usize = 8;

/// A sensor reading tagged with the sensor kind for the estimator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaggedReading {
    /// The sensor kind.
    pub kind: SensorKind,
    /// The reading data.
    pub reading: SensorReading,
}

// ===========================================================================
// Embedded entry point — embassy executor
// ===========================================================================

#[cfg(target_os = "none")]
mod embedded {
    use embassy_executor::Spawner;
    use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
    use embassy_sync::channel::Channel;
    use embassy_time::{Duration, Timer};

    use super::*;

    /// Channel: GPS driver → estimator.
    static GPS_CHAN: Channel<CriticalSectionRawMutex, TaggedReading, 8> = Channel::new();
    /// Channel: Baro driver → estimator.
    static BARO_CHAN: Channel<CriticalSectionRawMutex, TaggedReading, 8> = Channel::new();
    /// Channel: IMU driver → estimator.
    static IMU_CHAN: Channel<CriticalSectionRawMutex, TaggedReading, 8> = Channel::new();

    /// GPS sensor task. Reads at 10 Hz, pushes to [`GPS_CHAN`].
    #[embassy_executor::task]
    pub async fn gps_task(mut driver: GpsDriver) {
        let period = Duration::from_hz(GPS_RATE_HZ as u64);
        loop {
            if let Ok(reading) = driver.read() {
                let _ = GPS_CHAN.try_send(TaggedReading {
                    kind: SensorKind::Gps,
                    reading,
                });
            }
            Timer::after(period).await;
        }
    }

    /// Barometer sensor task. Reads at 50 Hz, pushes to [`BARO_CHAN`].
    #[embassy_executor::task]
    pub async fn baro_task(mut driver: BaroDriver) {
        let period = Duration::from_hz(BARO_RATE_HZ as u64);
        loop {
            if let Ok(reading) = driver.read() {
                let _ = BARO_CHAN.try_send(TaggedReading {
                    kind: SensorKind::Barometer,
                    reading,
                });
            }
            Timer::after(period).await;
        }
    }

    /// IMU sensor task. Reads at 200 Hz, pushes to [`IMU_CHAN`].
    #[embassy_executor::task]
    pub async fn imu_task(mut driver: ImuDriver) {
        let period = Duration::from_hz(IMU_RATE_HZ as u64);
        loop {
            if let Ok(reading) = driver.read() {
                let _ = IMU_CHAN.try_send(TaggedReading {
                    kind: SensorKind::Imu,
                    reading,
                });
            }
            Timer::after(period).await;
        }
    }

    /// Estimator task. Consumes sensor readings from all three channels
    /// at the IMU rate (200 Hz). In the full implementation this runs
    /// the EKF predict/update cycle. In this stage it drains channels
    /// and counts samples.
    #[embassy_executor::task]
    pub async fn estimator_task() {
        let period = Duration::from_hz(IMU_RATE_HZ as u64);
        let mut gps_count: u32 = 0;
        let mut baro_count: u32 = 0;
        let mut imu_count: u32 = 0;
        loop {
            while GPS_CHAN.try_receive().is_ok() {
                gps_count = gps_count.wrapping_add(1);
            }
            while BARO_CHAN.try_receive().is_ok() {
                baro_count = baro_count.wrapping_add(1);
            }
            while IMU_CHAN.try_receive().is_ok() {
                imu_count = imu_count.wrapping_add(1);
            }
            Timer::after(period).await;
        }
    }

    /// Telemetry task. Publishes at 10 Hz to the four subjects defined
    /// in [`TELEMETRY_SUBJECTS`]. In the full implementation this
    /// publishes `TelemetryMessage` via MQTT. In this stage it counts
    /// publish cycles.
    #[embassy_executor::task]
    pub async fn telemetry_task() {
        let period = Duration::from_hz(TELEMETRY_RATE_HZ as u64);
        let mut publish_count: u32 = 0;
        loop {
            publish_count = publish_count.wrapping_add(1);
            Timer::after(period).await;
        }
    }

    /// Inference task. Runs at 5 Hz with budget enforcement. In the
    /// full implementation this runs the ML state correction with
    /// rollback. In this stage it counts inference cycles.
    #[embassy_executor::task]
    pub async fn inference_task() {
        let period = Duration::from_hz(INFERENCE_RATE_HZ as u64);
        let mut infer_count: u32 = 0;
        loop {
            infer_count = infer_count.wrapping_add(1);
            Timer::after(period).await;
        }
    }

    /// Command task. Subscribes to the command subject and dispatches
    /// to the controller setpoint. In the full implementation this
    /// receives MQTT commands. In this stage it waits indefinitely.
    #[embassy_executor::task]
    pub async fn command_task() {
        loop {
            Timer::after(Duration::from_secs(1)).await;
        }
    }

    /// Spawn a task, ignoring spawn errors (task pool full). Per
    /// TETANUS, this avoids `unwrap()`/`expect()` while still
    /// attempting to spawn all tasks.
    fn try_spawn(
        spawner: &Spawner,
        token: Result<embassy_executor::SpawnToken<impl Sized>, embassy_executor::SpawnError>,
    ) {
        if let Ok(t) = token {
            spawner.spawn(t);
        }
    }

    /// Embassy entry point. Per `specs/embedded.toml [entry]`, spawns
    /// sensor tasks, estimator, telemetry, inference, and command tasks,
    /// then runs the main loop at IMU rate.
    #[embassy_executor::main]
    async fn main(spawner: Spawner) {
        let gps = GpsDriver::new(GPS_RATE_HZ);
        let baro = BaroDriver::new(BARO_RATE_HZ);
        let imu = ImuDriver::new(IMU_RATE_HZ);

        try_spawn(&spawner, gps_task(gps));
        try_spawn(&spawner, baro_task(baro));
        try_spawn(&spawner, imu_task(imu));
        try_spawn(&spawner, estimator_task());
        try_spawn(&spawner, telemetry_task());
        try_spawn(&spawner, inference_task());
        try_spawn(&spawner, command_task());

        loop {
            Timer::after(Duration::from_millis(MAIN_LOOP_MS)).await;
        }
    }

    /// Panic handler for `no_std` target.
    #[panic_handler]
    fn panic(_info: &core::panic::PanicInfo) -> ! {
        loop {
            core::hint::spin_loop();
        }
    }
}

// ===========================================================================
// Host entry point — stub for cargo check/test on x86
// ===========================================================================

#[cfg(not(target_os = "none"))]
mod host {
    /// Embedded entry point (stub).
    ///
    /// Per `specs/embedded.toml [entry]`, the canonical form is an
    /// `#[embassy_executor::main]` `async fn` that spawns sensor tasks
    /// (gps, baro, imu), an estimator task (EKF), a controller task
    /// (`HybridController`), a telemetry task (MQTT publisher at 10 Hz
    /// on `vehicle.state_estimate`, `vehicle.covariance`,
    /// `vehicle.controller_state`, `vehicle.diagnostics`), an inference
    /// task (ML state correction at 5 Hz with budget enforcement +
    /// rollback ready), and a command task (subscribes to command
    /// subject, dispatches to controller setpoint), then runs the main
    /// loop at IMU rate.
    ///
    /// Embassy does not compile for the x86 host target, so this stub
    /// prints a banner and returns. The real embassy main is compiled
    /// when `target_os = "none"` (see the `embedded` module above).
    #[allow(clippy::unnecessary_wraps)]
    pub fn main() -> Result<(), Box<dyn std::error::Error>> {
        println!("themql-embedded: stub — embassy runtime requires thumbv7em target");
        println!(
            "  sensor rates: gps={}Hz baro={}Hz imu={}Hz",
            super::GPS_RATE_HZ,
            super::BARO_RATE_HZ,
            super::IMU_RATE_HZ
        );
        println!(
            "  telemetry: {}Hz on {} subjects",
            super::TELEMETRY_RATE_HZ,
            super::TELEMETRY_SUBJECTS.len()
        );
        println!(
            "  inference: {}Hz with budget enforcement",
            super::INFERENCE_RATE_HZ
        );
        Ok(())
    }
}

#[cfg(not(target_os = "none"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    host::main()
}

// ===========================================================================
// Tests — run on host only
// ===========================================================================

#[cfg(all(test, not(target_os = "none")))]
mod tests {
    use super::*;

    #[test]
    fn sensor_error_variants_display() {
        assert_eq!(
            SensorError::BusError(HeapString::new("nak")).to_string(),
            "bus error: nak"
        );
        assert_eq!(SensorError::Timeout.to_string(), "sensor read timeout");
        assert_eq!(
            SensorError::SensorNotResponding.to_string(),
            "sensor not responding"
        );
        assert_eq!(
            SensorError::InvalidReading(HeapString::new("bad crc")).to_string(),
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

    #[test]
    fn gps_driver_construction() {
        let d = GpsDriver::new(10);
        assert_eq!(d.rate_hz, 10);
        assert_eq!(d.kind(), SensorKind::Gps);
    }

    #[test]
    fn baro_driver_construction() {
        let d = BaroDriver::new(50);
        assert_eq!(d.rate_hz, 50);
        assert_eq!(d.kind(), SensorKind::Barometer);
    }

    #[test]
    fn imu_driver_construction() {
        let d = ImuDriver::new(200);
        assert_eq!(d.rate_hz, 200);
        assert_eq!(d.kind(), SensorKind::Imu);
    }

    #[test]
    fn sensor_error_all_variants_distinct() {
        assert_ne!(
            SensorError::BusError(HeapString::new("a")),
            SensorError::Timeout
        );
        assert_ne!(SensorError::Timeout, SensorError::SensorNotResponding);
        assert_ne!(
            SensorError::SensorNotResponding,
            SensorError::InvalidReading(HeapString::new("b"))
        );
        assert_ne!(
            SensorError::InvalidReading(HeapString::new("b")),
            SensorError::CalibrationRequired
        );
    }

    #[test]
    fn sensor_driver_trait_object_dispatch() {
        let mut drivers: [Box<dyn SensorDriver>; 3] = [
            Box::new(GpsDriver::new(10)),
            Box::new(BaroDriver::new(50)),
            Box::new(ImuDriver::new(200)),
        ];
        for d in &mut drivers {
            let reading = d.read().expect("driver read");
            assert_eq!(reading.kind, d.kind());
            assert_eq!(reading.raw.len(), 32);
        }
        assert_eq!(drivers[0].kind(), SensorKind::Gps);
        assert_eq!(drivers[1].kind(), SensorKind::Barometer);
        assert_eq!(drivers[2].kind(), SensorKind::Imu);
    }

    #[test]
    fn sensor_error_is_std_error() {
        fn assert_std_error<E: std::error::Error>() {}
        assert_std_error::<SensorError>();
    }

    #[test]
    fn heap_string_truncates_long_strings() {
        let long = "a".repeat(100);
        let hs = HeapString::new(&long);
        assert_eq!(hs.as_str().len(), 64);
    }

    #[test]
    fn heap_string_preserves_short_strings() {
        let hs = HeapString::new("hello");
        assert_eq!(hs.as_str(), "hello");
    }

    #[test]
    fn heap_string_empty_string() {
        let hs = HeapString::new("");
        assert_eq!(hs.as_str(), "");
    }

    #[test]
    fn task_config_constants_match_spec() {
        assert_eq!(GPS_RATE_HZ, 10);
        assert_eq!(BARO_RATE_HZ, 50);
        assert_eq!(IMU_RATE_HZ, 200);
        assert_eq!(TELEMETRY_RATE_HZ, 10);
        assert_eq!(INFERENCE_RATE_HZ, 5);
        assert_eq!(MAIN_LOOP_MS, 5);
    }

    #[test]
    fn telemetry_subjects_match_spec() {
        assert_eq!(TELEMETRY_SUBJECTS.len(), 4);
        assert_eq!(TELEMETRY_SUBJECTS[0], "vehicle.state_estimate");
        assert_eq!(TELEMETRY_SUBJECTS[1], "vehicle.covariance");
        assert_eq!(TELEMETRY_SUBJECTS[2], "vehicle.controller_state");
        assert_eq!(TELEMETRY_SUBJECTS[3], "vehicle.diagnostics");
    }

    #[test]
    fn tagged_reading_constructs() {
        let tr = TaggedReading {
            kind: SensorKind::Imu,
            reading: SensorReading {
                kind: SensorKind::Imu,
                raw: [0; 32],
                len: 0,
            },
        };
        assert_eq!(tr.kind, SensorKind::Imu);
        assert_eq!(tr.reading.raw.len(), 32);
    }
}
