use tauri::{AppHandle, Emitter, State};
use tokio::time::{sleep, Duration};
use tracing::error;

use crate::error::AppError;
use crate::modbus::validate::{limits, ValidationLimits};
use crate::ports;
use crate::state::AppState;
use crate::types::{
    ConnectionConfig, DeviceInfo, ErrorEvent, PidValues, PortInfo, Reading, RunStatus, Segment,
    StatusEvent,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_serial_ports() -> Result<Vec<PortInfo>, AppError> {
    ports::list_serial_ports()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    cfg: ConnectionConfig,
) -> Result<DeviceInfo, AppError> {
    let info = state.device.connect(cfg).await?;
    emit_status(&app, &info);
    Ok(info)
}

#[tauri::command(rename_all = "camelCase")]
pub async fn disconnect(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    stop_monitoring(state.clone()).await?;
    state.device.disconnect().await?;
    emit_status(
        &app,
        &DeviceInfo {
            connected: false,
            ..DeviceInfo::default()
        },
    );
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn get_device_info(state: State<'_, AppState>) -> Result<DeviceInfo, AppError> {
    state.device.get_info().await
}

#[tauri::command(rename_all = "camelCase")]
pub fn get_validation_limits() -> ValidationLimits {
    limits()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn read_pid(state: State<'_, AppState>) -> Result<PidValues, AppError> {
    state.device.read_pid().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn write_setpoint(state: State<'_, AppState>, value: f64) -> Result<(), AppError> {
    state.device.write_setpoint(value).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn write_pid(state: State<'_, AppState>, values: PidValues) -> Result<(), AppError> {
    state.device.write_pid(values).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn set_run_status(state: State<'_, AppState>, status: RunStatus) -> Result<(), AppError> {
    state.device.set_run_status(status).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn upload_curve(state: State<'_, AppState>) -> Result<Vec<Segment>, AppError> {
    state.device.upload_curve().await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn download_curve(
    state: State<'_, AppState>,
    segments: Vec<Segment>,
) -> Result<(), AppError> {
    state.device.download_curve(segments).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn start_monitoring(
    app: AppHandle,
    state: State<'_, AppState>,
    interval_ms: u32,
) -> Result<(), AppError> {
    stop_monitoring(state.clone()).await?;
    let min_interval = limits().refresh_interval_min_ms;
    let interval = Duration::from_millis(interval_ms.max(min_interval) as u64);
    let device = state.device.clone();
    let handle = tauri::async_runtime::spawn(async move {
        loop {
            match device.read_reading().await {
                Ok(reading) => emit_reading(&app, &reading),
                Err(err) => {
                    error!("monitoring read failed: {err}");
                    let _ = app.emit(
                        "device://error",
                        ErrorEvent {
                            scope: "monitoring".to_string(),
                            message: err.to_string(),
                        },
                    );
                }
            }
            sleep(interval).await;
        }
    });
    *state.monitor.lock().await = Some(handle);
    Ok(())
}

#[tauri::command(rename_all = "camelCase")]
pub async fn stop_monitoring(state: State<'_, AppState>) -> Result<(), AppError> {
    if let Some(handle) = state.monitor.lock().await.take() {
        handle.abort();
    }
    Ok(())
}

fn emit_status(app: &AppHandle, info: &DeviceInfo) {
    let _ = app.emit(
        "device://status",
        StatusEvent {
            connected: info.connected,
            model: info.model_name.clone(),
        },
    );
}

fn emit_reading(app: &AppHandle, reading: &Reading) {
    let _ = app.emit("device://reading", reading);
}
