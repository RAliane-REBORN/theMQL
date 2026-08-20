//! Embedded MQTT publisher via `minimq` (MQTT v5, no_std, async).
//!
//! Provides a thin wrapper around `minimq::Session` for publishing
//! telemetry to the four subjects defined in
//! `specs/embedded.toml [tasks.telemetry]`. The caller owns the packet
//! buffers (`Buffers::new(rx, tx)`) and the transport (any type
//! implementing `minimq::Io` — typically `embassy-net` TCP sockets).
//!
//! ## TETANUS compliance
//!
//! All buffer sizes are fixed at compile time. No `unwrap`/`expect` in
//! the hot path. Functions ≤ 60 lines. No dynamic allocation after init.

#[cfg(any(target_os = "none", test))]
use crate::sensors::types::heap_string;
#[cfg(any(target_os = "none", test))]
use crate::SensorError;

/// TX buffer size for the MQTT session. Must accommodate the largest
/// outbound packet (PUBLISH with topic + JSON payload ≤ 256 bytes +
/// MQTT v5 header overhead).
pub const MQTT_TX_BUF_SIZE: usize = 1024;

/// RX buffer size for the MQTT session. Must accommodate the largest
/// inbound packet (SUBACK, PUBACK, etc. — typically small).
pub const MQTT_RX_BUF_SIZE: usize = 512;

/// Fixed-capacity JSON payload buffer for telemetry messages.
/// Hand-formatted (no `serde_json`) to keep deps minimal per TETANUS.
pub const PAYLOAD_BUF_SIZE: usize = 256;

/// Construct a minimq `ConfigBuilder` with caller-owned buffers and
/// the given client id. The caller is responsible for providing the
/// transport (`minimq::Io` impl) when calling `Session::connect`.
///
/// # Errors
/// Returns [`SensorError::InvalidReading`] if the client id is empty
/// or the config is invalid.
#[cfg(any(target_os = "none", test))]
#[allow(clippy::module_name_repetitions)]
pub fn build_mqtt_config<'a>(
    rx: &'a mut [u8],
    tx: &'a mut [u8],
    client_id: &str,
) -> Result<minimq::ConfigBuilder<'a>, SensorError> {
    if rx.len() < MQTT_RX_BUF_SIZE || tx.len() < MQTT_TX_BUF_SIZE {
        return Err(SensorError::InvalidReading(heap_string(
            "mqtt buffer too small",
        )));
    }
    let buffers = minimq::Buffers::new(rx, tx);
    let config = minimq::ConfigBuilder::new(buffers)
        .client_id(client_id)
        .map_err(|_| SensorError::InvalidReading(heap_string("mqtt config")))?;
    Ok(config)
}

/// Format a telemetry JSON payload for the `vehicle.state_estimate`
/// subject. Hand-formatted to avoid `serde_json_core` (minimal deps).
/// Returns the number of bytes written to `buf`.
///
/// # Errors
/// Returns 0 if the buffer is too small (caller should skip this publish).
#[must_use]
pub fn format_state_estimate(buf: &mut [u8], pos: [f64; 3], vel: [f64; 3]) -> usize {
    if buf.len() < 32 {
        return 0;
    }
    let mut len = 0usize;
    len += write_str(buf, len, b"{\"pos\":");
    len += write_f64_array(buf, len, &pos);
    len += write_str(buf, len, b",\"vel\":");
    len += write_f64_array(buf, len, &vel);
    len += write_str(buf, len, b"}");
    len.min(buf.len())
}

/// Format a telemetry JSON payload for `vehicle.diagnostics`.
#[must_use]
pub fn format_diagnostics(buf: &mut [u8], cycle_count: u32, error_count: u32) -> usize {
    if buf.len() < 32 {
        return 0;
    }
    let mut len = 0usize;
    len += write_str(buf, len, b"{\"cycles\":");
    len += write_u32(buf, len, cycle_count);
    len += write_str(buf, len, b",\"errors\":");
    len += write_u32(buf, len, error_count);
    len += write_str(buf, len, b"}");
    len.min(buf.len())
}

/// Write a byte slice into `buf` at `offset`, returning the new offset.
/// Does nothing if the write would exceed the buffer.
fn write_str(buf: &mut [u8], offset: usize, s: &[u8]) -> usize {
    if offset >= buf.len() {
        return offset;
    }
    let end = offset.saturating_add(s.len());
    if end > buf.len() {
        return offset;
    }
    buf[offset..end].copy_from_slice(s);
    end
}

/// Write a `[f64; 3]` as a JSON array into `buf` at `offset`.
fn write_f64_array(buf: &mut [u8], offset: usize, arr: &[f64; 3]) -> usize {
    let mut len = write_str(buf, offset, b"[");
    for (i, val) in arr.iter().enumerate() {
        if i > 0 {
            len = write_str(buf, len, b",");
        }
        len = write_f64(buf, len, *val);
    }
    write_str(buf, len, b"]")
}

/// Write a single `f64` as a decimal string into `buf` at `offset`.
fn write_f64(buf: &mut [u8], offset: usize, val: f64) -> usize {
    if offset >= buf.len() {
        return offset;
    }
    let neg = val < 0.0;
    let abs_val = if neg { -val } else { val };
    let int_part = abs_val as u64;
    let frac = ((abs_val - int_part as f64) * 1_000_000.0) as u64;
    let mut pos = offset;
    if neg && pos < buf.len() {
        buf[pos] = b'-';
        pos += 1;
    }
    pos = write_u64_dec(buf, pos, int_part);
    if pos < buf.len() {
        buf[pos] = b'.';
        pos += 1;
    }
    pos = write_u64_padded(buf, pos, frac, 6);
    if pos > buf.len() {
        pos = buf.len();
    }
    pos
}

/// Write a `u64` as decimal at `offset`. Returns new offset.
fn write_u64_dec(buf: &mut [u8], offset: usize, val: u64) -> usize {
    if offset >= buf.len() {
        return offset;
    }
    if val == 0 {
        buf[offset] = b'0';
        return offset + 1;
    }
    let mut tmp = [0u8; 20];
    let mut len = 0;
    let mut v = val;
    while v > 0 && len < 20 {
        tmp[len] = b'0' + (v % 10) as u8;
        v /= 10;
        len += 1;
    }
    let remaining = buf.len() - offset;
    let copy_len = len.min(remaining);
    for i in 0..copy_len {
        buf[offset + i] = tmp[len - 1 - i];
    }
    offset + copy_len
}

/// Write a `u64` zero-padded to `width` digits at `offset`.
fn write_u64_padded(buf: &mut [u8], offset: usize, val: u64, width: usize) -> usize {
    if offset >= buf.len() {
        return offset;
    }
    let mut tmp = [0u8; 20];
    let mut len = 0;
    let mut v = val;
    while v > 0 && len < 20 {
        tmp[len] = b'0' + (v % 10) as u8;
        v /= 10;
        len += 1;
    }
    let padded_len = width.max(len);
    let remaining = buf.len() - offset;
    let copy_len = padded_len.min(remaining);
    for i in 0..copy_len {
        if i < padded_len - len {
            buf[offset + i] = b'0';
        } else {
            buf[offset + i] = tmp[len - 1 - (i - (padded_len - len))];
        }
    }
    offset + copy_len
}

/// Write a `u32` as a decimal string into `buf` at `offset`.
fn write_u32(buf: &mut [u8], offset: usize, val: u32) -> usize {
    write_u64_dec(buf, offset, u64::from(val))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_state_estimate_fits_in_buffer() {
        let mut buf = [0u8; PAYLOAD_BUF_SIZE];
        let len = format_state_estimate(&mut buf, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        assert!(len > 0, "payload must be non-empty");
        assert!(
            len <= PAYLOAD_BUF_SIZE,
            "payload must fit in buffer, got len={len}"
        );
        let s = core::str::from_utf8(&buf[..len]).unwrap_or("");
        assert!(s.contains("\"pos\""), "must contain pos field: {s}");
        assert!(s.contains("\"vel\""), "must contain vel field: {s}");
    }

    #[test]
    fn format_diagnostics_fits_in_buffer() {
        let mut buf = [0u8; 64];
        let len = format_diagnostics(&mut buf, 12345, 2);
        assert!(len > 0, "payload must be non-empty");
        assert!(len <= 64, "payload must fit in 64-byte buffer");
        let s = core::str::from_utf8(&buf[..len]).unwrap_or("");
        assert!(s.contains("\"cycles\""), "must contain cycles: {s}");
    }

    #[test]
    fn format_state_estimate_returns_zero_for_tiny_buffer() {
        let mut buf = [0u8; 10];
        let len = format_state_estimate(&mut buf, [1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        assert_eq!(len, 0);
    }

    #[test]
    fn telemetry_subjects_count_matches_spec() {
        assert_eq!(crate::TELEMETRY_SUBJECTS.len(), 4);
    }
}
