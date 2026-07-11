use crate::error::AppError;

pub const SENTINEL_NO_DATA: u16 = 0x7FFF;

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

pub fn signed_from_u16(raw: u16) -> i16 {
    if raw < 32768 {
        raw as i16
    } else {
        (raw as i32 - 65536) as i16
    }
}

pub fn parameter_from_raw(raw: u16) -> Option<i16> {
    if raw == SENTINEL_NO_DATA {
        None
    } else {
        Some(signed_from_u16(raw))
    }
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
    let rounded = (value * factor).round() as i32;
    to_uint16(rounded * scale.scale_factor as i32, "scaled value")
}

pub fn to_uint16(value: i32, label: &str) -> Result<u16, AppError> {
    if !(-32768..=65535).contains(&value) {
        return Err(AppError::out_of_range(
            label,
            value as f64,
            -32768.0,
            65535.0,
        ));
    }

    if value < 0 {
        Ok((value + 65536) as u16)
    } else {
        Ok(value as u16)
    }
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
    to_uint16((seconds * 10.0).round() as i32, "PID D")
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

    #[test]
    fn converts_to_uint16_after_range_check() {
        assert_eq!(to_uint16(100, "x").unwrap(), 100);
        assert_eq!(to_uint16(0, "x").unwrap(), 0);
        assert_eq!(to_uint16(-1, "x").unwrap(), 65535);
        assert_eq!(to_uint16(-200, "x").unwrap(), 65336);
        assert_eq!(to_uint16(-32768, "x").unwrap(), 32768);
        assert_eq!(to_uint16(65535, "x").unwrap(), 65535);
        assert!(matches!(
            to_uint16(65536, "x"),
            Err(AppError::OutOfRange { .. })
        ));
        assert!(matches!(
            to_uint16(-32769, "x"),
            Err(AppError::OutOfRange { .. })
        ));
        assert!(write_scaled(f64::NAN, ScaleConfig::default()).is_err());
        assert!(write_scaled(f64::INFINITY, ScaleConfig::default()).is_err());
        assert!(d_seconds_to_raw(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn interprets_signed_values_and_sentinel() {
        assert_eq!(parameter_from_raw(100), Some(100));
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
