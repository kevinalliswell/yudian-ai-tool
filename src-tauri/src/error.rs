use std::fmt;

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

/// Why writes are disabled for the connected device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ReadOnlyReason {
    DptUnavailable,
    UnsupportedModel,
}

impl fmt::Display for ReadOnlyReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DptUnavailable => "DPT could not be read",
            Self::UnsupportedModel => "its model is not supported",
        })
    }
}

/// Which safety precondition refused a run command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RunBlocker {
    UnsupportedModel,
    CurveNotVerified,
    InvalidPv,
    InvalidSv,
}

impl fmt::Display for RunBlocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsupportedModel => "run requires a supported device model",
            Self::CurveNotVerified => "run requires a verified curve download",
            Self::InvalidPv => "run requires valid PV data within the temperature range",
            Self::InvalidSv => "run requires valid SV data within the temperature range",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WriteOperation {
    Pid,
    Curve,
}

impl fmt::Display for WriteOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pid => "PID write",
            Self::Curve => "curve download",
        })
    }
}

/// Result of restoring the previous values after a failed write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackOutcome {
    Succeeded,
    Failed(String),
}

impl RollbackOutcome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Succeeded => "succeeded",
            Self::Failed(_) => "failed",
        }
    }
}

impl fmt::Display for RollbackOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Succeeded => f.write_str("rollback succeeded"),
            Self::Failed(error) => write!(f, "rollback failed: {error}"),
        }
    }
}

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

    #[error("device is busy with a write transaction")]
    Busy,

    #[error("a program is running; hold (HoLd) or stop it before downloading a curve")]
    DeviceRunning,

    /// The caller stopped waiting while a write transaction was still
    /// running; the device may hold the old or the new values.
    #[error("write outcome unknown: {0}")]
    OutcomeUnknown(String),

    #[error("device is read-only because {reason}")]
    ReadOnly { reason: ReadOnlyReason },

    #[error("{reason}")]
    RunBlocked { reason: RunBlocker },

    /// A multi-register write failed after it started changing the device;
    /// `rollback` reports whether the previous values were restored.
    #[error("{operation} failed: {cause}; {rollback}")]
    WriteFailed {
        operation: WriteOperation,
        cause: String,
        rollback: RollbackOutcome,
    },
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
            AppError::Busy => "busy",
            AppError::DeviceRunning => "deviceRunning",
            AppError::OutcomeUnknown(_) => "outcomeUnknown",
            AppError::ReadOnly { .. } => "readOnly",
            AppError::RunBlocked { .. } => "runBlocked",
            AppError::WriteFailed { .. } => "writeFailed",
        }
    }
}

// Hand-written so every variant serializes to a flat `{ kind, message, ... }`.
// A derived internally-tagged enum cannot serialize the `String` newtype
// variants (Serial/Modbus/Backend/InvalidData); it fails at runtime, so the
// frontend never receives the real error for the most common failures
// (bad port, modbus read/write errors, invalid register data).
//
// `message` is English and meant for logs; the frontend localizes by `kind`
// plus the structured fields added for the variants below.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("AppError", 6)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.to_string())?;
        match self {
            AppError::OutOfRange {
                label,
                value,
                min,
                max,
            } => {
                state.serialize_field("label", label)?;
                state.serialize_field("value", value)?;
                state.serialize_field("min", min)?;
                state.serialize_field("max", max)?;
            }
            AppError::ReadOnly { reason } => state.serialize_field("reason", reason)?,
            AppError::RunBlocked { reason } => state.serialize_field("reason", reason)?,
            AppError::WriteFailed {
                operation,
                rollback,
                ..
            } => {
                state.serialize_field("operation", operation)?;
                state.serialize_field("rollback", rollback.as_str())?;
            }
            _ => {}
        }
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
            AppError::Busy,
            AppError::DeviceRunning,
            AppError::OutcomeUnknown("curve download still running".to_string()),
            AppError::ReadOnly {
                reason: ReadOnlyReason::DptUnavailable,
            },
            AppError::RunBlocked {
                reason: RunBlocker::CurveNotVerified,
            },
            AppError::WriteFailed {
                operation: WriteOperation::Curve,
                cause: "segment 1 read-back mismatch".to_string(),
                rollback: RollbackOutcome::Succeeded,
            },
        ];

        for error in cases {
            let value = serialized(&error);
            assert_eq!(value["kind"], error.kind());
            assert_eq!(value["message"], error.to_string());
        }
    }

    #[test]
    fn structured_variants_expose_machine_readable_fields() {
        let range = serialized(&AppError::out_of_range("PID D", 400.0, 0.0, 320.0));
        assert_eq!(range["label"], "PID D");
        assert_eq!(range["value"], 400.0);
        assert_eq!(range["max"], 320.0);

        let read_only = serialized(&AppError::ReadOnly {
            reason: ReadOnlyReason::UnsupportedModel,
        });
        assert_eq!(read_only["kind"], "readOnly");
        assert_eq!(read_only["reason"], "unsupportedModel");

        let blocked = serialized(&AppError::RunBlocked {
            reason: RunBlocker::InvalidPv,
        });
        assert_eq!(blocked["kind"], "runBlocked");
        assert_eq!(blocked["reason"], "invalidPv");

        let failed = serialized(&AppError::WriteFailed {
            operation: WriteOperation::Pid,
            cause: "timeout".to_string(),
            rollback: RollbackOutcome::Failed("timeout".to_string()),
        });
        assert_eq!(failed["kind"], "writeFailed");
        assert_eq!(failed["operation"], "pid");
        assert_eq!(failed["rollback"], "failed");
        assert_eq!(
            failed["message"],
            "PID write failed: timeout; rollback failed: timeout"
        );
    }

    #[test]
    fn newtype_variants_carry_the_display_message() {
        let value = serialized(&AppError::Serial("port busy".to_string()));
        assert_eq!(value["kind"], "serial");
        assert_eq!(value["message"], "serial error: port busy");
    }
}
