use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

use crate::backend::{create_backend, BackendMode, DeviceBackend};
use crate::error::AppError;
use crate::modbus::{convert, registers, validate};
use crate::types::{ConnectionConfig, DeviceInfo, PidValues, Reading, RunStatus, Segment};

#[derive(Clone)]
pub struct DeviceHandle {
    tx: mpsc::Sender<DeviceRequest>,
}

impl DeviceHandle {
    pub fn spawn(mode: BackendMode) -> Self {
        let (tx, rx) = mpsc::channel(64);
        let actor = DeviceActor::new(mode, rx);
        tauri::async_runtime::spawn(actor.run());
        Self { tx }
    }

    async fn request<T>(
        &self,
        build: impl FnOnce(oneshot::Sender<Result<T, AppError>>) -> DeviceRequest,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        self.tx
            .send(build(reply_tx))
            .await
            .map_err(|_| AppError::Backend("device actor is not running".to_string()))?;
        reply_rx
            .await
            .map_err(|_| AppError::Backend("device actor dropped response".to_string()))?
    }

    pub async fn connect(&self, cfg: ConnectionConfig) -> Result<DeviceInfo, AppError> {
        self.request(|reply| DeviceRequest::Connect { cfg, reply })
            .await
    }

    pub async fn disconnect(&self) -> Result<(), AppError> {
        self.request(|reply| DeviceRequest::Disconnect { reply })
            .await
    }

    pub async fn get_info(&self) -> Result<DeviceInfo, AppError> {
        self.request(|reply| DeviceRequest::GetInfo { reply }).await
    }

    pub async fn read_reading(&self) -> Result<Reading, AppError> {
        self.request(|reply| DeviceRequest::ReadReading { reply })
            .await
    }

    pub async fn read_pid(&self) -> Result<PidValues, AppError> {
        self.request(|reply| DeviceRequest::ReadPid { reply }).await
    }

    pub async fn write_setpoint(&self, value: f64) -> Result<(), AppError> {
        self.request(|reply| DeviceRequest::WriteSetpoint { value, reply })
            .await
    }

    pub async fn write_pid(&self, values: PidValues) -> Result<(), AppError> {
        self.request(|reply| DeviceRequest::WritePid { values, reply })
            .await
    }

    pub async fn set_run_status(&self, status: RunStatus) -> Result<(), AppError> {
        self.request(|reply| DeviceRequest::SetRunStatus { status, reply })
            .await
    }

    pub async fn upload_curve(&self) -> Result<Vec<Segment>, AppError> {
        self.request(|reply| DeviceRequest::UploadCurve { reply })
            .await
    }

    pub async fn download_curve(&self, segments: Vec<Segment>) -> Result<(), AppError> {
        self.request(|reply| DeviceRequest::DownloadCurve { segments, reply })
            .await
    }
}

enum DeviceRequest {
    Connect {
        cfg: ConnectionConfig,
        reply: oneshot::Sender<Result<DeviceInfo, AppError>>,
    },
    Disconnect {
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    GetInfo {
        reply: oneshot::Sender<Result<DeviceInfo, AppError>>,
    },
    ReadReading {
        reply: oneshot::Sender<Result<Reading, AppError>>,
    },
    ReadPid {
        reply: oneshot::Sender<Result<PidValues, AppError>>,
    },
    WriteSetpoint {
        value: f64,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    WritePid {
        values: PidValues,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    SetRunStatus {
        status: RunStatus,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
    UploadCurve {
        reply: oneshot::Sender<Result<Vec<Segment>, AppError>>,
    },
    DownloadCurve {
        segments: Vec<Segment>,
        reply: oneshot::Sender<Result<(), AppError>>,
    },
}

struct DeviceActor {
    mode: BackendMode,
    backend: Option<Box<dyn DeviceBackend>>,
    info: DeviceInfo,
    rx: mpsc::Receiver<DeviceRequest>,
}

impl DeviceActor {
    fn new(mode: BackendMode, rx: mpsc::Receiver<DeviceRequest>) -> Self {
        Self {
            mode,
            backend: None,
            info: DeviceInfo::default(),
            rx,
        }
    }

    async fn run(mut self) {
        while let Some(request) = self.rx.recv().await {
            match request {
                DeviceRequest::Connect { cfg, reply } => {
                    let _ = reply.send(self.connect(cfg).await);
                }
                DeviceRequest::Disconnect { reply } => {
                    let _ = reply.send(self.disconnect().await);
                }
                DeviceRequest::GetInfo { reply } => {
                    let _ = reply.send(Ok(self.info.clone()));
                }
                DeviceRequest::ReadReading { reply } => {
                    let _ = reply.send(self.read_reading().await);
                }
                DeviceRequest::ReadPid { reply } => {
                    let _ = reply.send(self.read_pid().await);
                }
                DeviceRequest::WriteSetpoint { value, reply } => {
                    let _ = reply.send(self.write_setpoint(value).await);
                }
                DeviceRequest::WritePid { values, reply } => {
                    let _ = reply.send(self.write_pid(values).await);
                }
                DeviceRequest::SetRunStatus { status, reply } => {
                    let _ = reply.send(self.set_run_status(status).await);
                }
                DeviceRequest::UploadCurve { reply } => {
                    let _ = reply.send(self.upload_curve().await);
                }
                DeviceRequest::DownloadCurve { segments, reply } => {
                    let _ = reply.send(self.download_curve(segments).await);
                }
            }
        }
    }

    fn scale(&self) -> convert::ScaleConfig {
        convert::ScaleConfig {
            decimal_point: self.info.decimal_point,
            scale_factor: self.info.scale_factor,
        }
    }

    fn backend(&mut self) -> Result<&mut Box<dyn DeviceBackend>, AppError> {
        self.backend.as_mut().ok_or(AppError::NotConnected)
    }

    async fn connect(&mut self, cfg: ConnectionConfig) -> Result<DeviceInfo, AppError> {
        validate::validate_slave_addr(cfg.slave_addr)?;
        if self.backend.is_some() {
            self.disconnect().await?;
        }
        let mut backend = create_backend(self.mode);
        backend.connect(&cfg).await?;

        let model_raw = backend
            .read_registers(registers::MODEL, 1)
            .await?
            .first()
            .copied();
        let dpt_raw = backend
            .read_registers(registers::DPT, 1)
            .await
            .ok()
            .and_then(|values| values.first().copied())
            .and_then(convert::parameter_from_raw)
            .map(|value| value as u16);
        let scale = convert::parse_dpt(dpt_raw);
        let model_code =
            model_raw.and_then(|raw| convert::parameter_from_raw(raw).map(|v| v as u16));
        let model_name = model_code.map(registers::model_name);

        self.info = DeviceInfo {
            connected: true,
            model_code,
            model_name,
            decimal_point: scale.decimal_point,
            scale_factor: scale.scale_factor,
        };
        self.backend = Some(backend);
        info!("device connected: {:?}", self.info.model_name);
        Ok(self.info.clone())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        if let Some(mut backend) = self.backend.take() {
            if let Err(err) = backend.disconnect().await {
                error!("disconnect failed: {err}");
            }
        }
        self.info = DeviceInfo::default();
        Ok(())
    }

    async fn read_reading(&mut self) -> Result<Reading, AppError> {
        let scale = self.scale();
        let backend = self.backend()?;
        let values = backend.read_registers(registers::PV, 3).await?;
        let pv = values
            .first()
            .and_then(|raw| convert::read_scaled(*raw, scale));
        let sv = values
            .get(1)
            .and_then(|raw| convert::read_scaled(*raw, scale));
        let mv = values.get(2).map(|raw| convert::mv_percent(*raw));
        Ok(Reading {
            pv,
            sv,
            mv,
            ts: unix_ms(),
        })
    }

    async fn read_pid(&mut self) -> Result<PidValues, AppError> {
        let scale = self.scale();
        let backend = self.backend()?;
        let values = backend.read_registers(registers::P, 3).await?;
        let p = values
            .first()
            .and_then(|raw| convert::read_scaled(*raw, scale))
            .ok_or_else(|| AppError::InvalidData("PID P has no valid data".to_string()))?;
        let i = values
            .get(1)
            .and_then(|raw| convert::parameter_from_raw(*raw))
            .ok_or_else(|| AppError::InvalidData("PID I has no valid data".to_string()))?;
        let d = values
            .get(2)
            .and_then(|raw| convert::d_seconds_from_raw(*raw))
            .ok_or_else(|| AppError::InvalidData("PID D has no valid data".to_string()))?;
        Ok(PidValues { p, i: i as u32, d })
    }

    async fn write_setpoint(&mut self, value: f64) -> Result<(), AppError> {
        validate::validate_temperature(value)?;
        let raw = convert::write_scaled(value, self.scale())?;
        self.backend()?.write_register(registers::SP1, raw).await
    }

    async fn write_pid(&mut self, values: PidValues) -> Result<(), AppError> {
        validate::validate_pid(values.p, values.i, values.d)?;
        let scale = self.scale();
        let previous = self.read_pid().await?;
        let encoded = encode_pid(&values, scale)?;
        let encoded_previous = encode_pid(&previous, scale)?;
        let backend = self.backend()?;

        match write_pid_transaction(backend, encoded).await {
            Ok(()) => Ok(()),
            Err(write_error) => {
                warn!("PID write failed, restoring previous values: {write_error}");
                match write_pid_transaction(backend, encoded_previous).await {
                    Ok(()) => Err(AppError::Backend(format!(
                        "PID write failed: {write_error}; rollback succeeded"
                    ))),
                    Err(rollback_error) => Err(AppError::Backend(format!(
                        "PID write failed: {write_error}; rollback failed: {rollback_error}"
                    ))),
                }
            }
        }
    }

    async fn set_run_status(&mut self, status: RunStatus) -> Result<(), AppError> {
        self.backend()?
            .write_register(registers::SRUN, status.register_value())
            .await
    }

    async fn upload_curve(&mut self) -> Result<Vec<Segment>, AppError> {
        let scale = self.scale();
        let backend = self.backend()?;
        let pno = backend
            .read_registers(registers::PNO, 1)
            .await?
            .first()
            .and_then(|raw| convert::parameter_from_raw(*raw))
            .unwrap_or(0);
        if pno <= 0 {
            return Ok(Vec::new());
        }

        let count = (pno as usize).min(validate::limits().segment_max_count);
        let mut segments = Vec::with_capacity(count);
        for index in 0..count {
            let result = backend
                .read_registers(registers::SP_START + index as u16 * 2, 2)
                .await;
            match result {
                Ok(values) => {
                    if let (Some(temperature), Some(minutes)) = (
                        values
                            .first()
                            .and_then(|raw| convert::read_scaled(*raw, scale)),
                        values
                            .get(1)
                            .and_then(|raw| convert::parameter_from_raw(*raw)),
                    ) {
                        segments.push(Segment {
                            temperature,
                            minutes: minutes.max(0) as i32,
                        });
                    } else {
                        warn!("curve segment {index} contains invalid data");
                        sleep(Duration::from_millis(50)).await;
                        return Err(AppError::InvalidData(format!(
                            "curve segment {index} contains invalid data"
                        )));
                    }
                }
                Err(err) => {
                    warn!("curve segment {index} read failed: {err}");
                    sleep(Duration::from_millis(50)).await;
                    return Err(err);
                }
            }
            sleep(Duration::from_millis(50)).await;
        }
        Ok(segments)
    }

    async fn download_curve(&mut self, segments: Vec<Segment>) -> Result<(), AppError> {
        validate::validate_segments(&segments)?;
        let scale = self.scale();
        let previous_segments = self.upload_curve().await?;
        let encoded_segments = encode_segments(&segments, scale)?;
        let encoded_previous = encode_segments(&previous_segments, scale)?;
        let backend = self.backend()?;

        match write_curve_transaction(backend, &encoded_segments).await {
            Ok(()) => Ok(()),
            Err(write_error) => {
                warn!("curve download failed, restoring previous curve: {write_error}");
                match write_curve_transaction(backend, &encoded_previous).await {
                    Ok(()) => Err(AppError::Backend(format!(
                        "curve download failed: {write_error}; rollback succeeded"
                    ))),
                    Err(rollback_error) => Err(AppError::Backend(format!(
                        "curve download failed: {write_error}; rollback failed: {rollback_error}"
                    ))),
                }
            }
        }
    }
}

fn encode_pid(
    values: &PidValues,
    scale: convert::ScaleConfig,
) -> Result<(u16, u16, u16), AppError> {
    Ok((
        convert::write_scaled(values.p, scale)?,
        convert::to_uint16(values.i as i32, "PID I")?,
        convert::d_seconds_to_raw(values.d)?,
    ))
}

async fn write_pid_transaction(
    backend: &mut Box<dyn DeviceBackend>,
    encoded: (u16, u16, u16),
) -> Result<(), AppError> {
    backend.write_register(registers::P, encoded.0).await?;
    backend.write_register(registers::I, encoded.1).await?;
    backend.write_register(registers::D, encoded.2).await?;

    let values = backend.read_registers(registers::P, 3).await?;
    if values.first().copied() != Some(encoded.0)
        || values.get(1).copied() != Some(encoded.1)
        || values.get(2).copied() != Some(encoded.2)
    {
        return Err(AppError::InvalidData(
            "PID read-back verification failed".to_string(),
        ));
    }
    Ok(())
}

fn encode_segments(
    segments: &[Segment],
    scale: convert::ScaleConfig,
) -> Result<Vec<(u16, u16)>, AppError> {
    segments
        .iter()
        .map(|segment| {
            Ok((
                convert::write_scaled(segment.temperature, scale)?,
                convert::to_uint16(segment.minutes, "segment minutes")?,
            ))
        })
        .collect()
}

async fn write_curve_transaction(
    backend: &mut Box<dyn DeviceBackend>,
    encoded_segments: &[(u16, u16)],
) -> Result<(), AppError> {
    for (index, (temperature, minutes)) in encoded_segments.iter().enumerate() {
        let base = registers::SP_START + index as u16 * 2;
        backend.write_register(base, *temperature).await?;
        backend.write_register(base + 1, *minutes).await?;
        sleep(Duration::from_millis(50)).await;

        let values = backend.read_registers(base, 2).await?;
        if values.first().copied() != Some(*temperature)
            || values.get(1).copied() != Some(*minutes)
        {
            return Err(AppError::InvalidData(format!(
                "curve segment {index} read-back mismatch"
            )));
        }
    }

    let pno = convert::to_uint16(encoded_segments.len() as i32, "Pno")?;
    backend.write_register(registers::PNO, pno).await?;
    let committed_pno = backend
        .read_registers(registers::PNO, 1)
        .await?
        .first()
        .copied();
    if committed_pno != Some(pno) {
        return Err(AppError::InvalidData(
            "curve PNO read-back mismatch".to_string(),
        ));
    }

    for (index, (temperature, minutes)) in encoded_segments.iter().enumerate() {
        let base = registers::SP_START + index as u16 * 2;
        let values = backend.read_registers(base, 2).await?;
        if values.first().copied() != Some(*temperature)
            || values.get(1).copied() != Some(*minutes)
        {
            return Err(AppError::InvalidData(format!(
                "curve segment {index} final verification mismatch"
            )));
        }
    }

    Ok(())
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_connection() -> ConnectionConfig {
        ConnectionConfig {
            port: "MOCK".to_string(),
            slave_addr: 1,
            baudrate: 9600,
        }
    }

    #[tokio::test]
    async fn mock_connect_and_read_info() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        let info = handle.connect(mock_connection()).await.unwrap();
        assert!(info.connected);
        assert_eq!(info.model_name.as_deref(), Some("AI-516P"));
        assert_eq!(info.decimal_point, 1);
    }

    #[tokio::test]
    async fn pid_write_round_trip_uses_mock_backend() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();
        handle
            .write_pid(PidValues {
                p: 12.5,
                i: 240,
                d: 3.0,
            })
            .await
            .unwrap();

        let actual = handle.read_pid().await.unwrap();
        assert_eq!(actual.p, 12.5);
        assert_eq!(actual.i, 240);
        assert_eq!(actual.d, 3.0);
    }

    #[tokio::test]
    async fn curve_round_trip_uses_mock_backend() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();
        let expected = vec![
            Segment {
                temperature: 120.0,
                minutes: 10,
            },
            Segment {
                temperature: 80.0,
                minutes: 5,
            },
        ];
        handle.download_curve(expected.clone()).await.unwrap();
        assert_eq!(handle.upload_curve().await.unwrap(), expected);
    }

    #[tokio::test]
    async fn curve_transaction_can_replace_a_longer_curve_with_a_shorter_curve() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();
        let original = vec![
            Segment {
                temperature: 100.0,
                minutes: 10,
            },
            Segment {
                temperature: 200.0,
                minutes: 20,
            },
            Segment {
                temperature: 300.0,
                minutes: 30,
            },
        ];
        let replacement = vec![Segment {
            temperature: 150.0,
            minutes: 15,
        }];

        handle.download_curve(original).await.unwrap();
        handle.download_curve(replacement.clone()).await.unwrap();

        assert_eq!(handle.upload_curve().await.unwrap(), replacement);
    }

    #[tokio::test]
    async fn mock_reading_contains_live_values() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();
        let reading = handle.read_reading().await.unwrap();
        assert_eq!(reading.pv, Some(100.5));
        assert_eq!(reading.sv, Some(100.0));
        assert_eq!(reading.mv, Some(50.0));
    }

    #[tokio::test]
    async fn offline_backend_returns_structured_error() {
        let handle = DeviceHandle::spawn(BackendMode::MockOffline);
        let err = handle.connect(mock_connection()).await.unwrap_err();
        assert!(matches!(err, AppError::Timeout));
    }
}
