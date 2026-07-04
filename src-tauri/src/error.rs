use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "camelCase")]
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
}
