//! Auto-update module — GitHub Releases integration.
//!
//! Checks for updates via the GitHub Releases API, displays version
//! information, notifies the user of available updates, and supports
//! manual update checks. Requires G02 (external access proof).
//!
//! ## Design principles
//!
//! * **Read-only update check** — This module only checks for and
//!   notifies about updates. Actual download/install is handled by
//!   P01b.
//! * **Channel support** — Stable and beta release channels.
//! * **Non-blocking** — Update checks run on a background timer.
//! * **User-initiated** — No automatic installation; the user must
//!   confirm before any action.

use std::fmt;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Current application version.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// GitHub repository (owner/repo).
pub const GITHUB_REPO: &str = "yovannyr/DRDJPro-Rust";
/// Default update check interval.
pub const DEFAULT_CHECK_INTERVAL: Duration = Duration::from_secs(86400);
/// Minimum time between consecutive check attempts (prevents spam).
pub const MIN_CHECK_GAP: Duration = Duration::from_secs(300);

/// Release channel for update checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReleaseChannel {
    #[default]
    Stable,
    Beta,
}

impl fmt::Display for ReleaseChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReleaseChannel::Stable => write!(f, "stable"),
            ReleaseChannel::Beta => write!(f, "beta"),
        }
    }
}

/// A release available on GitHub.
#[derive(Debug, Clone)]
pub struct Release {
    /// Version tag (e.g. "1.2.0").
    pub version: String,
    /// Release name/title.
    pub name: String,
    /// Whether this is a pre-release (beta).
    pub pre_release: bool,
    /// Release body (changelog).
    pub body: Option<String>,
    /// Download URLs for assets.
    pub download_urls: Vec<String>,
    /// Published timestamp (ISO 8601).
    pub published_at: Option<String>,
}

impl Release {
    /// Check if this release is newer than the current version.
    pub fn is_newer_than(&self, current: &str) -> bool {
        parse_version(&self.version) > parse_version(current)
    }

    /// Format a user-readable version string.
    pub fn display_version(&self) -> String {
        if self.pre_release {
            format!("{}-beta", self.version)
        } else {
            self.version.clone()
        }
    }
}

/// Update check result.
#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    /// Whether an update is available.
    pub update_available: bool,
    /// The latest release (if any).
    pub latest_release: Option<Release>,
    /// Current application version.
    pub current_version: String,
    /// Error message if the check failed.
    pub error: Option<String>,
}

impl UpdateCheckResult {
    /// Check if the result indicates success.
    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Format a user message about the update status.
    pub fn status_message(&self) -> String {
        match (&self.latest_release, self.update_available) {
            (Some(release), true) => format!("Update {} is available!", release.display_version()),
            (Some(_), false) => "You are running the latest version.".to_string(),
            (None, true) => "Unable to check for updates.".to_string(),
            (None, false) => "No update information available.".to_string(),
        }
    }
}

/// Update notification for the user.
#[derive(Debug, Clone)]
pub struct UpdateNotification {
    /// Whether to show the notification.
    pub show: bool,
    /// Title of the notification.
    pub title: String,
    /// Body message.
    pub body: String,
    /// The available release.
    pub release: Option<Release>,
}

impl UpdateNotification {
    /// Create a notification from a check result.
    pub fn from_result(result: &UpdateCheckResult) -> Self {
        if let Some(release) = &result.latest_release {
            if result.update_available {
                return Self {
                    show: true,
                    title: format!("Update {} Available", release.display_version()),
                    body: release
                        .body
                        .clone()
                        .unwrap_or_else(|| "See release notes for details.".to_string()),
                    release: Some(release.clone()),
                };
            }
        }
        Self {
            show: false,
            title: "No Update Available".to_string(),
            body: result.status_message(),
            release: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Version comparison
// ---------------------------------------------------------------------------

/// Parse a version string into components (major, minor, patch).
///
/// Handles formats like "1.2.3", "1.2.3-beta", "v1.2.3".
fn parse_version(version: &str) -> (u32, u32, u32) {
    let cleaned = version
        .trim()
        .trim_start_matches('v')
        .trim_start_matches('v')
        .split('-')
        .next()
        .unwrap_or(version);

    let mut parts = cleaned.split('.');
    let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);

    (major, minor, patch)
}

/// Compare two version tuples.
fn version_gt(a: (u32, u32, u32), b: (u32, u32, u32)) -> bool {
    a.0 > b.0 || (a.0 == b.0 && a.1 > b.1) || (a.0 == b.0 && a.1 == b.1 && a.2 > b.2)
}

// ---------------------------------------------------------------------------
// Update checker
// ---------------------------------------------------------------------------

/// Configuration for the update checker.
pub struct Config {
    /// GitHub repository (owner/repo).
    pub github_repo: String,
    /// Release channel (stable/beta).
    pub channel: ReleaseChannel,
    /// API base URL (GitHub).
    pub api_base_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            github_repo: GITHUB_REPO.to_string(),
            channel: ReleaseChannel::default(),
            api_base_url: "https://api.github.com".to_string(),
        }
    }
}

/// Checks for updates via GitHub Releases API.
pub struct UpdateChecker {
    config: Config,
    last_check: std::sync::Mutex<Option<Instant>>,
    last_result: std::sync::Mutex<Option<UpdateCheckResult>>,
}

impl UpdateChecker {
    /// Create a new update checker.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            last_check: std::sync::Mutex::new(None),
            last_result: std::sync::Mutex::new(None),
        }
    }

    /// Check for updates (requires network access).
    ///
    /// This is a stub implementation — in production it would call
    /// the GitHub Releases API and parse the JSON response.
    pub fn check(&self) -> UpdateCheckResult {
        let now = Instant::now();

        // Enforce minimum check gap.
        {
            let last = self.last_check.lock().unwrap();
            if let Some(last_check) = *last {
                if now.duration_since(last_check) < MIN_CHECK_GAP {
                    // Keep the cached release data and report that no new check ran.
                    let mut cached =
                        self.last_result
                            .lock()
                            .unwrap()
                            .clone()
                            .unwrap_or(UpdateCheckResult {
                                update_available: false,
                                latest_release: None,
                                current_version: APP_VERSION.to_string(),
                                error: None,
                            });
                    cached.error = Some("Check interval too short".to_string());
                    return cached;
                }
            }
        }

        // In production, this would:
        // 1. HTTP GET {api_base_url}/repos/{github_repo}/releases
        // 2. Filter by pre_release based on channel
        // 3. Parse JSON to Release structs
        // 4. Compare versions
        //
        // For now, return a simulated result for testing.
        let result = self.simulated_check();

        *self.last_check.lock().unwrap() = Some(now);
        *self.last_result.lock().unwrap() = Some(result.clone());

        result
    }

    /// Get the last check result (cached).
    pub fn last_result(&self) -> Option<UpdateCheckResult> {
        self.last_result.lock().unwrap().clone()
    }

    /// Get the time since last check.
    pub fn time_since_last_check(&self) -> Option<Duration> {
        self.last_check
            .lock()
            .unwrap()
            .map(|instant| Instant::now().duration_since(instant))
    }

    /// Check if manual check is allowed.
    pub fn can_check_now(&self) -> bool {
        self.last_check
            .lock()
            .unwrap()
            .is_none_or(|last| Instant::now().duration_since(last) >= MIN_CHECK_GAP)
    }

    /// Simulated check for testing (no network required).
    fn simulated_check(&self) -> UpdateCheckResult {
        // In production: fetch from GitHub API.
        // For testing: return current version as latest.
        UpdateCheckResult {
            update_available: false,
            latest_release: Some(Release {
                version: APP_VERSION.to_string(),
                name: format!("Version {}", APP_VERSION),
                pre_release: false,
                body: Some("Current stable release.".to_string()),
                download_urls: Vec::new(),
                published_at: None,
            }),
            current_version: APP_VERSION.to_string(),
            error: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Update timer — periodic background checks
// ---------------------------------------------------------------------------

/// Background update checker with periodic intervals.
pub struct UpdateTimer {
    checker: std::sync::Arc<UpdateChecker>,
    interval: Duration,
    running: std::sync::Arc<std::sync::atomic::AtomicBool>,
    worker: std::sync::Mutex<Option<std::thread::JoinHandle<()>>>,
    stopped: std::sync::Mutex<Option<std::sync::mpsc::SyncSender<()>>>,
}
impl UpdateTimer {
    pub fn new(checker: UpdateChecker, interval: Duration) -> Self {
        Self {
            checker: std::sync::Arc::new(checker),
            interval: interval.max(Duration::from_millis(100)),
            running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            worker: std::sync::Mutex::new(None),
            stopped: std::sync::Mutex::new(None),
        }
    }
    /// Start one background worker; stop wakes its interval wait immediately.
    pub fn start(&self) {
        let mut worker = self.worker.lock().unwrap();
        if self.running.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(previous) = worker.take() {
            let _ = previous.join();
        }
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        *self.stopped.lock().unwrap() = Some(sender);
        let checker = std::sync::Arc::clone(&self.checker);
        let running = std::sync::Arc::clone(&self.running);
        let interval = self.interval;
        *worker = Some(std::thread::spawn(move || {
            loop {
                let result = checker.check();
                if result.update_available {
                    let notification = UpdateNotification::from_result(&result);
                    if notification.show {
                        eprintln!("[Update] {}\n{}", notification.title, notification.body);
                    }
                }
                match receiver.recv_timeout(interval) {
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    _ => break,
                }
            }
            running.store(false, Ordering::Release);
        }));
    }
    /// Join the control-plane worker. Never call from an audio callback.
    pub fn stop(&self) {
        let mut worker = self.worker.lock().unwrap();
        if let Some(sender) = self.stopped.lock().unwrap().take() {
            let _ = sender.try_send(());
        }
        if let Some(handle) = worker.take() {
            let _ = handle.join();
        }
        self.running.store(false, Ordering::Release);
    }
}
impl Drop for UpdateTimer {
    fn drop(&mut self) {
        self.stop();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_zero_interval_is_bounded_and_worker_can_restart_after_stop() {
        let timer = UpdateTimer::new(UpdateChecker::new(Config::default()), Duration::ZERO);
        assert_eq!(timer.interval, Duration::from_millis(100));
        for _ in 0..2 {
            timer.start();
            assert!(timer.running.load(Ordering::Acquire));
            timer.stop();
            assert!(!timer.running.load(Ordering::Acquire));
            assert!(timer.worker.lock().unwrap().is_none());
        }
    }

    #[test]
    fn parse_version_formats() {
        assert_eq!(parse_version("1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("v1.2.3"), (1, 2, 3));
        assert_eq!(parse_version("1.2.3-beta"), (1, 2, 3));
        assert_eq!(parse_version("0.1.0"), (0, 1, 0));
        assert_eq!(parse_version("2.0.0-rc1"), (2, 0, 0));
    }

    #[test]
    fn version_comparison() {
        assert!(version_gt((1, 1, 0), (1, 0, 0)));
        assert!(version_gt((2, 0, 0), (1, 9, 9)));
        assert!(version_gt((1, 2, 3), (1, 2, 2)));
        assert!(!version_gt((1, 0, 0), (1, 0, 0)));
        assert!(!version_gt((1, 0, 0), (2, 0, 0)));
    }

    #[test]
    fn release_is_newer_than() {
        let release = Release {
            version: "1.2.0".to_string(),
            name: "Test Release".to_string(),
            pre_release: false,
            body: None,
            download_urls: Vec::new(),
            published_at: None,
        };

        assert!(release.is_newer_than("1.1.0"));
        assert!(release.is_newer_than("1.0.0"));
        assert!(!release.is_newer_than("1.2.0"));
        assert!(!release.is_newer_than("2.0.0"));
    }

    #[test]
    fn release_display_version() {
        let stable = Release {
            version: "1.2.0".to_string(),
            name: "Stable".to_string(),
            pre_release: false,
            body: None,
            download_urls: Vec::new(),
            published_at: None,
        };
        assert_eq!(stable.display_version(), "1.2.0");

        let beta = Release {
            version: "1.3.0".to_string(),
            name: "Beta".to_string(),
            pre_release: true,
            body: None,
            download_urls: Vec::new(),
            published_at: None,
        };
        assert_eq!(beta.display_version(), "1.3.0-beta");
    }

    #[test]
    fn update_check_result_status_message() {
        // Update available.
        let result = UpdateCheckResult {
            update_available: true,
            latest_release: Some(Release {
                version: "2.0.0".to_string(),
                name: "New Version".to_string(),
                pre_release: false,
                body: None,
                download_urls: Vec::new(),
                published_at: None,
            }),
            current_version: APP_VERSION.to_string(),
            error: None,
        };
        assert!(result.status_message().contains("2.0.0"));
        assert!(result.status_message().contains("available"));

        // No update.
        let result = UpdateCheckResult {
            update_available: false,
            latest_release: Some(Release {
                version: APP_VERSION.to_string(),
                name: "Current".to_string(),
                pre_release: false,
                body: None,
                download_urls: Vec::new(),
                published_at: None,
            }),
            current_version: APP_VERSION.to_string(),
            error: None,
        };
        assert!(result.status_message().contains("latest"));

        // Error.
        let result = UpdateCheckResult {
            update_available: true,
            latest_release: None,
            current_version: APP_VERSION.to_string(),
            error: Some("Network error".to_string()),
        };
        assert!(result.status_message().contains("Unable"));
    }

    #[test]
    fn update_notification_from_result() {
        let result = UpdateCheckResult {
            update_available: true,
            latest_release: Some(Release {
                version: "2.0.0".to_string(),
                name: "New".to_string(),
                pre_release: false,
                body: Some("New features!".to_string()),
                download_urls: Vec::new(),
                published_at: None,
            }),
            current_version: APP_VERSION.to_string(),
            error: None,
        };

        let notification = UpdateNotification::from_result(&result);
        assert!(notification.show);
        assert_eq!(notification.title, "Update 2.0.0 Available");
        assert!(notification.body.contains("New features"));
    }

    #[test]
    fn update_checker_returns_current_version() {
        let checker = UpdateChecker::new(Config::default());
        let result = checker.check();

        assert!(result.is_success());
        assert_eq!(result.current_version, APP_VERSION);
        assert!(!result.update_available);
    }

    #[test]
    fn update_checker_respects_min_check_gap() {
        let checker = UpdateChecker::new(Config::default());

        // First check.
        let _ = checker.check();

        // Second check immediately should return cached with error.
        let result = checker.check();
        assert!(result.error.is_some());
        assert!(result.error.as_ref().unwrap().contains("short"));
    }

    #[test]
    fn update_checker_can_check_now() {
        let checker = UpdateChecker::new(Config::default());

        // Can check immediately.
        assert!(checker.can_check_now());

        // After check, cannot check again immediately.
        let _ = checker.check();
        assert!(!checker.can_check_now());
    }
}
