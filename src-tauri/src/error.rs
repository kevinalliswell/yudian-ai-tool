use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum AppError {
    #[error("device is not connected")]
    NotConnected,

    #[error("operation timed out")]
    Timeout,

    #[error("{label} out of range: {value}, expected {min}..={max}")]
    OutOfRange {
        label: String,
        value: f64,
        min: f64,
        max: f64,
    },

    #[error("serial error: {0}")]
    Serial(String),

    #[error("modbus error: {0}")]
    Modbus(String),

    #[error("backend error: {0}")]
    Backend(String),

    #[error("invalid data: {0}")]
    InvalidData(String),
}

impl AppError {
    pub fn out_of_range(label: impl Into<String>, value: f64, min: f64, max: f64) -> Self {
        Self::OutOfRange {
            label: label.into(),
            value,
            min,
            max,
        }
    }

    /// Stable, camelCase discriminant the frontend can switch on.
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::NotConnected => "notConnected",
            AppError::Timeout => "timeout",
            AppError::OutOfRange { .. } => "outOfRange",
            AppError::Serial(_) => "serial",
            AppError::Modbus(_) => "modbus",
            AppError::Backend(_) => "backend",
            AppError::InvalidData(_) => "invalidData",
        }
    }
}

// Hand-written so every variant serializes to a flat `{ kind, message }`.
// A derived internally-tagged enum cannot serialize the `String` newtype
// variants (Serial/Modbus/Backend/InvalidData); it fails at runtime, so the
// frontend never receives the real error for the most common failures
// (bad port, modbus read/write errors, invalid register data).
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn serialized(error: &AppError) -> serde_json::Value {
        serde_json::to_value(error).expect("AppError must serialize")
    }

    #[test]
    fn every_variant_serializes_to_kind_and_message() {
        let cases = [
            AppError::NotConnected,
            AppError::Timeout,
            AppError::out_of_range("temperature", 5.0, 0.0, 2.0),
            AppError::Serial("port busy".to_string()),
            AppError::Modbus("crc mismatch".to_string()),
            AppError::Backend("actor down".to_string()),
            AppError::InvalidData("PID P has no valid data".to_string()),
        ];

        for error in cases {
            let value = serialized(&error);
            assert_eq!(value["kind"], error.kind());
            assert_eq!(value["message"], error.to_string());
        }
    }

    #[test]
    fn newtype_variants_carry_the_display_message() {
        let value = serialized(&AppError::Serial("port busy".to_string()));
        assert_eq!(value["kind"], "serial");
        assert_eq!(value["message"], "serial error: port busy");
    }
}
