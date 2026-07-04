use tauri::async_runtime::JoinHandle;
use tokio::sync::Mutex;

use crate::backend::BackendMode;
use crate::device::actor::DeviceHandle;

pub struct AppState {
    pub device: DeviceHandle,
    pub monitor: Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            device: DeviceHandle::spawn(BackendMode::from_env()),
            monitor: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
