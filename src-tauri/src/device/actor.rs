use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout_at, Duration, Instant};
use tracing::{error, info, warn};

use crate::backend::{create_backend, BackendMode, DeviceBackend};
use crate::error::AppError;
use crate::modbus::{convert, registers, validate};
use crate::types::{ConnectionConfig, DeviceInfo, PidValues, Reading, RunStatus, Segment};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CURVE_REQUEST_BASE_TIMEOUT_SECS: u64 = 5;
const CURVE_REQUEST_PER_SEGMENT_TIMEOUT_SECS: u64 = 2;

#[derive(Clone)]
pub struct DeviceHandle {
    tx: mpsc::Sender<QueuedRequest>,
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
        request_timeout: Duration,
        build: impl FnOnce(oneshot::Sender<Result<T, AppError>>) -> DeviceRequest,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let deadline = Instant::now() + request_timeout;
        timeout_at(
            deadline,
            self.tx.send(QueuedRequest {
                deadline,
                request: build(reply_tx),
            }),
        )
        .await
        .map_err(|_| AppError::Timeout)?
        .map_err(|_| AppError::Backend("device actor is not running".to_string()))?;
        timeout_at(deadline, reply_rx)
            .await
            .map_err(|_| AppError::Timeout)?
            .map_err(|_| {
                if Instant::now() >= deadline {
                    AppError::Timeout
                } else {
                    AppError::Backend("device actor dropped response".to_string())
                }
            })?
    }

    pub async fn connect(&self, cfg: ConnectionConfig) -> Result<DeviceInfo, AppError> {
        self.request(CONNECT_REQUEST_TIMEOUT, |reply| DeviceRequest::Connect {
            cfg,
            reply,
        })
        .await
    }

    pub async fn disconnect(&self) -> Result<(), AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| DeviceRequest::Disconnect {
            reply,
        })
        .await
    }

    pub async fn get_info(&self) -> Result<DeviceInfo, AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| DeviceRequest::GetInfo {
            reply,
        })
        .await
    }

    pub async fn read_reading(&self) -> Result<Reading, AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| {
            DeviceRequest::ReadReading { reply }
        })
        .await
    }

    pub async fn read_pid(&self) -> Result<PidValues, AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| DeviceRequest::ReadPid {
            reply,
        })
        .await
    }

    pub async fn read_setpoint(&self) -> Result<f64, AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| {
            DeviceRequest::ReadSetpoint { reply }
        })
        .await
    }

    pub async fn write_setpoint(&self, value: f64) -> Result<(), AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| {
            DeviceRequest::WriteSetpoint { value, reply }
        })
        .await
    }

    pub async fn write_pid(&self, values: PidValues) -> Result<(), AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| DeviceRequest::WritePid {
            values,
            reply,
        })
        .await
    }

    pub async fn set_run_status(&self, status: RunStatus) -> Result<(), AppError> {
        self.request(DEFAULT_REQUEST_TIMEOUT, |reply| {
            DeviceRequest::SetRunStatus { status, reply }
        })
        .await
    }

    pub async fn upload_curve(&self) -> Result<Vec<Segment>, AppError> {
        self.request(
            curve_request_timeout(validate::limits().segment_max_count),
            |reply| DeviceRequest::UploadCurve { reply },
        )
        .await
    }

    pub async fn download_curve(&self, segments: Vec<Segment>) -> Result<(), AppError> {
        self.request(curve_request_timeout(segments.len()), |reply| {
            DeviceRequest::DownloadCurve { segments, reply }
        })
        .await
    }
}

fn curve_request_timeout(segment_count: usize) -> Duration {
    Duration::from_secs(
        CURVE_REQUEST_BASE_TIMEOUT_SECS
            + segment_count.max(1) as u64 * CURVE_REQUEST_PER_SEGMENT_TIMEOUT_SECS,
    )
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
    ReadSetpoint {
        reply: oneshot::Sender<Result<f64, AppError>>,
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

struct QueuedRequest {
    deadline: Instant,
    request: DeviceRequest,
}

struct DeviceActor {
    mode: BackendMode,
    backend: Option<Box<dyn DeviceBackend>>,
    info: DeviceInfo,
    curve_verified: bool,
    rx: mpsc::Receiver<QueuedRequest>,
}

impl DeviceActor {
    fn new(mode: BackendMode, rx: mpsc::Receiver<QueuedRequest>) -> Self {
        Self {
            mode,
            backend: None,
            info: DeviceInfo::default(),
            curve_verified: false,
            rx,
        }
    }

    async fn run(mut self) {
        while let Some(queued) = self.rx.recv().await {
            let _ = timeout_at(queued.deadline, self.handle_request(queued.request)).await;
        }
    }

    async fn handle_request(&mut self, request: DeviceRequest) {
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
            DeviceRequest::ReadSetpoint { reply } => {
                let _ = reply.send(self.read_setpoint().await);
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
        self.curve_verified = false;
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
        self.curve_verified = false;
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

    async fn read_setpoint(&mut self) -> Result<f64, AppError> {
        let scale = self.scale();
        let raw = self
            .backend()?
            .read_registers(registers::SP1, 1)
            .await?
            .first()
            .copied()
            .ok_or_else(|| AppError::InvalidData("SP1 has no valid data".to_string()))?;
        let value = convert::read_scaled(raw, scale)
            .ok_or_else(|| AppError::InvalidData("SP1 has no valid data".to_string()))?;
        validate::validate_temperature(value)
            .map_err(|_| AppError::InvalidData("SP1 is out of range".to_string()))?;
        Ok(value)
    }

    async fn write_setpoint(&mut self, value: f64) -> Result<(), AppError> {
        validate::validate_temperature(value)?;
        let raw = convert::write_scaled(value, self.scale())?;
        self.backend()?.write_register(registers::SP1, raw).await
    }

    async fn write_pid(&mut self, values: PidValues) -> Result<(), AppError> {
        validate::validate_pid(values.p, values.i, values.d)?;
        let scale = self.scale();
        let previous: [u16; 3] = self
            .backend()?
            .read_registers(registers::P, 3)
            .await?
            .try_into()
            .map_err(|_| AppError::InvalidData("PID backup has insufficient data".to_string()))?;
        let encoded = encode_pid(&values, scale)?;
        let backend = self.backend()?;

        match write_pid_transaction(backend.as_mut(), encoded).await {
            Ok(()) => Ok(()),
            Err(write_error) => {
                warn!("PID write failed, restoring previous values: {write_error}");
                match write_pid_transaction(backend.as_mut(), previous).await {
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
        if status == RunStatus::Run {
            self.validate_run_prerequisites().await?;
        }
        self.backend()?
            .write_register(registers::SRUN, status.register_value())
            .await
    }

    async fn validate_run_prerequisites(&mut self) -> Result<(), AppError> {
        if self.backend.is_none() {
            return Err(AppError::NotConnected);
        }
        let model_code = self.info.model_code.ok_or_else(|| {
            AppError::InvalidData("run requires a supported device model".to_string())
        })?;
        if !matches!(
            model_code,
            registers::MODEL_AI_516
                | registers::MODEL_AI_516P
                | registers::MODEL_AI_518
                | registers::MODEL_AI_518P
        ) {
            return Err(AppError::InvalidData(format!(
                "run is not supported for device model {model_code}"
            )));
        }

        if !self.curve_verified {
            return Err(AppError::InvalidData(
                "run requires a verified curve download".to_string(),
            ));
        }

        let reading = self.read_reading().await?;
        validate_run_value("PV", reading.pv)?;
        validate_run_value("SV", reading.sv)?;
        Ok(())
    }

    async fn upload_curve(&mut self) -> Result<Vec<Segment>, AppError> {
        let scale = self.scale();
        let backend = self.backend()?;
        let pno = backend
            .read_registers(registers::PNO, 1)
            .await?
            .first()
            .and_then(|raw| convert::parameter_from_raw(*raw))
            .ok_or_else(|| AppError::InvalidData("PNO has no valid data".to_string()))?;
        if pno == 0 {
            return Ok(Vec::new());
        }
        if pno < 0 {
            return Err(AppError::InvalidData("PNO is negative".to_string()));
        }

        let max_count = validate::limits().segment_max_count;
        if pno as usize > max_count {
            return Err(AppError::out_of_range(
                "curve segment count",
                pno as f64,
                0.0,
                max_count as f64,
            ));
        }
        let count = pno as usize;
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
                        if minutes < 0 {
                            warn!("curve segment {index} has negative minutes");
                            sleep(Duration::from_millis(50)).await;
                            return Err(AppError::InvalidData(format!(
                                "curve segment {index} minutes is negative"
                            )));
                        }
                        if validate::validate_temperature(temperature).is_err() {
                            warn!("curve segment {index} temperature is out of range");
                            sleep(Duration::from_millis(50)).await;
                            return Err(AppError::InvalidData(format!(
                                "curve segment {index} temperature is out of range"
                            )));
                        }
                        segments.push(Segment {
                            temperature,
                            minutes: minutes as i32,
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
        self.curve_verified = false;
        validate::validate_segments(&segments)?;
        let scale = self.scale();
        let previous_segments = self.upload_curve().await?;
        let encoded_segments = encode_segments(&segments, scale)?;
        let encoded_previous = encode_segments(&previous_segments, scale)?;
        let backend = self.backend()?;

        match write_curve_transaction(backend, &encoded_segments).await {
            Ok(()) => {
                self.curve_verified = true;
                Ok(())
            }
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

fn validate_run_value(label: &str, value: Option<f64>) -> Result<(), AppError> {
    let value =
        value.ok_or_else(|| AppError::InvalidData(format!("run requires valid {label} data")))?;
    if !value.is_finite() {
        return Err(AppError::InvalidData(format!(
            "run requires finite {label} data"
        )));
    }
    validate::validate_temperature(value).map_err(|_| {
        AppError::InvalidData(format!("run requires {label} within the temperature range"))
    })
}

fn encode_pid(values: &PidValues, scale: convert::ScaleConfig) -> Result<[u16; 3], AppError> {
    Ok([
        convert::write_scaled(values.p, scale)?,
        convert::to_uint16(values.i as i32, "PID I")?,
        convert::d_seconds_to_raw(values.d)?,
    ])
}

async fn write_pid_transaction(
    backend: &mut dyn DeviceBackend,
    encoded: [u16; 3],
) -> Result<(), AppError> {
    backend.write_register(registers::P, encoded[0]).await?;
    backend.write_register(registers::I, encoded[1]).await?;
    backend.write_register(registers::D, encoded[2]).await?;

    let values = backend.read_registers(registers::P, 3).await?;
    if values.as_slice() != encoded.as_slice() {
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
        if values.first().copied() != Some(*temperature) || values.get(1).copied() != Some(*minutes)
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
        if values.first().copied() != Some(*temperature) || values.get(1).copied() != Some(*minutes)
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
    use std::collections::HashMap;

    use async_trait::async_trait;

    use super::*;

    struct FailOnceBackend {
        registers: HashMap<u16, u16>,
        fail_on_write: usize,
        writes: usize,
    }

    impl FailOnceBackend {
        fn new(fail_on_write: usize) -> Self {
            Self {
                registers: HashMap::from([
                    (registers::P, 120),
                    (registers::I, 300),
                    (registers::D, 45),
                ]),
                fail_on_write,
                writes: 0,
            }
        }
    }

    struct CurveDataBackend {
        registers: HashMap<u16, u16>,
    }

    struct SlowBackend;

    impl CurveDataBackend {
        fn with_pno(pno: u16) -> Self {
            Self {
                registers: HashMap::from([(registers::PNO, pno)]),
            }
        }

        fn with_reading(pv: u16, sv: u16) -> Self {
            Self {
                registers: HashMap::from([
                    (registers::PNO, 1),
                    (registers::SP_START, 1000),
                    (registers::SP_START + 1, 20),
                    (registers::PV, pv),
                    (registers::SV, sv),
                ]),
            }
        }

        fn with_negative_minutes() -> Self {
            Self {
                registers: HashMap::from([
                    (registers::PNO, 1),
                    (registers::SP_START, 1000),
                    (registers::SP_START + 1, 65535),
                ]),
            }
        }
    }

    #[async_trait]
    impl DeviceBackend for SlowBackend {
        async fn connect(&mut self, _cfg: &ConnectionConfig) -> Result<(), AppError> {
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), AppError> {
            Ok(())
        }

        async fn read_registers(&mut self, _addr: u16, _count: u16) -> Result<Vec<u16>, AppError> {
            tokio::time::sleep(Duration::from_secs(4)).await;
            Ok(vec![0; 3])
        }

        async fn write_register(&mut self, _addr: u16, _value: u16) -> Result<(), AppError> {
            tokio::time::sleep(Duration::from_secs(4)).await;
            Ok(())
        }
    }

    #[async_trait]
    impl DeviceBackend for CurveDataBackend {
        async fn connect(&mut self, _cfg: &ConnectionConfig) -> Result<(), AppError> {
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), AppError> {
            Ok(())
        }

        async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError> {
            Ok((0..count)
                .map(|offset| self.registers.get(&(addr + offset)).copied().unwrap_or(0))
                .collect())
        }

        async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError> {
            self.registers.insert(addr, value);
            Ok(())
        }
    }

    #[async_trait]
    impl DeviceBackend for FailOnceBackend {
        async fn connect(&mut self, _cfg: &ConnectionConfig) -> Result<(), AppError> {
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), AppError> {
            Ok(())
        }

        async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError> {
            Ok((0..count)
                .map(|offset| self.registers.get(&(addr + offset)).copied().unwrap_or(0))
                .collect())
        }

        async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError> {
            self.writes += 1;
            if self.writes == self.fail_on_write {
                return Err(AppError::Backend("injected PID write failure".to_string()));
            }
            self.registers.insert(addr, value);
            Ok(())
        }
    }

    fn spawn_test_backend(backend: Box<dyn DeviceBackend>) -> DeviceHandle {
        spawn_test_backend_with_info(
            backend,
            DeviceInfo {
                connected: true,
                model_code: Some(registers::MODEL_AI_516P),
                model_name: Some("AI-516P".to_string()),
                ..DeviceInfo::default()
            },
        )
    }

    fn spawn_test_backend_with_info(
        backend: Box<dyn DeviceBackend>,
        info: DeviceInfo,
    ) -> DeviceHandle {
        let (tx, rx) = mpsc::channel(64);
        let actor = DeviceActor {
            mode: BackendMode::Mock,
            backend: Some(backend),
            info,
            curve_verified: false,
            rx,
        };
        tauri::async_runtime::spawn(actor.run());
        DeviceHandle { tx }
    }

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
    async fn mock_read_setpoint_returns_current_value() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();

        assert_eq!(handle.read_setpoint().await.unwrap(), 100.0);
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
    async fn pid_write_rolls_back_when_middle_register_write_fails() {
        let handle = spawn_test_backend(Box::new(FailOnceBackend::new(2)));

        let result = handle
            .write_pid(PidValues {
                p: 12.5,
                i: 240,
                d: 3.0,
            })
            .await;

        assert!(matches!(
            result,
            Err(AppError::Backend(message)) if message.contains("rollback succeeded")
        ));
        assert_eq!(
            handle.read_pid().await.unwrap(),
            PidValues {
                p: 12.0,
                i: 300,
                d: 4.5,
            }
        );
    }

    #[tokio::test]
    async fn upload_curve_rejects_segment_count_above_limit() {
        let handle = spawn_test_backend(Box::new(CurveDataBackend::with_pno(51)));

        let result = handle.upload_curve().await;

        assert!(matches!(
            result,
            Err(AppError::OutOfRange { label, .. }) if label == "curve segment count"
        ));
    }

    #[tokio::test]
    async fn upload_curve_accepts_empty_curve() {
        let handle = spawn_test_backend(Box::new(CurveDataBackend::with_pno(0)));

        assert!(handle.upload_curve().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn upload_curve_rejects_missing_segment_count() {
        let handle = spawn_test_backend(Box::new(CurveDataBackend::with_pno(32767)));

        let result = handle.upload_curve().await;

        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("PNO")
        ));
    }

    #[tokio::test]
    async fn upload_curve_rejects_negative_segment_minutes() {
        let handle = spawn_test_backend(Box::new(CurveDataBackend::with_negative_minutes()));

        let result = handle.upload_curve().await;

        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("segment 0 minutes")
        ));
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
    async fn run_rejects_unverified_curve() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();

        let result = handle.set_run_status(RunStatus::Run).await;

        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("curve")
        ));
    }

    #[tokio::test]
    async fn run_is_allowed_after_curve_download_verification() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        handle.connect(mock_connection()).await.unwrap();
        handle
            .download_curve(vec![Segment {
                temperature: 12.34,
                minutes: 10,
            }])
            .await
            .unwrap();

        handle.set_run_status(RunStatus::Run).await.unwrap();
    }

    #[tokio::test]
    async fn run_rejects_invalid_pv_and_sv() {
        let invalid_pv = spawn_test_backend(Box::new(CurveDataBackend::with_reading(
            convert::SENTINEL_NO_DATA,
            1000,
        )));
        invalid_pv
            .download_curve(vec![Segment {
                temperature: 120.0,
                minutes: 10,
            }])
            .await
            .unwrap();
        let pv_result = invalid_pv.set_run_status(RunStatus::Run).await;
        assert!(matches!(
            pv_result,
            Err(AppError::InvalidData(message)) if message.contains("PV")
        ));

        let invalid_sv = spawn_test_backend(Box::new(CurveDataBackend::with_reading(
            1000,
            convert::SENTINEL_NO_DATA,
        )));
        invalid_sv
            .download_curve(vec![Segment {
                temperature: 120.0,
                minutes: 10,
            }])
            .await
            .unwrap();
        let sv_result = invalid_sv.set_run_status(RunStatus::Run).await;
        assert!(matches!(
            sv_result,
            Err(AppError::InvalidData(message)) if message.contains("SV")
        ));
    }

    #[tokio::test]
    async fn run_rejects_unknown_model() {
        let handle = spawn_test_backend_with_info(
            Box::new(CurveDataBackend::with_reading(1000, 1000)),
            DeviceInfo {
                connected: true,
                model_code: Some(9999),
                model_name: Some(registers::model_name(9999)),
                ..DeviceInfo::default()
            },
        );
        handle
            .download_curve(vec![Segment {
                temperature: 120.0,
                minutes: 10,
            }])
            .await
            .unwrap();

        let result = handle.set_run_status(RunStatus::Run).await;

        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("model")
        ));
    }

    #[tokio::test]
    async fn offline_backend_returns_structured_error() {
        let handle = DeviceHandle::spawn(BackendMode::MockOffline);
        let err = handle.connect(mock_connection()).await.unwrap_err();
        assert!(matches!(err, AppError::Timeout));
    }

    #[tokio::test]
    async fn request_times_out_when_backend_does_not_respond() {
        let handle = spawn_test_backend(Box::new(SlowBackend));

        let result = handle.read_reading().await;

        assert!(matches!(result, Err(AppError::Timeout)));

        let started = Instant::now();
        assert!(handle.get_info().await.is_ok());
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
