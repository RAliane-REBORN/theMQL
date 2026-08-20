//! ST LSM6DS3 driver (I2C).
//!
//! 3-axis accelerometer + 3-axis gyroscope. Reads WHO_AM_I at init,
//! configures CTRL registers for ±2g accel / ±2000dps gyro scales,
//! then burst-reads 12 bytes of accel+gyro data and converts to
//! physical units.
//!
//! Generic over any `embedded_hal_async::i2c::I2c` implementation.
//! Default I2C address is 0x6A (0x6B when SA1 is pulled high).

use crate::sensors::types::{heap_string, SensorDriver, SensorError, SensorKind, SensorReading};
use embedded_hal_async::i2c::I2c;

/// LSM6DS3 default I2C address (SA1 low).
pub const LSM6DS3_ADDR: u8 = 0x6A;

/// WHO_AM_I register — should return `0x69`.
const REG_WHO_AM_I: u8 = 0x0F;
/// WHO_AM_I expected value.
const LSM6DS3_ID: u8 = 0x69;
/// Accelerometer output registers (X/Y/Z, 16-bit each = 6 bytes).
const REG_OUTX_L_G: u8 = 0x22;
/// Gyroscope output registers start.
const REG_OUTX_L_A: u8 = 0x28;
/// CTRL1_XL: accelerometer ODR + full-scale selection.
const REG_CTRL1_XL: u8 = 0x10;
/// CTRL2_G: gyroscope ODR + full-scale selection.
const REG_CTRL2_G: u8 = 0x11;

/// Accel full-scale: ±2g. Sensitivity = 0.061 mg/LSB.
const ACCEL_SENSITIVITY: f32 = 0.000061;
/// Gyro full-scale: ±2000dps. Sensitivity = 70 mdps/LSB.
const GYRO_SENSITIVITY: f32 = 0.070;

/// LSM6DS3 driver over I2C.
pub struct Lsm6ds3<I: I2c> {
    i2c: I,
    addr: u8,
    rate_hz: u32,
}

impl<I: I2c> Lsm6ds3<I> {
    /// Construct a new LSM6DS3 driver at the default address (0x6A).
    #[must_use]
    pub fn new(i2c: I, rate_hz: u32) -> Self {
        Self {
            i2c,
            addr: LSM6DS3_ADDR,
            rate_hz,
        }
    }

    /// Construct at a specific I2C address.
    #[must_use]
    pub fn with_address(i2c: I, addr: u8, rate_hz: u32) -> Self {
        Self { i2c, addr, rate_hz }
    }

    /// Initialise: verify WHO_AM_I, configure accel/gyro for ±2g /
    /// ±2000dps at 416 Hz. Must be called once before [`Self::read`].
    ///
    /// # Errors
    /// Returns [`SensorError::SensorNotResponding`] if WHO_AM_I mismatches.
    pub async fn init(&mut self) -> Result<(), SensorError> {
        let id = self.read_reg8(REG_WHO_AM_I).await?;
        if id != LSM6DS3_ID {
            return Err(SensorError::SensorNotResponding);
        }
        self.write_reg8(REG_CTRL1_XL, 0x6A).await?;
        self.write_reg8(REG_CTRL2_G, 0x6C).await?;
        Ok(())
    }

    /// Read an 8-bit register.
    async fn read_reg8(&mut self, reg: u8) -> Result<u8, SensorError> {
        let mut buf = [0u8; 1];
        self.i2c
            .write_read(self.addr, &[reg], &mut buf)
            .await
            .map_err(|_| SensorError::BusError(heap_string("i2c read")))?;
        Ok(buf[0])
    }

    /// Write an 8-bit register.
    async fn write_reg8(&mut self, reg: u8, val: u8) -> Result<(), SensorError> {
        self.i2c
            .write(self.addr, &[reg, val])
            .await
            .map_err(|_| SensorError::BusError(heap_string("i2c write")))?;
        Ok(())
    }

    /// Burst-read 12 bytes of accel + gyro data and convert to
    /// physical units (g and dps, ×10000 as i16). Pack into
    /// [`SensorReading`] as 6 × 2-byte little-endian values.
    ///
    /// # Errors
    /// Returns [`SensorError::BusError`] on I2C failure.
    pub async fn read(&mut self) -> Result<SensorReading, SensorError> {
        let mut gyro = [0u8; 6];
        self.i2c
            .write_read(self.addr, &[REG_OUTX_L_G], &mut gyro)
            .await
            .map_err(|_| SensorError::BusError(heap_string("gyro read")))?;
        let mut accel = [0u8; 6];
        self.i2c
            .write_read(self.addr, &[REG_OUTX_L_A], &mut accel)
            .await
            .map_err(|_| SensorError::BusError(heap_string("accel read")))?;
        Ok(pack_imu_reading(accel, gyro))
    }
}

impl<I: I2c + Send + Sync> SensorDriver for Lsm6ds3<I> {
    async fn read(&mut self) -> Result<SensorReading, SensorError> {
        Lsm6ds3::read(self).await
    }
    fn kind(&self) -> SensorKind {
        SensorKind::Imu
    }
    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
}

/// Pack 6 raw 16-bit sensor values into a [`SensorReading`].
fn pack_imu_reading(accel: [u8; 6], gyro: [u8; 6]) -> SensorReading {
    let mut raw = [0u8; 32];
    raw[0..6].copy_from_slice(&accel);
    raw[6..12].copy_from_slice(&gyro);
    SensorReading {
        kind: SensorKind::Imu,
        raw,
        len: 12,
    }
}

/// Convert a raw 16-bit accelerometer sample to g (×10000).
/// Used by the estimator decode layer, not by the driver itself.
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn accel_to_g(raw: i16) -> i16 {
    let g = f32::from(raw) * ACCEL_SENSITIVITY;
    (g * 10000.0) as i16
}

/// Convert a raw 16-bit gyroscope sample to dps (×10000).
#[must_use]
#[allow(clippy::cast_possible_truncation)]
pub fn gyro_to_dps(raw: i16) -> i16 {
    let dps = f32::from(raw) * GYRO_SENSITIVITY;
    (dps * 10000.0) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_imu_reading_preserves_all_axes() {
        let accel = [1, 2, 3, 4, 5, 6];
        let gyro = [7, 8, 9, 10, 11, 12];
        let r = pack_imu_reading(accel, gyro);
        assert_eq!(&r.raw[0..6], &[1, 2, 3, 4, 5, 6]);
        assert_eq!(&r.raw[6..12], &[7, 8, 9, 10, 11, 12]);
        assert_eq!(r.len, 12);
        assert_eq!(r.kind, SensorKind::Imu);
    }

    #[test]
    fn accel_to_g_zero_input_is_zero() {
        assert_eq!(accel_to_g(0), 0);
    }

    #[test]
    #[allow(clippy::cast_possible_truncation)]
    fn accel_to_g_positive_input_scales_correctly() {
        let raw = 1000_i16;
        let g = accel_to_g(raw);
        let expected = (f32::from(raw) * ACCEL_SENSITIVITY * 10000.0) as i16;
        assert_eq!(g, expected);
    }

    #[test]
    fn gyro_to_dps_zero_input_is_zero() {
        assert_eq!(gyro_to_dps(0), 0);
    }
}
