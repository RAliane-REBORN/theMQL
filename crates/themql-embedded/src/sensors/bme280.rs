//! Bosch BME280 driver (I2C).
//!
//! Barometer / pressure / humidity / temperature. Reads calibration
//! coefficients from the sensor at init, then burst-reads raw data and
//! applies the datasheet compensation formulas (BME280 datasheet §4.6).
//!
//! Generic over any `embedded_hal_async::i2c::I2c` implementation.
//! Default I2C address is 0x76 (secondary 0x77 when SDO is pulled high).

use crate::sensors::types::{heap_string, SensorDriver, SensorError, SensorKind, SensorReading};
use embedded_hal_async::i2c::I2c;

/// BME280 default I2C address (SDO low).
pub const BME280_ADDR: u8 = 0x76;

/// WHO_AM_I register — should return `0x60`.
const REG_ID: u8 = 0xD0;
/// Reset register — writing `0xB6` triggers a soft reset.
const REG_RESET: u8 = 0xE0;
/// Humidity control register.
const REG_CTRL_HUM: u8 = 0xF2;
/// Control register (temperature + pressure oversampling + mode).
const REG_CTRL_MEAS: u8 = 0xF4;
/// Config register (standby time + IIR filter).
const REG_CONFIG: u8 = 0xF5;
/// Calibration coefficient start register (calib00).
const REG_CALIB00: u8 = 0x88;
/// Calibration coefficient start register (calib26).
const REG_CALIB26: u8 = 0xE1;
/// Raw data start register (pressure, temperature, humidity).
const REG_DATA: u8 = 0xF7;

/// WHO_AM_I expected value for BME280.
const BME280_ID: u8 = 0x60;

/// Calibration coefficients (trimmed at factory). 12 values from
/// 0x88–0xA1 and 9 values from 0xE1–0xF0, per datasheet §4.6.1.
#[derive(Debug, Clone, Copy, Default)]
pub struct Bme280Calib {
    /// Temperature compensation T1 (unsigned, 16-bit).
    pub t1: u16,
    /// Temperature compensation T2 (signed).
    pub t2: i16,
    /// Temperature compensation T3 (signed).
    pub t3: i16,
    /// Pressure compensation P1 (unsigned).
    pub p1: u16,
    /// Pressure compensation P2 (signed).
    pub p2: i16,
    /// Pressure compensation P3 (signed).
    pub p3: i16,
    /// Pressure compensation P4 (signed).
    pub p4: i16,
    /// Pressure compensation P5 (signed).
    pub p5: i16,
    /// Pressure compensation P6 (signed).
    pub p6: i16,
    /// Pressure compensation P7 (signed).
    pub p7: i16,
    /// Pressure compensation P8 (signed).
    pub p8: i16,
    /// Pressure compensation P9 (signed).
    pub p9: i16,
    /// Humidity compensation H1 (unsigned, 8-bit).
    pub h1: u8,
    /// Humidity compensation H2 (signed, 16-bit).
    pub h2: i16,
    /// Humidity compensation H3 (unsigned, 8-bit).
    pub h3: u8,
    /// Humidity compensation H4 (signed, 12-bit, split across two regs).
    pub h4: i16,
    /// Humidity compensation H5 (signed, 12-bit, split across two regs).
    pub h5: i16,
    /// Humidity compensation H6 (signed, 8-bit).
    pub h6: i8,
}

/// BME280 driver over I2C. Holds the bus handle and calibration
/// coefficients (read once at init).
pub struct Bme280<I: I2c> {
    i2c: I,
    addr: u8,
    rate_hz: u32,
    calib: Bme280Calib,
    /// Fine-grained temperature used by pressure/humidity compensation.
    t_fine: i32,
}

impl<I: I2c> Bme280<I> {
    /// Construct a new BME280 driver at the default address (0x76).
    #[must_use]
    pub fn new(i2c: I, rate_hz: u32) -> Self {
        Self {
            i2c,
            addr: BME280_ADDR,
            rate_hz,
            calib: Bme280Calib::default(),
            t_fine: 0,
        }
    }

    /// Construct at a specific I2C address (0x76 or 0x77).
    #[must_use]
    pub fn with_address(i2c: I, addr: u8, rate_hz: u32) -> Self {
        Self {
            i2c,
            addr,
            rate_hz,
            calib: Bme280Calib::default(),
            t_fine: 0,
        }
    }

    /// Initialise: verify WHO_AM_I, soft-reset, read calibration
    /// coefficients, configure oversampling. Must be called once before
    /// [`Self::read`].
    ///
    /// # Errors
    /// Returns [`SensorError::SensorNotResponding`] if WHO_AM_I is wrong.
    /// Returns [`SensorError::BusError`] on I2C failure.
    pub async fn init(&mut self) -> Result<(), SensorError> {
        let id = self.read_reg8(REG_ID).await?;
        if id != BME280_ID {
            return Err(SensorError::SensorNotResponding);
        }
        self.write_reg8(REG_RESET, 0xB6).await?;
        self.read_calibration().await?;
        self.configure().await?;
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

    /// Read all 21 calibration coefficients from registers 0x88–0xA1
    /// and 0xE1–0xF0. Splits the reads into two bursts per datasheet.
    async fn read_calibration(&mut self) -> Result<(), SensorError> {
        let mut buf1 = [0u8; 26];
        self.i2c
            .write_read(self.addr, &[REG_CALIB00], &mut buf1)
            .await
            .map_err(|_| SensorError::BusError(heap_string("calib1")))?;
        let mut buf2 = [0u8; 9];
        self.i2c
            .write_read(self.addr, &[REG_CALIB26], &mut buf2)
            .await
            .map_err(|_| SensorError::BusError(heap_string("calib2")))?;
        self.calib = parse_calibration(&buf1, &buf2);
        debug_assert!(self.calib.t1 != 0, "T1 must be non-zero");
        Ok(())
    }

    /// Configure oversampling: 1× humidity, 1× temperature, 1× pressure,
    /// normal mode with 1000ms standby and IIR filter off.
    async fn configure(&mut self) -> Result<(), SensorError> {
        self.write_reg8(REG_CTRL_HUM, 0x01).await?;
        self.write_reg8(REG_CONFIG, 0xA0).await?;
        self.write_reg8(REG_CTRL_MEAS, 0x27).await?;
        Ok(())
    }

    /// Burst-read 8 bytes of raw data (press + temp + hum) and
    /// compensate. Returns a [`SensorReading`] with temperature (°C ×100),
    /// pressure (Pa), and humidity (% ×100) packed into the raw buffer.
    ///
    /// # Errors
    /// Returns [`SensorError::BusError`] on I2C failure.
    pub async fn read(&mut self) -> Result<SensorReading, SensorError> {
        let mut buf = [0u8; 8];
        self.i2c
            .write_read(self.addr, &[REG_DATA], &mut buf)
            .await
            .map_err(|_| SensorError::BusError(heap_string("data read")))?;
        let raw_press = u32_from_3(&buf, 0) >> 4;
        let raw_temp = u32_from_3(&buf, 3) >> 4;
        let raw_hum = u16::from_be_bytes([buf[6], buf[7]]);
        let temp_c100 = self.compensate_temp(raw_temp);
        let press_pa = self.compensate_pressure(raw_press);
        let hum_c100 = self.compensate_humidity(raw_hum);
        Ok(pack_reading(temp_c100, press_pa, hum_c100))
    }

    /// Compensate temperature per datasheet §4.6.3. Returns °C × 100
    /// as an integer (e.g. 2512 = 25.12°C). Updates `t_fine`.
    fn compensate_temp(&mut self, raw: u32) -> i32 {
        let c = &self.calib;
        let t1 = i32::from(c.t1);
        let t2 = i32::from(c.t2);
        let t3 = i32::from(c.t3);
        let var1 = ((raw as i32) >> 3) - (t1 << 1);
        let var2 = (var1 * t2) >> 11;
        let var3 = ((var1 >> 1) * (var1 >> 1)) >> 12;
        let var3 = (var3 * t3) >> 14;
        self.t_fine = var2 + var3;
        (self.t_fine * 5 + 128) >> 8
    }

    /// Compensate pressure per datasheet §4.6.3. Returns Pa as i32.
    fn compensate_pressure(&self, raw: u32) -> i32 {
        let c = &self.calib;
        let tf = self.t_fine;
        let var1 = ((tf as i64) << 2) - 64000;
        let var2 = ((var1 * var1) >> 11) * (i64::from(c.p6));
        let var2 = var2 + ((var1 * i64::from(c.p5)) << 1);
        let var2 = (var2 >> 2) + (i64::from(c.p4) << 16);
        let var1 = ((i64::from(c.p3) * ((var1 * var1) >> 13)) >> 3)
            + ((i64::from(c.p2) * var1) >> 1);
        let var1 = var1 >> 18;
        let var1 = ((32768 + var1) * i64::from(c.p1)) >> 15;
        if var1 == 0 {
            return 0;
        }
        let p = ((1048576 - raw as i64) - (var2 >> 12)) * 3125;
        let p = (p << 1) / var1;
        let var1 = (i64::from(c.p9) * ((p >> 3) * (p >> 3))) >> 12;
        let var2 = (i64::from(c.p8) * p) >> 13;
        let p = p + ((var1 + var2 + i64::from(c.p7)) >> 4);
        p as i32
    }

    /// Compensate humidity per datasheet §4.6.3. Returns %RH × 100
    /// as an i32 (e.g. 4500 = 45.00%).
    fn compensate_humidity(&self, raw: u16) -> i32 {
        let c = &self.calib;
        let tf = self.t_fine as i64;
        let h = tf - 76800;
        let h_x1 = ((raw as i64) << 14) - (i64::from(c.h4) << 20)
            - (i64::from(c.h5) * h)
            + (16384 >> 2);
        let h_x2 = (h_x1 * i64::from(c.h6)) >> 10;
        let h_x3 = (h_x2 * i64::from(c.h3)) >> 11;
        if h_x3 < 0 {
            return 0;
        }
        let h_x4 = (h_x3 * h_x3) >> 12;
        let h_x5 = (h_x4 * 16384) >> 10;
        let h_x6 = h_x3 + h_x5;
        let v = (h_x6 >> 2) * 100;
        if v > 4294967296 {
            0
        } else if v > 1_000_000 {
            1_000_000
        } else {
            v as i32
        }
    }
}

impl<I: I2c + Send + Sync> SensorDriver for Bme280<I> {
    async fn read(&mut self) -> Result<SensorReading, SensorError> {
        Bme280::read(self).await
    }
    fn kind(&self) -> SensorKind {
        SensorKind::Barometer
    }
    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
}

/// Assemble a 20-bit big-endian value from 3 bytes starting at `off`.
fn u32_from_3(buf: &[u8], off: usize) -> u32 {
    debug_assert!(off + 2 < buf.len(), "3-byte read in bounds");
    let hi = u32::from(buf[off]);
    let mid = u32::from(buf[off + 1]);
    let lo = u32::from(buf[off + 2]);
    (hi << 16) | (mid << 8) | lo
}

/// Parse 21 calibration coefficients from the two register bursts.
fn parse_calibration(buf1: &[u8; 26], buf2: &[u8; 9]) -> Bme280Calib {
    let t1 = u16::from_le_bytes([buf1[0], buf1[1]]);
    let t2 = i16::from_le_bytes([buf1[2], buf1[3]]);
    let t3 = i16::from_le_bytes([buf1[4], buf1[5]]);
    let p1 = u16::from_le_bytes([buf1[6], buf1[7]]);
    let p2 = i16::from_le_bytes([buf1[8], buf1[9]]);
    let p3 = i16::from_le_bytes([buf1[10], buf1[11]]);
    let p4 = i16::from_le_bytes([buf1[12], buf1[13]]);
    let p5 = i16::from_le_bytes([buf1[14], buf1[15]]);
    let p6 = i16::from_le_bytes([buf1[16], buf1[17]]);
    let p7 = i16::from_le_bytes([buf1[18], buf1[19]]);
    let p8 = i16::from_le_bytes([buf1[20], buf1[21]]);
    let p9 = i16::from_le_bytes([buf1[22], buf1[23]]);
    let h1 = buf1[25];
    let h2 = i16::from_le_bytes([buf2[0], buf2[1]]);
    let h3 = buf2[2];
    let h4 = ((i16::from(buf2[3])) << 4) | (i16::from(buf2[4] & 0x0F));
    let h5 = ((i16::from(buf2[5])) << 4) | (i16::from(buf2[4] >> 4));
    let h6 = i8::from_le_bytes([buf2[6]]);
    Bme280Calib {
        t1, t2, t3, p1, p2, p3, p4, p5, p6, p7, p8, p9,
        h1, h2, h3, h4, h5, h6,
    }
}

/// Pack compensated readings (temp ×100, pressure Pa, hum ×100) into
/// a [`SensorReading`]'s fixed 32-byte raw buffer.
fn pack_reading(temp_c100: i32, press_pa: i32, hum_c100: i32) -> SensorReading {
    let mut raw = [0u8; 32];
    raw[0..4].copy_from_slice(&temp_c100.to_le_bytes());
    raw[4..8].copy_from_slice(&press_pa.to_le_bytes());
    raw[8..12].copy_from_slice(&hum_c100.to_le_bytes());
    SensorReading {
        kind: SensorKind::Barometer,
        raw,
        len: 12,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_calibration_matches_known_layout() {
        let mut buf1 = [0u8; 26];
        buf1[0] = 0x6C;
        buf1[1] = 0x75;
        let buf2 = [0u8; 9];
        let c = parse_calibration(&buf1, &buf2);
        assert_eq!(c.t1, 0x756C);
        assert_eq!(c.h6, 0);
    }

    #[test]
    fn pack_reading_round_trips_4_byte_fields() {
        let r = pack_reading(2512, 101325, 4500);
        let temp = i32::from_le_bytes([
            r.raw[0], r.raw[1], r.raw[2], r.raw[3],
        ]);
        let press = i32::from_le_bytes([
            r.raw[4], r.raw[5], r.raw[6], r.raw[7],
        ]);
        let hum = i32::from_le_bytes([
            r.raw[8], r.raw[9], r.raw[10], r.raw[11],
        ]);
        assert_eq!(temp, 2512);
        assert_eq!(press, 101325);
        assert_eq!(hum, 4500);
        assert_eq!(r.len, 12);
    }
}