/// Configuration for the crash handler subsystem.

use serde::{Deserialize, Serialize};

/// Configuration for the crash handler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrashHandlerConfig {
    /// Whether crash reporting is enabled.
    pub enabled: bool,
    /// URL to upload crash reports to.
    pub server_url: String,
    /// Directory to store local crash minidumps.
    pub database_dir: String,
    /// Whether to monitor the child process.
    pub monitor_self: bool,
    /// Annotations attached to crash reports.
    pub annotation: CrashAnnotation,
    /// Upload settings.
    pub upload: UploadConfig,
    /// Whether to automatically send reports on next startup.
    pub auto_submit: bool,
}

impl Default for CrashHandlerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            server_url: String::new(),
            database_dir: String::new(),
            monitor_self: false,
            annotation: CrashAnnotation::default(),
            upload: UploadConfig::default(),
            auto_submit: false,
        }
    }
}

/// Annotations attached to crash reports.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CrashAnnotation {
    /// Application version string.
    pub version: String,
    /// Detected operating system.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Optional release channel (stable, beta).
    pub release_channel: Option<String>,
}

/// Upload configuration for crash reports.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadConfig {
    /// Whether to gzip-compress uploads.
    pub gzip: bool,
    /// Whether to disable rate limiting.
    pub no_rate_limit: bool,
    /// Maximum number of queued reports.
    pub max_queue_size: usize,
    /// Upload timeout in seconds.
    pub timeout_seconds: u64,
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            gzip: false,
            no_rate_limit: false,
            max_queue_size: 50,
            timeout_seconds: 10,
        }
    }
}
