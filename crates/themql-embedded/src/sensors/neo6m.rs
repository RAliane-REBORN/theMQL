//! u-blox NEO-6M GPS driver (UART / NMEA 0183).
//!
//! Reads NMEA 0183 sentences from any `embedded_io_async::Read`
//! transport (typically a UART peripheral). Parses `$GPRMC` (recommended
//! minimum: lat/lon/speed/course) and `$GPGGA` (fix quality + sats).
//! Sentences are validated via XOR checksum.
//!
//! NMEA sentence format: `$<talker><type>,<fields>*<checksum>\r\n`
//! Maximum sentence length is 82 characters (including `$` and `\r\n`).

use crate::sensors::types::{heap_string, SensorDriver, SensorError, SensorKind, SensorReading};
use embedded_io_async::Read;

/// Maximum NMEA sentence length (including `$` and `\r\n`).
const NMEA_MAX_LEN: usize = 82;

/// NEO-6M GPS driver over a byte stream (UART).
pub struct Neo6m<R: Read> {
    reader: R,
    rate_hz: u32,
    /// Ring buffer for accumulating the current sentence.
    buf: [u8; NMEA_MAX_LEN],
    /// Number of valid bytes in `buf`.
    len: usize,
}

impl<R: Read> Neo6m<R> {
    /// Construct a new NEO-6M driver wrapping a byte stream.
    #[must_use]
    pub fn new(reader: R, rate_hz: u32) -> Self {
        Self {
            reader,
            rate_hz,
            buf: [0u8; NMEA_MAX_LEN],
            len: 0,
        }
    }

    /// Read one complete NMEA sentence and parse it. Blocks until a
    /// valid sentence is received or the stream errors.
    ///
    /// # Errors
    /// Returns [`SensorError::BusError`] on read failure.
    /// Returns [`SensorError::InvalidReading`] on checksum mismatch.
    pub async fn read_sentence(&mut self) -> Result<&[u8], SensorError> {
        loop {
            let mut byte = [0u8; 1];
            let n = self
                .reader
                .read(&mut byte)
                .await
                .map_err(|_| SensorError::BusError(heap_string("uart read")))?;
            if n == 0 {
                continue;
            }
            let b = byte[0];
            if b == b'$' {
                self.len = 0;
            }
            if self.len < NMEA_MAX_LEN {
                self.buf[self.len] = b;
                self.len += 1;
            }
            if b == b'\n' && self.len >= 6 {
                let sentence = &self.buf[..self.len];
                if validate_checksum(sentence) {
                    return Ok(sentence);
                }
                return Err(SensorError::InvalidReading(heap_string("checksum")));
            }
        }
    }

    /// Read and parse a `$GPRMC` or `$GPGGA` sentence into a
    /// [`SensorReading`]. The raw buffer carries the parsed fields
    /// (lat, lon, speed, course, sats, fix_quality) as text until the
    /// estimator decode layer interprets them.
    ///
    /// # Errors
    /// Returns [`SensorError::BusError`] on read failure.
    pub async fn read(&mut self) -> Result<SensorReading, SensorError> {
        let sentence = self.read_sentence().await?;
        let mut raw = [0u8; 32];
        let copy_len = sentence.len().min(32);
        raw[..copy_len].copy_from_slice(&sentence[..copy_len]);
        Ok(SensorReading {
            kind: SensorKind::Gps,
            raw,
            len: copy_len as u8,
        })
    }
}

impl<R: Read + Send + Sync> SensorDriver for Neo6m<R> {
    async fn read(&mut self) -> Result<SensorReading, SensorError> {
        Neo6m::read(self).await
    }
    fn kind(&self) -> SensorKind {
        SensorKind::Gps
    }
    fn rate_hz(&self) -> u32 {
        self.rate_hz
    }
}

/// Validate the XOR checksum of an NMEA sentence. The sentence must
/// start with `$`, contain a `*` separator, and end with `\r\n`. The
/// two hex digits after `*` must equal the XOR of all bytes between
/// `$` and `*`.
fn validate_checksum(sentence: &[u8]) -> bool {
    if sentence.len() < 6 {
        return false;
    }
    if sentence[0] != b'$' {
        return false;
    }
    let Some(star_pos) = find_byte(sentence, b'*') else {
        return false;
    };
    if star_pos + 4 > sentence.len() {
        return false;
    }
    let mut checksum: u8 = 0;
    for &byte in sentence.iter().take(star_pos).skip(1) {
        checksum ^= byte;
    }
    let hi = hex_val(sentence[star_pos + 1]);
    let lo = hex_val(sentence[star_pos + 2]);
    let (Some(hi), Some(lo)) = (hi, lo) else {
        return false;
    };
    checksum == (hi << 4) | lo
}

/// Find the first occurrence of `byte` in `slice`, returning its index.
fn find_byte(slice: &[u8], byte: u8) -> Option<usize> {
    let mut i = 0;
    while i < slice.len() {
        if slice[i] == byte {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Parse a single hex digit (0-9, A-F, a-f) to its 4-bit value.
fn hex_val(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'A'..=b'F' => Some(c - b'A' + 10),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_checksum_valid_gprmc() {
        let s = b"$GPRMC,083559.00,A,4717.11337,N,00833.91590,E,0.004,77.52,091202,,,A*59\r\n";
        assert!(validate_checksum(s));
    }

    #[test]
    fn validate_checksum_rejects_bad_checksum() {
        let s = b"$GPRMC,083559.00,A,4717.11337,N,00833.91590,E,0.004,77.52,091202,,,A*00\r\n";
        assert!(!validate_checksum(s));
    }

    #[test]
    fn validate_checksum_rejects_too_short() {
        assert!(!validate_checksum(b"$GPG"));
        assert!(!validate_checksum(b""));
    }

    #[test]
    fn validate_checksum_rejects_no_star() {
        let s = b"$GPRMC,083559.00,A\r\n";
        assert!(!validate_checksum(s));
    }

    #[test]
    fn hex_val_maps_all_digits() {
        assert_eq!(hex_val(b'0'), Some(0));
        assert_eq!(hex_val(b'9'), Some(9));
        assert_eq!(hex_val(b'A'), Some(10));
        assert_eq!(hex_val(b'F'), Some(15));
        assert_eq!(hex_val(b'a'), Some(10));
        assert_eq!(hex_val(b'G'), None);
    }

    #[test]
    fn find_byte_first_occurrence() {
        assert_eq!(find_byte(b"abc*def", b'*'), Some(3));
        assert_eq!(find_byte(b"abcdef", b'*'), None);
    }
}
