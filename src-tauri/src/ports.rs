use crate::error::AppError;
use crate::types::PortInfo;

pub fn list_serial_ports() -> Result<Vec<PortInfo>, AppError> {
    let ports = serialport::available_ports().map_err(|err| AppError::Serial(err.to_string()))?;

    Ok(ports
        .into_iter()
        .map(|port| PortInfo {
            name: port.port_name,
            description: Some(format!("{:?}", port.port_type)),
        })
        .collect())
}
