pub mod backend;
pub mod commands;
pub mod device;
pub mod error;
pub mod modbus;
pub mod ports;
pub mod state;
pub mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(state::AppState::new())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .invoke_handler(tauri::generate_handler![
            commands::list_serial_ports,
            commands::connect,
            commands::disconnect,
            commands::get_device_info,
            commands::get_validation_limits,
            commands::read_pid,
            commands::write_setpoint,
            commands::write_pid,
            commands::set_run_status,
            commands::upload_curve,
            commands::download_curve,
            commands::start_monitoring,
            commands::stop_monitoring
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Yudian AI Tool");
}
