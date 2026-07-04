use async_trait::async_trait;

use crate::error::AppError;
use crate::types::ConnectionConfig;

pub mod mock;
pub mod real;

#[async_trait]
pub trait DeviceBackend: Send {
    async fn connect(&mut self, cfg: &ConnectionConfig) -> Result<(), AppError>;
    async fn disconnect(&mut self) -> Result<(), AppError>;
    async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError>;
    async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendMode {
    Real,
    Mock,
    MockOffline,
}

impl BackendMode {
    pub fn from_env() -> Self {
        match std::env::var("YUDIAN_BACKEND") {
            Ok(value) if value.eq_ignore_ascii_case("mock") => Self::Mock,
            Ok(value) if value.eq_ignore_ascii_case("mock-offline") => Self::MockOffline,
            _ => Self::Real,
        }
    }
}

pub fn create_backend(mode: BackendMode) -> Box<dyn DeviceBackend> {
    match mode {
        BackendMode::Real => Box::new(real::RealModbusBackend::default()),
        BackendMode::Mock => Box::new(mock::MockBackend::normal()),
        BackendMode::MockOffline => Box::new(mock::MockBackend::offline()),
    }
}
