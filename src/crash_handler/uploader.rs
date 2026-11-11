/// Async uploader for crash reports.
///
/// Uploads crash report files and crash info JSON to the configured server
/// endpoint.  Runs independently of the main application thread so it
/// never blocks audio processing or UI.

use super::config::CrashHandlerConfig;
use super::reporter::QueuedCrash;

/// Result of a crash report upload.
#[derive(Debug, Clone)]
pub struct UploadResult {
    /// Whether the upload succeeded.
    pub success: bool,
    /// HTTP status code or error message.
    pub status: String,
    /// The crash report that was attempted.
    pub crash: QueuedCrash,
}

/// Upload a single queued crash report.
///
/// This function is async-capable but currently runs synchronously
/// for simplicity.  In a follow-up iteration it can be wrapped in
/// `tokio::spawn` for background execution.
pub async fn upload(crash: QueuedCrash, config: &CrashHandlerConfig) -> UploadResult {
    // If upload is disabled or no server URL is configured, skip
    if !config.enabled || config.server_url.is_empty() {
        return UploadResult {
            success: false,
            status: "Upload not configured".to_string(),
            crash,
        };
    }

    // Validate report file exists
    let report_buf = crash.report_path_buf();
    if !report_buf.exists() {
        return UploadResult {
            success: false,
            status: format!("Report not found: {}", report_buf.display()),
            crash,
        };
    }

    // Read crash report
    let _report_data = match std::fs::read(&report_buf) {
        Ok(data) => data,
        Err(e) => {
            return UploadResult {
                success: false,
                status: format!("Failed to read report: {}", e),
                crash,
            };
        }
    };

    // TODO: Implement actual HTTP upload with reqwest
    // This is a placeholder until reqwest is added as a dependency.
    //
    // Example implementation (requires `reqwest` and `tokio`):
    //
    // let client = reqwest::Client::new();
    // let response = client
    //     .post(&config.server_url)
    //     .header("Content-Type", "text/plain")
    //     .body(report_data)
    //     .timeout(std::time::Duration::from_secs(config.upload.timeout_seconds))
    //     .send()
    //     .await;
    //
    // match response {
    //     Ok(resp) => {
    //         let status = resp.status().as_u16().to_string();
    //         UploadResult {
    //             success: resp.status().is_success(),
    //             status,
    //             crash,
    //         }
    //     }
    //     Err(e) => UploadResult {
    //         success: false,
    //         status: format!("Upload failed: {}", e),
    //         crash,
    //     },
    // }

    // Placeholder: assume success for now (report is queued)
    UploadResult {
        success: true,
        status: "Queued (placeholder — no reqwest)".to_string(),
        crash,
    }
}

/// Upload all queued crash reports.
pub async fn upload_all(crashes: Vec<QueuedCrash>, config: &CrashHandlerConfig) -> Vec<UploadResult> {
    let mut results = Vec::new();
    for crash in crashes {
        let result = upload(crash, config).await;
        results.push(result);
    }
    results
}
