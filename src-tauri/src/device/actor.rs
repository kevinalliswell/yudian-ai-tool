use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::{sleep, timeout, timeout_at, Duration, Instant};
use tracing::{error, info, warn};

use crate::backend::{create_backend, BackendMode, DeviceBackend};
use crate::error::AppError;
use crate::modbus::{convert, registers, validate};
use crate::types::{
    ConnectionConfig, DeviceInfo, PidValues, Reading, RunStatus, Segment, StatusUpdate,
};

const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(3);
const CONNECT_REQUEST_TIMEOUT: Duration = Duration::from_secs(5);
const CURVE_UPLOAD_BASE_TIMEOUT_SECS: u64 = 5;
const CURVE_UPLOAD_PER_SEGMENT_TIMEOUT_SECS: u64 = 2;
/// How long a write transaction may wait in the queue before the caller
/// gives up; an abandoned transaction is skipped without touching the bus.
const TRANSACTION_QUEUE_TIMEOUT: Duration = Duration::from_secs(5);
/// Upper bound for a single backend call inside a write transaction.
const TRANSACTION_OP_TIMEOUT: Duration = Duration::from_secs(1);
const SEGMENT_THROTTLE: Duration = Duration::from_millis(50);

#[derive(Clone)]
pub struct DeviceHandle {
    tx: mpsc::Sender<QueuedRequest>,
    in_transaction: Arc<AtomicBool>,
    status: watch::Receiver<StatusUpdate>,
}

/// The actor's ends of the channels shared with its `DeviceHandle`.
struct ActorChannels {
    rx: mpsc::Receiver<QueuedRequest>,
    in_transaction: Arc<AtomicBool>,
    status: watch::Sender<StatusUpdate>,
}

impl DeviceHandle {
    pub fn spawn(mode: BackendMode) -> Self {
        let (handle, channels) = Self::channel();
        tauri::async_runtime::spawn(DeviceActor::new(mode, channels).run());
        handle
    }

    fn channel() -> (Self, ActorChannels) {
        let (tx, rx) = mpsc::channel(64);
        let (status_tx, status_rx) = watch::channel(StatusUpdate::default());
        let in_transaction = Arc::new(AtomicBool::new(false));
        let handle = Self {
            tx,
            in_transaction: in_transaction.clone(),
            status: status_rx,
        };
        let channels = ActorChannels {
            rx,
            in_transaction,
            status: status_tx,
        };
        (handle, channels)
    }

    /// Connection state as last published by the actor.
    pub fn subscribe_status(&self) -> watch::Receiver<StatusUpdate> {
        self.status.clone()
    }

    fn is_busy(&self) -> bool {
        self.in_transaction.load(Ordering::Acquire)
    }

    async fn enqueue(&self, queued: QueuedRequest) -> Result<(), AppError> {
        if self.is_busy() {
            return Err(AppError::Busy);
        }
        timeout_at(queued.deadline, self.tx.send(queued))
            .await
            .map_err(|_| AppError::Timeout)?
            .map_err(|_| AppError::Backend("device actor is not running".to_string()))
    }

    /// Queues a request that is safe to cancel: reads and single-register
    /// writes. The actor drops it once `request_timeout` has passed.
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
        self.enqueue(QueuedRequest {
            deadline,
            started: None,
            request: build(reply_tx),
        })
        .await?;
        let reply = timeout_at(deadline, reply_rx).await;
        match reply {
            Ok(Ok(result)) => result,
            // A write transaction started while this request waited in the queue.
            _ if self.is_busy() => Err(AppError::Busy),
            Err(_) => Err(AppError::Timeout),
            Ok(Err(_)) if Instant::now() >= deadline => Err(AppError::Timeout),
            Ok(Err(_)) => Err(AppError::Backend(
                "device actor dropped response".to_string(),
            )),
        }
    }

    /// Queues a multi-register write that must run to completion once it
    /// starts, so that a failure is always followed by its rollback.
    async fn transaction<T>(
        &self,
        ceiling: Duration,
        build: impl FnOnce(oneshot::Sender<Result<T, AppError>>) -> DeviceRequest,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
    {
        let (reply_tx, reply_rx) = oneshot::channel();
        let (started_tx, mut started_rx) = oneshot::channel();
        let queue_deadline = Instant::now() + TRANSACTION_QUEUE_TIMEOUT;
        self.enqueue(QueuedRequest {
            deadline: queue_deadline,
            started: Some(started_tx),
            request: build(reply_tx),
        })
        .await?;

        let started = match timeout_at(queue_deadline, &mut started_rx).await {
            Ok(result) => result.is_ok(),
            // The actor may have started it right at the deadline.
            Err(_) => started_rx.try_recv().is_ok(),
        };
        if !started {
            // Dropping `started_rx` makes the actor skip the request unwritten.
            return Err(AppError::Timeout);
        }

        match timeout(ceiling, reply_rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AppError::Backend(
                "device actor dropped response".to_string(),
            )),
            Err(_) => Err(AppError::OutcomeUnknown(
                "write transaction is still running; re-read the device".to_string(),
            )),
        }
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
        self.transaction(pid_transaction_ceiling(), |reply| DeviceRequest::WritePid {
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
        self.request(curve_upload_timeout(), |reply| DeviceRequest::UploadCurve {
            reply,
        })
        .await
    }

    pub async fn download_curve(&self, segments: Vec<Segment>) -> Result<(), AppError> {
        self.transaction(curve_transaction_ceiling(segments.len()), |reply| {
            DeviceRequest::DownloadCurve { segments, reply }
        })
        .await
    }
}

fn curve_upload_timeout() -> Duration {
    Duration::from_secs(
        CURVE_UPLOAD_BASE_TIMEOUT_SECS
            + validate::limits().segment_max_count as u64 * CURVE_UPLOAD_PER_SEGMENT_TIMEOUT_SECS,
    )
}

/// Worst case for a transaction whose every backend call runs into
/// `TRANSACTION_OP_TIMEOUT`, plus slack for scheduling.
fn transaction_ceiling(ops: usize, throttles: usize) -> Duration {
    TRANSACTION_OP_TIMEOUT * ops as u32
        + SEGMENT_THROTTLE * throttles as u32
        + Duration::from_secs(5)
}

fn pid_transaction_ceiling() -> Duration {
    // Backup read, then 3 writes + read-back forward and again for rollback.
    transaction_ceiling(1 + 4 + 4, 0)
}

fn curve_transaction_ceiling(new_count: usize) -> Duration {
    // The backup can hold up to the maximum curve, which rollback rewrites.
    let backup = validate::limits().segment_max_count;
    // Per segment: 2 writes + 1 read-back and a final read; plus Pno write + read.
    let write_ops = |count: usize| 4 * count + 2;
    transaction_ceiling(
        1 + backup + write_ops(new_count) + write_ops(backup),
        backup + new_count + backup,
    )
}

/// Bounds every backend call inside a write transaction, so a hung call
/// becomes an ordinary error that the transaction answers with its rollback
/// instead of stalling or being cancelled halfway.
struct GuardedBackend<'a> {
    inner: &'a mut dyn DeviceBackend,
    timed_out: bool,
}

impl<'a> GuardedBackend<'a> {
    fn new(inner: &'a mut dyn DeviceBackend) -> Self {
        Self {
            inner,
            timed_out: false,
        }
    }
}

#[async_trait]
impl DeviceBackend for GuardedBackend<'_> {
    async fn connect(&mut self, cfg: &ConnectionConfig) -> Result<(), AppError> {
        self.inner.connect(cfg).await
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        self.inner.disconnect().await
    }

    async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError> {
        match timeout(
            TRANSACTION_OP_TIMEOUT,
            self.inner.read_registers(addr, count),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                self.timed_out = true;
                Err(AppError::Timeout)
            }
        }
    }

    async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError> {
        match timeout(
            TRANSACTION_OP_TIMEOUT,
            self.inner.write_register(addr, value),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                self.timed_out = true;
                Err(AppError::Timeout)
            }
        }
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
    /// Cancellable requests: dropped once passed. Transactions: the queue
    /// deadline after which the caller has stopped waiting for the start.
    deadline: Instant,
    /// Present for write transactions; the actor signals here before the
    /// first bus operation and skips the request if the caller is gone.
    started: Option<oneshot::Sender<()>>,
    request: DeviceRequest,
}

struct DeviceActor {
    mode: BackendMode,
    backend: Option<Box<dyn DeviceBackend>>,
    info: DeviceInfo,
    dpt_valid: bool,
    curve_verified: bool,
    /// Set when a transaction abandoned a hung backend call; a late reply may
    /// still arrive, so the link is reset once the transaction finishes.
    link_suspect: bool,
    in_transaction: Arc<AtomicBool>,
    status: watch::Sender<StatusUpdate>,
    rx: mpsc::Receiver<QueuedRequest>,
}

impl DeviceActor {
    fn new(mode: BackendMode, channels: ActorChannels) -> Self {
        Self {
            mode,
            backend: None,
            info: DeviceInfo::default(),
            dpt_valid: false,
            curve_verified: false,
            link_suspect: false,
            in_transaction: channels.in_transaction,
            status: channels.status,
            rx: channels.rx,
        }
    }

    fn publish_status(&self, reason: Option<String>) {
        self.status.send_replace(StatusUpdate {
            info: self.info.clone(),
            reason,
        });
    }

    async fn run(mut self) {
        while let Some(queued) = self.rx.recv().await {
            if Instant::now() >= queued.deadline {
                continue;
            }
            match queued.started {
                Some(started) => self.run_transaction(started, queued.request).await,
                None => {
                    if timeout_at(queued.deadline, self.handle_request(queued.request))
                        .await
                        .is_err()
                    {
                        self.reset_link("request timed out").await;
                    }
                }
            }
        }
    }

    /// Runs a write transaction to completion with no outer timeout: each
    /// backend call is bounded by `GuardedBackend`, so a failure always
    /// reaches the rollback instead of being cancelled mid-write.
    async fn run_transaction(&mut self, started: oneshot::Sender<()>, request: DeviceRequest) {
        self.in_transaction.store(true, Ordering::Release);
        if started.send(()).is_ok() {
            self.handle_request(request).await;
        }
        self.in_transaction.store(false, Ordering::Release);
        if std::mem::take(&mut self.link_suspect) {
            self.reset_link("backend call timed out during a write transaction")
                .await;
        }
    }

    async fn reset_link(&mut self, reason: &str) {
        if let Some(mut backend) = self.backend.take() {
            if timeout_at(
                Instant::now() + DEFAULT_REQUEST_TIMEOUT,
                backend.disconnect(),
            )
            .await
            .is_err()
            {
                error!("disconnect during link reset also timed out");
            }
        }
        self.info = DeviceInfo::default();
        self.dpt_valid = false;
        self.curve_verified = false;
        self.link_suspect = false;
        warn!("device backend reset: {reason}");
        self.publish_status(Some(reason.to_string()));
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

    fn ensure_writable(&self) -> Result<(), AppError> {
        if !self.info.write_enabled {
            let reason = if !self.dpt_valid {
                "DPT could not be read"
            } else {
                "its model is not supported"
            };
            return Err(AppError::InvalidData(format!(
                "device is read-only because {reason}"
            )));
        }
        Ok(())
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
        let dpt_values = backend.read_registers(registers::DPT, 1).await.ok();
        let dpt_raw = dpt_values
            .as_ref()
            .and_then(|values| values.first().copied())
            .and_then(convert::parameter_from_raw)
            .and_then(|value| u16::try_from(value).ok());
        let dpt_valid = dpt_raw.is_some();
        let scale = convert::parse_dpt(dpt_raw);
        let model_code = model_raw
            .and_then(convert::parameter_from_raw)
            .and_then(|value| u16::try_from(value).ok());
        let model_name = model_code.map(registers::model_name);
        let write_enabled = dpt_valid
            && model_code
                .map(registers::is_supported_model)
                .unwrap_or(false);

        self.info = DeviceInfo {
            connected: true,
            write_enabled,
            model_code,
            model_name,
            decimal_point: scale.decimal_point,
            scale_factor: scale.scale_factor,
        };
        self.dpt_valid = dpt_valid;
        self.curve_verified = false;
        self.backend = Some(backend);
        info!("device connected: {:?}", self.info.model_name);
        self.publish_status(None);
        Ok(self.info.clone())
    }

    async fn disconnect(&mut self) -> Result<(), AppError> {
        if let Some(mut backend) = self.backend.take() {
            if let Err(err) = backend.disconnect().await {
                error!("disconnect failed: {err}");
            }
        }
        self.info = DeviceInfo::default();
        self.dpt_valid = false;
        self.curve_verified = false;
        self.publish_status(None);
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
        let i =
            u32::try_from(i).map_err(|_| AppError::InvalidData("PID I is negative".to_string()))?;
        let d = values
            .get(2)
            .and_then(|raw| convert::d_seconds_from_raw(*raw))
            .ok_or_else(|| AppError::InvalidData("PID D has no valid data".to_string()))?;
        Ok(PidValues { p, i, d })
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
        self.ensure_writable()?;
        validate::validate_temperature(value)?;
        let raw = convert::write_scaled(value, self.scale())?;
        self.backend()?.write_register(registers::SP1, raw).await
    }

    async fn write_pid(&mut self, values: PidValues) -> Result<(), AppError> {
        self.ensure_writable()?;
        validate::validate_pid(values.p, values.i, values.d)?;
        let encoded = encode_pid(&values, self.scale())?;
        let backend = self.backend()?;
        let mut guarded = GuardedBackend::new(backend.as_mut());
        let result = pid_transaction(&mut guarded, encoded).await;
        self.link_suspect |= guarded.timed_out;
        result
    }

    async fn set_run_status(&mut self, status: RunStatus) -> Result<(), AppError> {
        self.ensure_writable()?;
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
        if !registers::is_supported_model(model_code) {
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
        read_curve(self.backend()?.as_mut(), scale).await
    }

    async fn download_curve(&mut self, segments: Vec<Segment>) -> Result<(), AppError> {
        self.ensure_writable()?;
        self.curve_verified = false;
        validate::validate_segments(&segments)?;
        let scale = self.scale();
        let encoded_segments = encode_segments(&segments, scale)?;
        let backend = self.backend()?;
        let mut guarded = GuardedBackend::new(backend.as_mut());
        let result = curve_transaction(&mut guarded, scale, &encoded_segments).await;
        self.link_suspect |= guarded.timed_out;
        self.curve_verified = result.is_ok();
        result
    }
}

async fn read_curve(
    backend: &mut dyn DeviceBackend,
    scale: convert::ScaleConfig,
) -> Result<Vec<Segment>, AppError> {
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
                        sleep(SEGMENT_THROTTLE).await;
                        return Err(AppError::InvalidData(format!(
                            "curve segment {index} minutes is negative"
                        )));
                    }
                    if validate::validate_temperature(temperature).is_err() {
                        warn!("curve segment {index} temperature is out of range");
                        sleep(SEGMENT_THROTTLE).await;
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
                    sleep(SEGMENT_THROTTLE).await;
                    return Err(AppError::InvalidData(format!(
                        "curve segment {index} contains invalid data"
                    )));
                }
            }
            Err(err) => {
                warn!("curve segment {index} read failed: {err}");
                sleep(SEGMENT_THROTTLE).await;
                return Err(err);
            }
        }
        sleep(SEGMENT_THROTTLE).await;
    }
    Ok(segments)
}

async fn pid_transaction(
    backend: &mut dyn DeviceBackend,
    encoded: [u16; 3],
) -> Result<(), AppError> {
    let previous: [u16; 3] = backend
        .read_registers(registers::P, 3)
        .await?
        .try_into()
        .map_err(|_| AppError::InvalidData("PID backup has insufficient data".to_string()))?;

    match write_pid_transaction(backend, encoded).await {
        Ok(()) => Ok(()),
        Err(write_error) => {
            warn!("PID write failed, restoring previous values: {write_error}");
            match write_pid_transaction(backend, previous).await {
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

async fn curve_transaction(
    backend: &mut dyn DeviceBackend,
    scale: convert::ScaleConfig,
    encoded_segments: &[(u16, u16)],
) -> Result<(), AppError> {
    let previous_segments = read_curve(backend, scale).await?;
    let encoded_previous = encode_segments(&previous_segments, scale)?;

    match write_curve_transaction(backend, encoded_segments).await {
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
        convert::encode_i16(i64::from(values.i), "PID I")?,
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
                convert::encode_i16(i64::from(segment.minutes), "segment minutes")?,
            ))
        })
        .collect()
}

async fn write_curve_transaction(
    backend: &mut dyn DeviceBackend,
    encoded_segments: &[(u16, u16)],
) -> Result<(), AppError> {
    for (index, (temperature, minutes)) in encoded_segments.iter().enumerate() {
        let base = registers::SP_START + index as u16 * 2;
        backend.write_register(base, *temperature).await?;
        backend.write_register(base + 1, *minutes).await?;
        sleep(SEGMENT_THROTTLE).await;

        let values = backend.read_registers(base, 2).await?;
        if values.first().copied() != Some(*temperature) || values.get(1).copied() != Some(*minutes)
        {
            return Err(AppError::InvalidData(format!(
                "curve segment {index} read-back mismatch"
            )));
        }
    }

    let pno = convert::encode_i16(encoded_segments.len() as i64, "Pno")?;
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
    use std::sync::{Arc, Mutex};

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
                write_enabled: true,
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
        let (handle, channels) = DeviceHandle::channel();
        let mut actor = DeviceActor::new(BackendMode::Mock, channels);
        actor.backend = Some(backend);
        actor.info = info;
        actor.dpt_valid = true;
        // tokio::spawn (not the Tauri runtime) so paused-time tests drive the actor too.
        tokio::spawn(actor.run());
        handle
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Op {
        Read,
        Write,
    }

    struct Delay {
        op: Option<Op>,
        addr: Option<u16>,
        /// 1-based occurrence of (op, addr) to delay; 0 delays every match.
        nth: usize,
        duration: Duration,
    }

    /// Register map shared with the test after the backend moves into the
    /// actor, with injectable per-operation delays and an operation log.
    #[derive(Clone, Default)]
    struct Script {
        registers: Arc<Mutex<HashMap<u16, u16>>>,
        log: Arc<Mutex<Vec<(Op, u16)>>>,
        delays: Arc<Mutex<Vec<Delay>>>,
    }

    impl Script {
        fn with_curve(segments: &[(u16, u16)]) -> Self {
            let script = Self::default();
            {
                let mut registers = script.registers.lock().unwrap();
                registers.insert(registers::PNO, segments.len() as u16);
                for (index, (temperature, minutes)) in segments.iter().enumerate() {
                    let base = registers::SP_START + index as u16 * 2;
                    registers.insert(base, *temperature);
                    registers.insert(base + 1, *minutes);
                }
            }
            script
        }

        fn delay(self, op: Option<Op>, addr: Option<u16>, nth: usize, duration: Duration) -> Self {
            self.delays.lock().unwrap().push(Delay {
                op,
                addr,
                nth,
                duration,
            });
            self
        }

        fn backend(&self) -> Box<dyn DeviceBackend> {
            Box::new(ScriptedBackend(self.clone()))
        }

        fn curve(&self) -> Vec<(u16, u16)> {
            let registers = self.registers.lock().unwrap();
            let pno = registers.get(&registers::PNO).copied().unwrap_or(0);
            (0..pno)
                .map(|index| {
                    let base = registers::SP_START + index * 2;
                    (registers[&base], registers[&(base + 1)])
                })
                .collect()
        }

        fn writes(&self) -> usize {
            self.log
                .lock()
                .unwrap()
                .iter()
                .filter(|(op, _)| *op == Op::Write)
                .count()
        }

        async fn step(&self, op: Op, addr: u16) {
            let delay = {
                let mut log = self.log.lock().unwrap();
                log.push((op, addr));
                let occurrence = log.iter().filter(|entry| **entry == (op, addr)).count();
                self.delays
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|delay| {
                        delay.op.is_none_or(|expected| expected == op)
                            && delay.addr.is_none_or(|expected| expected == addr)
                            && (delay.nth == 0 || delay.nth == occurrence)
                    })
                    .map(|delay| delay.duration)
            };
            if let Some(duration) = delay {
                tokio::time::sleep(duration).await;
            }
        }
    }

    struct ScriptedBackend(Script);

    #[async_trait]
    impl DeviceBackend for ScriptedBackend {
        async fn connect(&mut self, _cfg: &ConnectionConfig) -> Result<(), AppError> {
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<(), AppError> {
            Ok(())
        }

        async fn read_registers(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, AppError> {
            self.0.step(Op::Read, addr).await;
            let registers = self.0.registers.lock().unwrap();
            Ok((0..count)
                .map(|offset| registers.get(&(addr + offset)).copied().unwrap_or(0))
                .collect())
        }

        async fn write_register(&mut self, addr: u16, value: u16) -> Result<(), AppError> {
            self.0.step(Op::Write, addr).await;
            self.0.registers.lock().unwrap().insert(addr, value);
            Ok(())
        }
    }

    fn segments(values: &[(f64, i32)]) -> Vec<Segment> {
        values
            .iter()
            .map(|(temperature, minutes)| Segment {
                temperature: *temperature,
                minutes: *minutes,
            })
            .collect()
    }

    #[tokio::test]
    async fn connect_and_disconnect_publish_status() {
        let handle = DeviceHandle::spawn(BackendMode::Mock);
        let mut status = handle.subscribe_status();

        handle.connect(mock_connection()).await.unwrap();
        status.changed().await.unwrap();
        {
            let update = status.borrow_and_update();
            assert!(update.info.connected);
            assert_eq!(update.info.model_name.as_deref(), Some("AI-516P"));
            assert!(update.reason.is_none());
        }

        handle.disconnect().await.unwrap();
        status.changed().await.unwrap();
        assert!(!status.borrow().info.connected);
    }

    #[tokio::test(start_paused = true)]
    async fn link_reset_after_timeout_publishes_disconnected_status_with_reason() {
        let handle = spawn_test_backend(Box::new(SlowBackend));
        let mut status = handle.subscribe_status();

        assert!(matches!(
            handle.read_reading().await,
            Err(AppError::Timeout)
        ));
        status.changed().await.unwrap();

        let update = status.borrow();
        assert!(!update.info.connected);
        assert!(update
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("timed out")));
    }

    #[tokio::test(start_paused = true)]
    async fn hung_write_mid_download_rolls_back_to_the_original_curve() {
        let original = [(1000, 20), (1500, 30)];
        let script = Script::with_curve(&original).delay(
            Some(Op::Write),
            Some(registers::SP_START + 3),
            1,
            Duration::from_secs(60),
        );
        let handle = spawn_test_backend(script.backend());

        let result = handle
            .download_curve(segments(&[(120.0, 10), (80.0, 5)]))
            .await;

        assert!(matches!(
            result,
            Err(AppError::Backend(message)) if message.contains("rollback succeeded")
        ));
        assert_eq!(script.curve(), original);
        // The hung operation may leave a late reply on the bus, so the link is reset.
        assert!(!handle.get_info().await.unwrap().connected);
    }

    #[tokio::test(start_paused = true)]
    async fn slow_download_beyond_the_old_fixed_budget_still_commits() {
        // Old budget for one segment was 5 + 2 * 1 = 7 s; backing up five
        // segments and writing one at 0.9 s per operation takes about 11 s.
        let script =
            Script::with_curve(&[(1000, 20); 5]).delay(None, None, 0, Duration::from_millis(900));
        let handle = spawn_test_backend(script.backend());

        handle
            .download_curve(segments(&[(120.0, 10)]))
            .await
            .unwrap();

        assert_eq!(script.curve(), [(1200, 10)]);
        assert!(handle.get_info().await.unwrap().connected);
    }

    #[tokio::test(start_paused = true)]
    async fn transaction_abandoned_while_queued_performs_no_writes() {
        let script = Script::with_curve(&[(1000, 20)]).delay(
            Some(Op::Read),
            Some(registers::PV),
            1,
            Duration::from_secs(2),
        );
        let handle = spawn_test_backend(script.backend());

        let reading = tokio::spawn({
            let handle = handle.clone();
            async move { handle.read_reading().await }
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        let write = tokio::spawn({
            let handle = handle.clone();
            async move {
                handle
                    .write_pid(PidValues {
                        p: 12.5,
                        i: 240,
                        d: 3.0,
                    })
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
        write.abort();

        reading.await.unwrap().unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(script.writes(), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn requests_during_a_write_transaction_are_rejected_as_busy() {
        let script =
            Script::with_curve(&[(1000, 20)]).delay(None, None, 0, Duration::from_millis(500));
        let handle = spawn_test_backend(script.backend());

        let download = tokio::spawn({
            let handle = handle.clone();
            async move { handle.download_curve(segments(&[(120.0, 10)])).await }
        });
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(matches!(handle.read_reading().await, Err(AppError::Busy)));
        assert!(matches!(
            handle.write_setpoint(100.0).await,
            Err(AppError::Busy)
        ));
        download.await.unwrap().unwrap();
        assert!(handle.read_reading().await.is_ok());
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
    async fn pid_read_rejects_negative_integral_time() {
        let mut backend = FailOnceBackend::new(0);
        backend.registers.insert(registers::I, 0xFFFF);
        let handle = spawn_test_backend(Box::new(backend));

        let result = handle.read_pid().await;

        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("PID I")
        ));
    }

    #[tokio::test]
    async fn setpoint_write_rejects_values_that_overflow_the_register() {
        let handle = spawn_test_backend_with_info(
            Box::new(CurveDataBackend::with_pno(0)),
            DeviceInfo {
                connected: true,
                write_enabled: true,
                model_code: Some(registers::MODEL_AI_516P),
                model_name: Some("AI-516P".to_string()),
                decimal_point: 2,
                scale_factor: 1,
            },
        );

        let result = handle.write_setpoint(400.0).await;

        assert!(matches!(result, Err(AppError::OutOfRange { .. })));
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
                write_enabled: true,
                model_code: Some(9999),
                model_name: Some(registers::model_name(9999)),
                ..DeviceInfo::default()
            },
        );
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
    async fn dpt_read_failure_keeps_device_read_only() {
        let handle = DeviceHandle::spawn(BackendMode::MockDptFailure);
        let info = handle.connect(mock_connection()).await.unwrap();

        assert!(info.connected);
        assert!(!info.write_enabled);
        assert!(handle.read_reading().await.is_ok());

        let result = handle.write_setpoint(120.0).await;
        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("read-only")
        ));
    }

    #[tokio::test]
    async fn unknown_model_keeps_device_read_only() {
        let handle = DeviceHandle::spawn(BackendMode::MockUnknownModel);
        let info = handle.connect(mock_connection()).await.unwrap();

        assert!(info.connected);
        assert_eq!(info.model_name.as_deref(), Some("未知型号(0x270F)"));
        assert!(!info.write_enabled);
        assert!(handle.read_reading().await.is_ok());

        let result = handle.write_setpoint(120.0).await;
        assert!(matches!(
            result,
            Err(AppError::InvalidData(message)) if message.contains("model")
        ));
    }

    #[tokio::test]
    async fn request_times_out_when_backend_does_not_respond() {
        let handle = spawn_test_backend(Box::new(SlowBackend));

        let result = handle.read_reading().await;

        assert!(matches!(result, Err(AppError::Timeout)));

        let started = Instant::now();
        let info = handle.get_info().await.unwrap();
        assert!(!info.connected);
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
