use std::collections::HashMap;

use async_trait::async_trait;

use crate::backend::DeviceBackend;
use crate::error::AppError;
use crate::modbus::registers;
use crate::types::ConnectionConfig;

pub struct MockBackend {
    registers: HashMap<u16, u16>,
    connected: bool,
    offline: bool,
}

impl MockBackend {
    pub fn normal() -> Self {
        let mut registers = HashMap::new();
        registers.insert(registers::MODEL, registers::MODEL_AI_516P);
        registers.insert(registers::DPT, 1);
        registers.insert(registers::PV, 1005);
        registers.insert(registers::SV, 1000);
        registers.insert(registers::MV_READ, 12800);
        registers.insert(registers::SP1, 1000);
        registers.insert(registers::P, 120);
        registers.insert(registers::I, 300);
        registers.insert(registers::D, 45);
        registers.insert(registers::PNO, 3);
        registers.insert(registers::SP_START, 1000);
        registers.insert(registers::SP_START + 1, 20);
        registers.insert(registers::SP_START + 2, 1500);
        registers.insert(registers::SP_START + 3, 30);
        registers.insert(registers::SP_START + 4, 800);
        registers.insert(registers::SP_START + 5, 15);

        Self {
            registers,
            connected: false,
            offline: false,
        }
    }

    pub fn offline() -> Self {
        let mut backend = Self::normal();
        backend.offline = true;
        backend
    }

    fn ensure_ready(&self) -> Result<(), AppError> {
        if !self.connected {
            return Err(AppError::NotConnected);
        }
        if self.offline {
            return Err(AppError::Timeout);
        }
        Ok(())
    }
}

#[async_trait]
impl DeviceBackend for MockBackend {
    async fn connect(&mut self, _cfg: &ConnectionConfig) -> Result<(), AppError> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        self.connected = false;
        Ok(())
    }

    async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError> {
        self.ensure_ready()?;

        Ok((0..count)
            .map(|offset| self.registers.get(&(addr + offset)).copied().unwrap_or(0))
            .collect())
    }

    async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError> {
        self.ensure_ready()?;
        self.registers.insert(addr, value);
        Ok(())
    }
}
