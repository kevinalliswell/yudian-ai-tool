use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PortInfo {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionConfig {
    pub port: String,
    pub slave_addr: u8,
    pub baudrate: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub connected: bool,
    pub model_code: Option<u16>,
    pub model_name: Option<String>,
    pub decimal_point: u8,
    pub scale_factor: u8,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            connected: false,
            model_code: None,
            model_name: None,
            decimal_point: 1,
            scale_factor: 1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PidValues {
    pub p: f64,
    pub i: u32,
    pub d: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub temperature: f64,
    pub minutes: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum RunStatus {
    Run,
    Hold,
    Stop,
}

impl RunStatus {
    pub fn register_value(&self) -> u16 {
        match self {
            RunStatus::Run => 0,
            RunStatus::Stop => 1,
            RunStatus::Hold => 2,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    pub pv: Option<f64>,
    pub sv: Option<f64>,
    pub mv: Option<f64>,
    pub ts: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusEvent {
    pub connected: bool,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorEvent {
    pub scope: String,
    pub message: String,
}
