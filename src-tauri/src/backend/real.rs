use std::time::Duration;

use async_trait::async_trait;
use tokio::time::timeout;
use tokio_modbus::client::{Client, Reader, Writer};
use tokio_modbus::{client::rtu, Slave};
use tokio_serial::{DataBits, Parity, SerialStream, StopBits};

use crate::backend::DeviceBackend;
use crate::error::AppError;
use crate::types::ConnectionConfig;

const REQUEST_TIMEOUT: Duration = Duration::from_millis(300);

#[derive(Default)]
pub struct RealModbusBackend {
    ctx: Option<tokio_modbus::client::Context>,
}

#[async_trait]
impl DeviceBackend for RealModbusBackend {
    async fn connect(&mut self, cfg: &ConnectionConfig) -> Result<(), AppError> {
        let builder = tokio_serial::new(&cfg.port, cfg.baudrate)
            .data_bits(DataBits::Eight)
            .stop_bits(StopBits::Two)
            .parity(Parity::None)
            .timeout(REQUEST_TIMEOUT);
        let stream =
            SerialStream::open(&builder).map_err(|err| AppError::Serial(err.to_string()))?;
        self.ctx = Some(rtu::attach_slave(stream, Slave(cfg.slave_addr)));
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        if let Some(mut ctx) = self.ctx.take() {
            ctx.disconnect()
                .await
                .map_err(|err| AppError::Serial(err.to_string()))?;
        }
        Ok(())
    }

    async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError> {
        let ctx = self.ctx.as_mut().ok_or(AppError::NotConnected)?;
        let result = timeout(REQUEST_TIMEOUT, ctx.read_holding_registers(addr, count))
            .await
            .map_err(|_| AppError::Timeout)?
            .map_err(|err| AppError::Modbus(err.to_string()))?;
        result.map_err(|err| AppError::Modbus(format!("exception code: {err:?}")))
    }

    async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError> {
        let ctx = self.ctx.as_mut().ok_or(AppError::NotConnected)?;
        let result = timeout(REQUEST_TIMEOUT, ctx.write_single_register(addr, value))
            .await
            .map_err(|_| AppError::Timeout)?
            .map_err(|err| AppError::Modbus(err.to_string()))?;
        result.map_err(|err| AppError::Modbus(format!("exception code: {err:?}")))
    }
}
