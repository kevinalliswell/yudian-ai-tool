use std::ops::RangeInclusive;

use crate::error::AppError;

pub const SENTINEL_NO_DATA: u16 = 0x7FFF;

/// Reads whose high byte is 127 mark an invalid or reserved parameter code
/// (protocol V8.x); the controller never stores values in this band.
pub const INVALID_READ_RANGE: RangeInclusive<u16> = 32512..=0x7FFF;

/// Registers hold 16-bit two's complement values, and the controller accepts
/// at most 32000 for any parameter. Staying within this range guarantees a
/// written value reads back unchanged.
pub const WRITABLE_RANGE: RangeInclusive<i64> = i16::MIN as i64..=32000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScaleConfig {
    pub decimal_point: u8,
    pub scale_factor: u8,
}

impl Default for ScaleConfig {
    fn default() -> Self {
        Self {
            decimal_point: 1,
            scale_factor: 1,
        }
    }
}

pub fn parse_dpt(raw_dpt: Option<u16>) -> ScaleConfig {
    match raw_dpt {
        Some(raw) if raw >= 128 => ScaleConfig {
            decimal_point: (raw - 128) as u8,
            scale_factor: 10,
        },
        Some(raw) => ScaleConfig {
            decimal_point: raw as u8,
            scale_factor: 1,
        },
        None => ScaleConfig::default(),
    }
}

pub fn parameter_from_raw(raw: u16) -> Option<i16> {
    if INVALID_READ_RANGE.contains(&raw) {
        None
    } else {
        Some(raw as i16)
    }
}

/// Encodes a register value, rejecting anything the controller would store
/// differently from what was written (no truncation, no sign flips).
pub fn encode_i16(value: i64, label: &str) -> Result<u16, AppError> {
    if !WRITABLE_RANGE.contains(&value) {
        return Err(AppError::out_of_range(
            label,
            value as f64,
            *WRITABLE_RANGE.start() as f64,
            *WRITABLE_RANGE.end() as f64,
        ));
    }
    Ok(value as i16 as u16)
}

pub fn read_scaled(raw: u16, scale: ScaleConfig) -> Option<f64> {
    let signed = parameter_from_raw(raw)? as f64;
    let factor = 10_f64.powi(scale.decimal_point as i32);
    Some(signed / scale.scale_factor as f64 / factor)
}

pub fn write_scaled(value: f64, scale: ScaleConfig) -> Result<u16, AppError> {
    if !value.is_finite() {
        return Err(AppError::InvalidData(
            "scaled value must be finite".to_string(),
        ));
    }
    let factor = 10_f64.powi(scale.decimal_point as i32);
    // Float-to-int `as` saturates, so absurd inputs stay out of range below.
    let rounded = (value * factor).round() as i64;
    encode_i16(
        rounded.saturating_mul(i64::from(scale.scale_factor)),
        "scaled value",
    )
}

pub fn mv_percent(raw: u16) -> f64 {
    raw as f64 / 256.0
}

pub fn d_seconds_from_raw(raw: u16) -> Option<f64> {
    parameter_from_raw(raw).map(|value| value as f64 * 0.1)
}

pub fn d_seconds_to_raw(seconds: f64) -> Result<u16, AppError> {
    if !seconds.is_finite() {
        return Err(AppError::InvalidData("PID D must be finite".to_string()));
    }
    encode_i16((seconds * 10.0).round() as i64, "PID D")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_scaled_values_from_testing_table() {
        let cases = [
            (1, 1, 123.4, 1234, 123.4),
            (1, 10, 123.4, 12340, 123.4),
            (0, 1, 100.0, 100, 100.0),
            (2, 1, 12.34, 1234, 12.34),
        ];

        for (decimal_point, scale_factor, actual, raw, expected_actual) in cases {
            let scale = ScaleConfig {
                decimal_point,
                scale_factor,
            };
            assert_eq!(write_scaled(actual, scale).unwrap(), raw);
            assert_eq!(read_scaled(raw, scale).unwrap(), expected_actual);
        }
    }

    #[test]
    fn parses_dpt_with_default_fallback() {
        assert_eq!(
            parse_dpt(Some(0)),
            ScaleConfig {
                decimal_point: 0,
                scale_factor: 1
            }
        );
        assert_eq!(
            parse_dpt(Some(1)),
            ScaleConfig {
                decimal_point: 1,
                scale_factor: 1
            }
        );
        assert_eq!(
            parse_dpt(Some(2)),
            ScaleConfig {
                decimal_point: 2,
                scale_factor: 1
            }
        );
        assert_eq!(
            parse_dpt(Some(129)),
            ScaleConfig {
                decimal_point: 1,
                scale_factor: 10
            }
        );
        assert_eq!(
            parse_dpt(Some(130)),
            ScaleConfig {
                decimal_point: 2,
                scale_factor: 10
            }
        );
        assert_eq!(
            parse_dpt(None),
            ScaleConfig {
                decimal_point: 1,
                scale_factor: 1
            }
        );
    }

    fn is_out_of_range(result: Result<u16, AppError>) -> bool {
        matches!(result, Err(AppError::OutOfRange { .. }))
    }

    #[test]
    fn encodes_signed_values_within_the_writable_range() {
        assert_eq!(encode_i16(100, "x").unwrap(), 100);
        assert_eq!(encode_i16(0, "x").unwrap(), 0);
        assert_eq!(encode_i16(-1, "x").unwrap(), 65535);
        assert_eq!(encode_i16(-200, "x").unwrap(), 65336);
        assert_eq!(encode_i16(-32768, "x").unwrap(), 32768);
        assert_eq!(encode_i16(32000, "x").unwrap(), 32000);
    }

    #[test]
    fn rejects_values_the_device_would_read_back_differently() {
        // 32001..=32767 overlaps the invalid-parameter marker range and
        // anything above 32767 would be read back as a negative number.
        assert!(is_out_of_range(encode_i16(32001, "x")));
        assert!(is_out_of_range(encode_i16(32767, "x")));
        assert!(is_out_of_range(encode_i16(40000, "x")));
        assert!(is_out_of_range(encode_i16(65535, "x")));
        assert!(is_out_of_range(encode_i16(-32769, "x")));
    }

    #[test]
    fn every_encodable_value_round_trips_through_a_read() {
        for value in [-32768, -9990, -1, 0, 1, 12345, 32000] {
            let raw = encode_i16(value, "x").unwrap();
            assert_eq!(parameter_from_raw(raw).map(i64::from), Some(value));
        }
    }

    #[test]
    fn scaled_writes_reject_values_beyond_the_register() {
        let two_decimals = ScaleConfig {
            decimal_point: 2,
            scale_factor: 1,
        };
        // 400.00 would encode as 40000, which the device reads as -255.36.
        assert!(is_out_of_range(write_scaled(400.0, two_decimals)));
        assert_eq!(write_scaled(320.0, two_decimals).unwrap(), 32000);
        assert!(is_out_of_range(write_scaled(1e300, ScaleConfig::default())));
        assert!(write_scaled(f64::NAN, ScaleConfig::default()).is_err());
        assert!(write_scaled(f64::INFINITY, ScaleConfig::default()).is_err());
        assert!(d_seconds_to_raw(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn interprets_signed_values_and_invalid_markers() {
        assert_eq!(parameter_from_raw(100), Some(100));
        assert_eq!(parameter_from_raw(32000), Some(32000));
        assert_eq!(parameter_from_raw(32511), Some(32511));
        assert_eq!(parameter_from_raw(32512), None);
        assert_eq!(parameter_from_raw(32767), None);
        assert_eq!(parameter_from_raw(32768), Some(-32768));
        assert_eq!(parameter_from_raw(65535), Some(-1));
    }

    #[test]
    fn converts_mv_percent() {
        assert_eq!(mv_percent(12800), 50.0);
    }

    #[test]
    fn pid_d_is_symmetric_in_tenths_of_a_second() {
        assert_eq!(d_seconds_from_raw(123).unwrap(), 12.3);
        assert_eq!(d_seconds_to_raw(12.3).unwrap(), 123);
    }
}
