//! Real sensor drivers, generic over `embedded-hal` 1.0 traits.
//!
//! Each driver implements the [`super::SensorDriver`] trait and is
//! parameterised over the bus it uses:
//! - [`bme280::Bme280`] — I2C barometer/pressure/humidity (Bosch BME280)
//! - [`lsm6ds3::Lsm6ds3`] — I2C IMU (ST LSM6DS3)
//! - [`neo6m::Neo6m`] — UART GPS (u-blox NEO-6M, NMEA 0183)
//!
//! Drivers are unit-tested on the host with mock buses (no hardware
//! required). On embedded targets, concrete peripheral handles from
//! the MCU HAL are passed in at init.
//!
//! ## TETANUS compliance
//!
//! All drivers follow the NASA JPL Power of Ten rules: no recursion,
//! no `unwrap`/`expect`, fixed-bounds loops, functions ≤ 60 lines,
//! ≥ 2 assertions per function (via `debug_assert!` returning `Err`),
//! no dynamic allocation after init.

pub mod bme280;
pub mod lsm6ds3;
pub mod neo6m;
pub mod types;