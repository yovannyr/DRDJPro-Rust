//! Cloud sync — device registration and status tracking.
//!
//! Manages the local device identity and its synchronization state
//! with the cloud. Part of the Cloud-Sync feature (P02a).
//!
//! ## Design principles
//!
//! * **Device identity** — Each installation gets a unique device ID.
//! * **Sync state machine** — Devices transition through well-defined states.
//! * **Thread-safe** — State changes are Mutex-protected.
//! * **Offline-first** — Device works without network; syncs when available.

use std::fmt;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Unique device identifier (UUID).
pub type DeviceId = String;

/// Sync status of the device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeviceStatus {
    /// Device is offline, no sync possible.
    #[default]
    Offline,
    /// Device is registered with the cloud.
    Registered,
    /// Device is syncing data.
    Syncing,
    /// Device sync is complete and up to date.
    Synced,
    /// Sync conflict detected.
    Conflict,
    /// Device is suspended (user paused sync).
    Suspended,
}

impl fmt::Display for DeviceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DeviceStatus::Offline => write!(f, "offline"),
            DeviceStatus::Registered => write!(f, "registered"),
            DeviceStatus::Syncing => write!(f, "syncing"),
            DeviceStatus::Synced => write!(f, "synced"),
            DeviceStatus::Conflict => write!(f, "conflict"),
            DeviceStatus::Suspended => write!(f, "suspended"),
        }
    }
}

/// Device info for registration.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    /// Unique device ID.
    pub device_id: DeviceId,
    /// Device name (hostname or user-provided).
    pub device_name: String,
    /// Platform identifier (e.g., "windows-x64").
    pub platform: String,
    /// Application version.
    pub app_version: String,
    /// Whether the device is currently online.
    pub is_online: bool,
}

impl Default for DeviceInfo {
    fn default() -> Self {
        Self {
            device_id: generate_device_id(),
            device_name: "Unknown Device".to_string(),
            platform: std::env::consts::OS.to_string(),
            app_version: "0.0.0".to_string(),
            is_online: false,
        }
    }
}

/// Sync category for selective sync operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SyncCategory {
    /// Playlists and their metadata.
    Playlists,
    /// Track analysis data (BPM, key, energy, beatgrid).
    Analysis,
    /// User preferences and settings.
    Preferences,
    /// Custom loops and hot cues.
    Markers,
    /// Library metadata.
    Library,
}

impl fmt::Display for SyncCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncCategory::Playlists => write!(f, "playlists"),
            SyncCategory::Analysis => write!(f, "analysis"),
            SyncCategory::Preferences => write!(f, "preferences"),
            SyncCategory::Markers => write!(f, "markers"),
            SyncCategory::Library => write!(f, "library"),
        }
    }
}

/// Selection rules for selective sync.
#[derive(Debug, Clone)]
pub struct SelectionRules {
    /// Categories enabled for sync.
    pub enabled_categories: Vec<SyncCategory>,
    /// Maximum size per category (in MB).
    pub max_size_mb: std::collections::HashMap<SyncCategory, u64>,
}

impl Default for SelectionRules {
    fn default() -> Self {
        Self {
            enabled_categories: vec![
                SyncCategory::Playlists,
                SyncCategory::Analysis,
                SyncCategory::Preferences,
                SyncCategory::Markers,
                SyncCategory::Library,
            ],
            max_size_mb: {
                let mut map = std::collections::HashMap::new();
                map.insert(SyncCategory::Playlists, 10);
                map.insert(SyncCategory::Analysis, 100);
                map.insert(SyncCategory::Preferences, 1);
                map.insert(SyncCategory::Markers, 50);
                map.insert(SyncCategory::Library, 200);
                map
            },
        }
    }
}

/// Account info from the cloud provider.
#[derive(Debug, Clone)]
pub struct AccountInfo {
    /// Account ID from the cloud.
    pub account_id: String,
    /// Display name.
    pub display_name: String,
    /// Email or username.
    pub username: String,
    /// Whether the account is currently authenticated.
    pub is_authenticated: bool,
}

/// Sync queue entry for offline operations.
#[derive(Debug, Clone)]
pub struct SyncQueueEntry {
    /// Category being synced.
    pub category: SyncCategory,
    /// Operation type.
    pub operation: SyncOperation,
    /// Timestamp when the entry was queued.
    pub queued_at: std::time::Instant,
}

/// Sync operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncOperation {
    Upload,
    Download,
    Delete,
}

impl fmt::Display for SyncOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SyncOperation::Upload => write!(f, "upload"),
            SyncOperation::Download => write!(f, "download"),
            SyncOperation::Delete => write!(f, "delete"),
        }
    }
}

/// Device sync manager.
pub struct DeviceManager {
    info: Arc<Mutex<DeviceInfo>>,
    status: Arc<Mutex<DeviceStatus>>,
    selection_rules: Arc<Mutex<SelectionRules>>,
    sync_queue: Arc<Mutex<Vec<SyncQueueEntry>>>,
}

impl DeviceManager {
    /// Create a new device manager.
    pub fn new() -> Self {
        Self {
            info: Arc::new(Mutex::new(DeviceInfo::default())),
            status: Arc::new(Mutex::new(DeviceStatus::Offline)),
            selection_rules: Arc::new(Mutex::new(SelectionRules::default())),
            sync_queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Register the device with the cloud.
    ///
    /// Transitions the device from `Offline` to `Registered`.
    pub fn register(&self, device_name: Option<String>) -> Result<DeviceInfo, String> {
        let mut info = self.info.lock().map_err(|e| e.to_string())?;
        info.device_name = device_name.unwrap_or_else(|| {
            match std::env::consts::OS {
                "windows" => "Windows Device",
                "macos" => "macOS Device",
                "linux" => "Linux Device",
                _ => "Unknown Device",
            }
            .to_string()
        });
        info.platform = std::env::consts::OS.to_string();
        info.app_version = env!("CARGO_PKG_VERSION").to_string();

        *self.status.lock().map_err(|e| e.to_string())? = DeviceStatus::Registered;
        let device_info = info.clone();

        Ok(device_info)
    }

    /// Set the device online/offline.
    pub fn set_online(&self, online: bool) -> Result<(), String> {
        let mut info = self.info.lock().map_err(|e| e.to_string())?;
        info.is_online = online;

        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        match (online, *status) {
            (true, DeviceStatus::Offline) => *status = DeviceStatus::Registered,
            (false, _) => *status = DeviceStatus::Offline,
            _ => {}
        }

        Ok(())
    }

    /// Set the sync state.
    pub fn set_syncing(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        *status = DeviceStatus::Syncing;
        Ok(())
    }

    /// Mark sync as complete.
    pub fn mark_synced(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        *status = DeviceStatus::Synced;
        Ok(())
    }

    /// Mark a sync conflict.
    pub fn mark_conflict(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        *status = DeviceStatus::Conflict;
        Ok(())
    }

    /// Suspend sync.
    pub fn suspend(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        *status = DeviceStatus::Suspended;
        Ok(())
    }

    /// Resume sync after suspension.
    pub fn resume(&self) -> Result<(), String> {
        let mut status = self.status.lock().map_err(|e| e.to_string())?;
        if matches!(*status, DeviceStatus::Suspended | DeviceStatus::Conflict) {
            *status = DeviceStatus::Registered;
        }
        Ok(())
    }

    /// Get the device info.
    pub fn get_info(&self) -> Result<DeviceInfo, String> {
        self.info
            .lock()
            .map(|info| info.clone())
            .map_err(|e| e.to_string())
    }

    /// Get the device status.
    pub fn get_status(&self) -> Result<DeviceStatus, String> {
        self.status
            .lock()
            .map(|status| *status)
            .map_err(|e| e.to_string())
    }

    /// Get enabled sync categories.
    pub fn get_enabled_categories(&self) -> Result<Vec<SyncCategory>, String> {
        self.selection_rules
            .lock()
            .map(|rules| rules.enabled_categories.clone())
            .map_err(|e| e.to_string())
    }

    /// Set selective sync categories.
    pub fn set_sync_categories(&self, categories: Vec<SyncCategory>) -> Result<(), String> {
        let mut rules = self.selection_rules.lock().map_err(|e| e.to_string())?;
        rules.enabled_categories = categories;
        Ok(())
    }

    /// Queue an offline sync operation.
    pub fn queue_operation(
        &self,
        category: SyncCategory,
        operation: SyncOperation,
    ) -> Result<(), String> {
        let mut queue = self.sync_queue.lock().map_err(|e| e.to_string())?;
        queue.push(SyncQueueEntry {
            category,
            operation,
            queued_at: std::time::Instant::now(),
        });
        Ok(())
    }

    /// Dequeue and return the next pending operation.
    pub fn dequeue_operation(&self) -> Option<SyncQueueEntry> {
        let mut queue = self.sync_queue.lock().unwrap();
        if queue.is_empty() {
            None
        } else {
            Some(queue.remove(0))
        }
    }

    /// Get the number of pending queue entries.
    pub fn queue_length(&self) -> Result<usize, String> {
        self.sync_queue
            .lock()
            .map(|queue| queue.len())
            .map_err(|e| e.to_string())
    }

    /// Clear the sync queue.
    pub fn clear_queue(&self) -> Result<(), String> {
        self.sync_queue
            .lock()
            .map(|mut queue| queue.clear())
            .map_err(|e| e.to_string())
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate a unique device ID.
fn generate_device_id() -> DeviceId {
    // In production, use a proper UUID generator (uuid crate).
    // For now, use a deterministic hash of hostname + process ID.
    let hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".to_string());
    let pid = std::process::id();
    let version = env!("CARGO_PKG_VERSION");

    // Simple hash for deterministic device IDs in tests.
    let mut hash: u64 = 0;
    for byte in format!("{}-{}-{}", hostname, pid, version).bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }

    format!("dev-{:016x}", hash)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_id_format() {
        let id = generate_device_id();
        assert!(id.starts_with("dev-"));
        assert_eq!(id.len(), 20); // "dev-" + 16 hex chars
    }

    #[test]
    fn device_manager_default_state() {
        let manager = DeviceManager::new();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Offline);
    }

    #[test]
    fn device_manager_registration() {
        let manager = DeviceManager::new();
        let info = manager.register(Some("Test Deck".to_string())).unwrap();

        assert_eq!(info.device_name, "Test Deck");
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Registered);
    }

    #[test]
    fn device_status_transitions() {
        let manager = DeviceManager::new();

        // Register.
        manager.register(None).unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Registered);

        // Syncing.
        manager.set_syncing().unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Syncing);

        // Synced.
        manager.mark_synced().unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Synced);
    }

    #[test]
    fn device_conflict_and_suspend() {
        let manager = DeviceManager::new();

        manager.register(None).unwrap();
        manager.mark_conflict().unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Conflict);

        manager.resume().unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Registered);

        manager.suspend().unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Suspended);
    }

    #[test]
    fn device_online_offline() {
        let manager = DeviceManager::new();

        manager.register(None).unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Registered);

        // Go offline.
        manager.set_online(false).unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Offline);

        // Go online again.
        manager.set_online(true).unwrap();
        assert_eq!(manager.get_status().unwrap(), DeviceStatus::Registered);
    }

    #[test]
    fn sync_queue_operations() {
        let manager = DeviceManager::new();

        assert_eq!(manager.queue_length().unwrap(), 0);

        manager
            .queue_operation(SyncCategory::Playlists, SyncOperation::Upload)
            .unwrap();
        assert_eq!(manager.queue_length().unwrap(), 1);

        let entry = manager.dequeue_operation().unwrap();
        assert_eq!(entry.category, SyncCategory::Playlists);
        assert_eq!(entry.operation, SyncOperation::Upload);
        assert_eq!(manager.queue_length().unwrap(), 0);
    }

    #[test]
    fn sync_category_display() {
        assert_eq!(format!("{}", SyncCategory::Playlists), "playlists");
        assert_eq!(format!("{}", SyncCategory::Analysis), "analysis");
        assert_eq!(format!("{}", SyncCategory::Preferences), "preferences");
        assert_eq!(format!("{}", SyncCategory::Markers), "markers");
        assert_eq!(format!("{}", SyncCategory::Library), "library");
    }

    #[test]
    fn selection_rules_default_all_enabled() {
        let manager = DeviceManager::new();
        let categories = manager.get_enabled_categories().unwrap();
        assert_eq!(categories.len(), 5);
        assert!(categories.contains(&SyncCategory::Playlists));
        assert!(categories.contains(&SyncCategory::Library));
    }

    #[test]
    fn device_info_default_values() {
        let info = DeviceInfo::default();
        assert!(info.device_id.starts_with("dev-"));
        assert!(!info.device_name.is_empty());
        assert!(!info.platform.is_empty());
        assert!(!info.app_version.is_empty());
    }

    #[test]
    fn device_registration_sets_version() {
        let manager = DeviceManager::new();
        let info = manager.register(None).unwrap();
        assert_eq!(info.app_version, env!("CARGO_PKG_VERSION"));
    }
}
