use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use tauri_plugin_log::log::LevelFilter;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

const MAX_LOG_FILE_BYTES: u128 = 5 * 1024 * 1024;

/// Writes backend logs (including `tracing` events, via its `log` bridge) to
/// stdout and a rotating file in the app log directory.
pub fn plugin<R: Runtime>() -> TauriPlugin<R> {
    let level = if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };
    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .level(level)
        // Transport crates log every frame at debug level.
        .level_for("tokio_modbus", LevelFilter::Warn)
        .level_for("tokio_serial", LevelFilter::Warn)
        .level_for("mio_serial", LevelFilter::Warn)
        .max_file_size(MAX_LOG_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(5))
        // Technicians correlate log lines with the plant clock.
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use tauri_plugin_log::log::{self, Log, Metadata, Record};

    static CAPTURED: Mutex<Vec<String>> = Mutex::new(Vec::new());

    struct CaptureLogger;

    impl Log for CaptureLogger {
        fn enabled(&self, _metadata: &Metadata) -> bool {
            true
        }

        fn log(&self, record: &Record) {
            CAPTURED
                .lock()
                .unwrap()
                .push(format!("{} {}", record.level(), record.args()));
        }

        fn flush(&self) {}
    }

    /// The backend logs through `tracing`; without its `log` bridge every
    /// event is dropped because no tracing subscriber is installed and
    /// tauri-plugin-log only receives `log` records.
    #[test]
    fn tracing_events_reach_the_log_facade() {
        log::set_logger(&CaptureLogger).expect("no other logger in this test binary");
        log::set_max_level(log::LevelFilter::Trace);

        tracing::warn!("device backend reset: probe");

        assert!(CAPTURED
            .lock()
            .unwrap()
            .iter()
            .any(|line| line == "WARN device backend reset: probe"));
    }
}
