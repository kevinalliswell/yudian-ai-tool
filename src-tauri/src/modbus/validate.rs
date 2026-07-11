use serde::Serialize;

use crate::error::AppError;
use crate::types::Segment;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationLimits {
    pub temp_min: f64,
    pub temp_max: f64,
    pub pid_p_max: f64,
    pub pid_i_max: u32,
    pub pid_d_max: f64,
    pub segment_max_count: usize,
    pub segment_minutes_max: i32,
    pub slave_addr_min: u8,
    pub slave_addr_max: u8,
    pub refresh_interval_min_ms: u32,
}

impl Default for ValidationLimits {
    fn default() -> Self {
        Self {
            temp_min: -200.0,
            temp_max: 1800.0,
            pid_p_max: 9999.9,
            pid_i_max: 9999,
            pid_d_max: 999.9,
            segment_max_count: 50,
            segment_minutes_max: u16::MAX as i32,
            slave_addr_min: 1,
            slave_addr_max: 80,
            refresh_interval_min_ms: 200,
        }
    }
}

pub fn limits() -> ValidationLimits {
    ValidationLimits::default()
}

pub fn validate_temperature(value: f64) -> Result<(), AppError> {
    let limits = limits();
    if !value.is_finite() {
        return Err(AppError::InvalidData(
            "temperature must be finite".to_string(),
        ));
    }
    if value < limits.temp_min || value > limits.temp_max {
        Err(AppError::out_of_range(
            "temperature",
            value,
            limits.temp_min,
            limits.temp_max,
        ))
    } else {
        Ok(())
    }
}

pub fn validate_pid(p: f64, i: u32, d: f64) -> Result<(), AppError> {
    let limits = limits();
    if !p.is_finite() {
        return Err(AppError::InvalidData("PID P must be finite".to_string()));
    }
    if !d.is_finite() {
        return Err(AppError::InvalidData("PID D must be finite".to_string()));
    }
    if p < 0.0 || p > limits.pid_p_max {
        return Err(AppError::out_of_range("PID P", p, 0.0, limits.pid_p_max));
    }
    if i > limits.pid_i_max {
        return Err(AppError::out_of_range(
            "PID I",
            i as f64,
            0.0,
            limits.pid_i_max as f64,
        ));
    }
    if d < 0.0 || d > limits.pid_d_max {
        return Err(AppError::out_of_range("PID D", d, 0.0, limits.pid_d_max));
    }
    Ok(())
}

pub fn validate_slave_addr(value: u8) -> Result<(), AppError> {
    let limits = limits();
    if value < limits.slave_addr_min || value > limits.slave_addr_max {
        Err(AppError::out_of_range(
            "slave address",
            value as f64,
            limits.slave_addr_min as f64,
            limits.slave_addr_max as f64,
        ))
    } else {
        Ok(())
    }
}

pub fn validate_segments(segments: &[Segment]) -> Result<(), AppError> {
    let limits = limits();
    if segments.is_empty() {
        return Err(AppError::InvalidData(
            "curve must contain at least one segment".to_string(),
        ));
    }
    if segments.len() > limits.segment_max_count {
        return Err(AppError::out_of_range(
            "segment count",
            segments.len() as f64,
            1.0,
            limits.segment_max_count as f64,
        ));
    }
    for (index, segment) in segments.iter().enumerate() {
        validate_temperature(segment.temperature).map_err(|_| {
            AppError::out_of_range(
                format!("segment {index} temperature"),
                segment.temperature,
                limits.temp_min,
                limits.temp_max,
            )
        })?;
        if segment.minutes < 0 || segment.minutes > limits.segment_minutes_max {
            return Err(AppError::out_of_range(
                format!("segment {index} minutes"),
                segment.minutes as f64,
                0.0,
                limits.segment_minutes_max as f64,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segment() -> Segment {
        Segment {
            temperature: 100.0,
            minutes: 20,
        }
    }

    #[test]
    fn validates_temperature_boundaries() {
        assert!(validate_temperature(-200.0).is_ok());
        assert!(validate_temperature(1800.0).is_ok());
        assert!(validate_temperature(-200.1).is_err());
        assert!(validate_temperature(1800.1).is_err());
        assert!(validate_temperature(f64::NAN).is_err());
        assert!(validate_temperature(f64::INFINITY).is_err());
        assert!(validate_temperature(f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn validates_curve_segments() {
        assert!(validate_segments(&[]).is_err());

        let fifty = vec![segment(); 50];
        assert!(validate_segments(&fifty).is_ok());

        let fifty_one = vec![segment(); 51];
        assert!(validate_segments(&fifty_one).is_err());

        let invalid_temp = [Segment {
            temperature: 1800.1,
            minutes: 0,
        }];
        assert!(validate_segments(&invalid_temp).is_err());

        let non_finite_temp = [Segment {
            temperature: f64::NAN,
            minutes: 0,
        }];
        assert!(validate_segments(&non_finite_temp).is_err());

        let negative_minutes = [Segment {
            temperature: 100.0,
            minutes: -1,
        }];
        assert!(validate_segments(&negative_minutes).is_err());

        let max_minutes = [Segment {
            temperature: 100.0,
            minutes: u16::MAX as i32,
        }];
        assert!(validate_segments(&max_minutes).is_ok());

        let excessive_minutes = [Segment {
            temperature: 100.0,
            minutes: u16::MAX as i32 + 1,
        }];
        assert!(validate_segments(&excessive_minutes).is_err());
    }

    #[test]
    fn validates_pid_finite_values() {
        assert!(validate_pid(f64::NAN, 0, 0.0).is_err());
        assert!(validate_pid(0.0, 0, f64::INFINITY).is_err());
        assert!(validate_pid(0.0, 0, f64::NEG_INFINITY).is_err());
    }

    #[test]
    fn validates_slave_address_boundaries() {
        assert!(validate_slave_addr(1).is_ok());
        assert!(validate_slave_addr(80).is_ok());
        assert!(validate_slave_addr(0).is_err());
        assert!(validate_slave_addr(81).is_err());
    }
}
