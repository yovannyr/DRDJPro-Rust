/// Persistent queue for crash reports.
///
/// Stores crash report metadata (path, timestamp, message) in a local
/// JSON file so that reports can be submitted on the next application
/// startup.

use super::config::CrashHandlerConfig;
use super::reporter::QueuedCrash;
use std::fs;
use std::path::PathBuf;

/// The crash queue file stored alongside crash reports.
const QUEUE_FILE: &str = "crash_queue.json";

/// Load the crash queue from disk.
pub fn load(config: &CrashHandlerConfig) -> Result<(), super::CrashError> {
    if config.database_dir.is_empty() {
        return Ok(());
    }

    let queue_path = PathBuf::from(&config.database_dir).join(QUEUE_FILE);
    if queue_path.exists() {
        let contents = fs::read_to_string(&queue_path)
            .map_err(|e| super::CrashError::Io(e))?;
        let _queue: Result<Vec<QueuedCrash>, _> = serde_json::from_str(&contents);
        // Process queued reports on next startup (TODO)
    }

    Ok(())
}

/// Load all queued crash reports from the queue file.
pub fn load_all(config: &CrashHandlerConfig) -> Result<Vec<QueuedCrash>, super::CrashError> {
    let queue_path = PathBuf::from(&config.database_dir).join(QUEUE_FILE);
    if !queue_path.exists() {
        return Ok(Vec::new());
    }

    let contents = fs::read_to_string(&queue_path)
        .map_err(|e| super::CrashError::Io(e))?;
    let crashes: Vec<QueuedCrash> = serde_json::from_str(&contents)
        .map_err(|e| super::CrashError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("JSON deserialization failed: {}", e),
        )))?;

    Ok(crashes)
}

/// Save crash reports to the queue file.
pub fn save(crashes: &[QueuedCrash], config: &CrashHandlerConfig) -> Result<(), super::CrashError> {
    if config.database_dir.is_empty() {
        return Ok(());
    }

    // Enforce max queue size
    let crashes: Vec<_> = if crashes.len() > config.upload.max_queue_size {
        crashes[..config.upload.max_queue_size].to_vec()
    } else {
        crashes.to_vec()
    };

    let queue_path = PathBuf::from(&config.database_dir).join(QUEUE_FILE);
    let contents = serde_json::to_string_pretty(&crashes)
        .map_err(|e| super::CrashError::Io(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("JSON serialization failed: {}", e),
        )))?;

    fs::write(&queue_path, contents)
        .map_err(super::CrashError::Io)?;

    Ok(())
}

/// Get all queued crash reports that have not been submitted.
pub fn get_pending(config: &CrashHandlerConfig) -> Result<Vec<QueuedCrash>, super::CrashError> {
    let queued = load_from_queue_file(config)?;
    Ok(queued.into_iter().filter(|c| !c.submitted).collect())
}

/// Mark a crash report as submitted and remove it from the queue.
pub fn mark_submitted(crash: &QueuedCrash, config: &CrashHandlerConfig) -> Result<(), super::CrashError> {
    let all = load_from_queue_file(config)?;
    let pending = all
        .into_iter()
        .filter(|c| {
            !(c.info_path == crash.info_path
                && c.timestamp == crash.timestamp)
        })
        .collect::<Vec<_>>();
    save(&pending, config)
}

/// Delete a crash report (report + metadata) from disk and the queue.
pub fn delete(crash: &QueuedCrash, config: &CrashHandlerConfig) -> Result<(), super::CrashError> {
    let _ = std::fs::remove_file(&crash.info_path);
    let _ = std::fs::remove_file(&crash.report_path);
    mark_submitted(crash, config)
}
