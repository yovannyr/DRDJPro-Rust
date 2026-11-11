/// CrashReporter — captures panics and generates crash reports.
///
/// Uses `backtrace` for stack traces and stores crash metadata for
/// later review or upload.  Minidump generation is deferred to an
/// external handler process in production.

use super::config::CrashHandlerConfig;
use backtrace::Backtrace;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use chrono::Utc;

static INITIALIZED: AtomicBool = AtomicBool::new(false);
static PANIC_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());
static REPORTER_CONFIG: std::sync::Mutex<Option<CrashHandlerConfig>> = std::sync::Mutex::new(None);

/// Errors that can occur during crash reporting.
#[derive(Debug, thiserror::Error)]
pub enum CrashError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database directory could not be created: {0}")]
    DatabaseDir(String),
}

/// Initialize the crash reporter.
///
/// Registers a panic hook via `std::panic::set_hook`.
pub fn init(config: &CrashHandlerConfig) -> Result<(), CrashError> {
    if INITIALIZED.load(Ordering::SeqCst) {
        return Ok(());
    }

    // Ensure the database directory exists
    if !config.database_dir.is_empty() {
        fs::create_dir_all(&config.database_dir)
            .map_err(|e| CrashError::DatabaseDir(format!("{}: {}", config.database_dir, e)))?;
    }

    // Store config statically for panic hook access
    *REPORTER_CONFIG.lock().unwrap() = Some(config.clone());

    // Register panic hook
    if config.enabled {
        std::panic::set_hook(Box::new(|info| {
            let _guard = PANIC_MUTEX.lock();
            if let Some(cfg) = REPORTER_CONFIG.lock().unwrap().as_ref() {
                capture_panic(info, cfg);
            }
        }));
    }

    INITIALIZED.store(true, Ordering::SeqCst);
    Ok(())
}

/// Capture panic information and generate a crash report.
fn capture_panic(info: &std::panic::PanicHookInfo, config: &CrashHandlerConfig) {
    // 1. Capture stack trace
    let bt = Backtrace::new();
    let message = panic_message(info);
    let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();

    // 2. Generate crash report file
    let report_path = if !config.database_dir.is_empty() {
        let report_name = format!("crash_{}.txt", timestamp);
        let full_path = PathBuf::from(&config.database_dir).join(&report_name);

        let dump_content = format!(
            "CRASH REPORT\n\
             ============\n\
             Timestamp: {}\n\
             Message: {}\n\
             Version: {}\n\
             OS: {}\n\
             Arch: {}\n\
             \n\
             STACK TRACE\n\
             -----------\n\
             {:?}\n",
            timestamp, message, config.annotation.version, config.annotation.os, config.annotation.arch, bt
        );

        if let Ok(()) = fs::write(&full_path, dump_content) {
            full_path
        } else {
            PathBuf::new()
        }
    } else {
        PathBuf::new()
    };

    // 3. Write crash info JSON for programmatic access
    if !config.database_dir.is_empty() && !report_path.as_os_str().is_empty() {
        let report_path_str = report_path.to_string_lossy().to_string();
        let crash_info = serde_json::json!({
            "timestamp": timestamp,
            "message": message,
            "stack_trace": format!("{:?}", bt),
            "report_path": report_path_str,
            "version": config.annotation.version,
            "os": config.annotation.os,
            "arch": config.annotation.arch,
        });

        let info_path = report_path.with_extension("json");
        if let Ok(json) = serde_json::to_string_pretty(&crash_info) {
            let _ = fs::write(&info_path, json);
        }
    }

    eprintln!("CrashReporter: {} — {}", timestamp, message);
    eprintln!("CrashReporter: Report saved to {}", report_path.display());
}

/// Extract a human-readable panic message.
fn panic_message(info: &std::panic::PanicHookInfo) -> String {
    if let Some(location) = info.location() {
        format!("Panic at {}:{}:{}", location.file(), location.line(), location.column())
    } else {
        "Unknown panic".to_string()
    }
}

/// Load queued crash reports from disk.
pub fn load_queued(config: &CrashHandlerConfig) -> Result<Vec<QueuedCrash>, CrashError> {
    let mut queued = Vec::new();
    if config.database_dir.is_empty() {
        return Ok(queued);
    }

    let dir = PathBuf::from(&config.database_dir);
    if let Ok(entries) = fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "json") {
                if let Ok(contents) = fs::read_to_string(&path) {
                    if let Ok(info) = serde_json::from_str::<serde_json::Value>(&contents) {
                        let report_path_str = info["report_path"].as_str().unwrap_or("").to_string();
                        queued.push(QueuedCrash {
                            info_path: path.to_string_lossy().to_string(),
                            report_path: report_path_str,
                            timestamp: info["timestamp"].as_str().unwrap_or("").to_string(),
                            message: info["message"].as_str().unwrap_or("").to_string(),
                            submitted: false,
                        });
                    }
                }
            }
        }
    }

    queued.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(queued)
}

/// A queued crash report awaiting upload.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueuedCrash {
    pub info_path: String,
    pub report_path: String,
    pub timestamp: String,
    pub message: String,
    pub submitted: bool,
}

impl QueuedCrash {
    pub fn info_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.info_path)
    }
    pub fn report_path_buf(&self) -> PathBuf {
        PathBuf::from(&self.report_path)
    }
}
