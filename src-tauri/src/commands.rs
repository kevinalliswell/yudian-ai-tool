use tauri::{AppHandle, Emitter, State};
use tokio::time::{sleep, Duration};
use tracing::error;

use crate::error::AppError;
use crate::modbus::validate::{limits, ValidationLimits};
use crate::ports;
use crate::state::AppState;
use crate::types::{
    ConnectionConfig, DeviceInfo, ErrorEvent, PidValues, PortInfo, Reading, RunStatus, Segment,
    StatusUpdate,
};

#[tauri::command(rename_all = "camelCase")]
pub async fn list_serial_ports() -> Result<Vec<PortInfo>, AppError> {
    ports::list_serial_ports()
}

#[tauri::command(rename_all = "camelCase")]
pub async fn connect(
    state: State<'_, AppState>,
    cfg: ConnectionConfig,
) -> Result<DeviceInfo, AppError> {
    state.device.connect(cfg).await
}

#[tauri::command(rename_all = "camelCase")]
pub async fn disconnect(state: State<'_, AppState>) -> Result<(), AppError> {
    stop_monitoring(state.clone()).await?;
    state.device.disconnect().await
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
pub async fn read_setpoint(state: State<'_, AppState>) -> Result<f64, AppError> {
    state.device.read_setpoint().await
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
        let mut consecutive_failures: u32 = 0;
        loop {
            match device.read_reading().await {
                Ok(reading) => {
                    if consecutive_failures > 0 {
                        let _ = app.emit(
                            "device://error",
                            ErrorEvent {
                                scope: "monitoring".to_string(),
                                message: "monitoring recovered".to_string(),
                            },
                        );
                        consecutive_failures = 0;
                    }
                    emit_reading(&app, &reading);
                    sleep(interval).await;
                }
                // A write transaction owns the bus; the link itself is fine.
                Err(err) if !counts_as_monitor_failure(&err) => sleep(interval).await,
                Err(err) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    error!("monitoring read failed: {err}");
                    if should_emit_monitor_error(consecutive_failures) {
                        let _ = app.emit(
                            "device://error",
                            ErrorEvent {
                                scope: "monitoring".to_string(),
                                message: monitor_error_message(consecutive_failures, &err),
                            },
                        );
                    }
                    if consecutive_failures >= 5 {
                        // The actor publishes the resulting status itself.
                        if let Err(disconnect_error) = device.disconnect().await {
                            error!("monitoring disconnect failed: {disconnect_error}");
                        }
                        break;
                    }
                    sleep(monitor_backoff(interval, consecutive_failures)).await;
                }
            }
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

/// Forwards the actor's connection state to the frontend; the actor is the
/// only source, so a link reset reaches the UI just like a disconnect.
pub fn emit_status(app: &AppHandle, update: &StatusUpdate) {
    let _ = app.emit("device://status", &update.info);
    if let Some(reason) = &update.reason {
        let _ = app.emit(
            "device://error",
            ErrorEvent {
                scope: "connection".to_string(),
                message: reason.clone(),
            },
        );
    }
}

fn emit_reading(app: &AppHandle, reading: &Reading) {
    let _ = app.emit("device://reading", reading);
}

fn monitor_backoff(interval: Duration, consecutive_failures: u32) -> Duration {
    let shift = consecutive_failures.saturating_sub(1).min(3);
    let cap = interval.max(Duration::from_secs(30));
    interval.saturating_mul(1u32 << shift).min(cap)
}

fn counts_as_monitor_failure(error: &AppError) -> bool {
    !matches!(error, AppError::Busy)
}

fn should_emit_monitor_error(consecutive_failures: u32) -> bool {
    consecutive_failures <= 3 || consecutive_failures == 5
}

fn monitor_error_message(consecutive_failures: u32, error: &AppError) -> String {
    match consecutive_failures {
        3 => format!(
            "monitoring connection unstable after {consecutive_failures} consecutive failures: {error}"
        ),
        5 => "monitoring stopped after 5 consecutive failures".to_string(),
        _ => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitor_backoff_doubles_and_caps_at_thirty_seconds() {
        let interval = Duration::from_secs(1);

        assert_eq!(monitor_backoff(interval, 1), Duration::from_secs(1));
        assert_eq!(monitor_backoff(interval, 2), Duration::from_secs(2));
        assert_eq!(monitor_backoff(interval, 3), Duration::from_secs(4));
        assert_eq!(monitor_backoff(interval, 4), Duration::from_secs(8));
        assert_eq!(monitor_backoff(interval, 5), Duration::from_secs(8));
        assert_eq!(
            monitor_backoff(Duration::from_secs(10), 3),
            Duration::from_secs(30)
        );
        assert_eq!(
            monitor_backoff(Duration::from_secs(60), 2),
            Duration::from_secs(60)
        );
    }

    #[test]
    fn busy_bus_does_not_count_towards_monitor_disconnect() {
        assert!(!counts_as_monitor_failure(&AppError::Busy));
        assert!(counts_as_monitor_failure(&AppError::Timeout));
        assert!(counts_as_monitor_failure(&AppError::NotConnected));
    }

    #[test]
    fn monitor_error_events_are_rate_limited_to_state_changes() {
        assert!(should_emit_monitor_error(1));
        assert!(should_emit_monitor_error(2));
        assert!(should_emit_monitor_error(3));
        assert!(!should_emit_monitor_error(4));
        assert!(should_emit_monitor_error(5));
        assert!(!should_emit_monitor_error(6));
    }
}
