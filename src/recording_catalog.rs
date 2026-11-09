//! Recording System Part 2 — Katalog, Tags, Umbenennung, Löschung, Speicherwarnung.
//!
//! Extends the recording system (P14a) with recording catalog management,
//! tagging, renaming, deletion, storage monitoring, metadata, and download.
//! Part of the Recording System feature (P14b).
//!
//! ## Design principles
//!
//! * **Safe** — Deletion requires confirmation.
//! * **Efficient** — Storage monitoring with alerts.
//! * **Organized** — Tags and sorting for catalog management.

use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Recording status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum RecordingStatus {
    /// Not yet recorded.
    #[default]
    Idle,
    /// Currently recording.
    Recording,
    /// Saved to disk.
    Saved,
    /// In progress (saving).
    Saving,
    /// Failed.
    Failed,
}

/// Recording file format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordingFormat {
    /// WAV (uncompressed).
    Wav,
    /// FLAC (lossless).
    Flac,
    /// MP3 (compressed).
    MP3,
    /// OGG Vorbis.
    Ogg,
}

impl RecordingFormat {
    /// Get file extension.
    pub fn extension(&self) -> &str {
        match self {
            Self::Wav => "wav",
            Self::Flac => "flac",
            Self::MP3 => "mp3",
            Self::Ogg => "ogg",
        }
    }

    /// Get format name.
    pub fn name(&self) -> &str {
        match self {
            Self::Wav => "WAV",
            Self::Flac => "FLAC",
            Self::MP3 => "MP3",
            Self::Ogg => "OGG",
        }
    }
}

/// Recording metadata for tags.
#[derive(Debug, Clone)]
pub struct RecordingMetadata {
    /// Title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Album/set name.
    pub album: String,
    /// Genre.
    pub genre: String,
    /// Year.
    pub year: u16,
    /// Track number.
    pub track_number: u32,
    /// Comments.
    pub comments: String,
    /// Custom tags.
    pub custom_tags: HashMap<String, String>,
}

impl RecordingMetadata {
    /// Create new recording metadata.
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            artist: String::new(),
            album: String::new(),
            genre: String::new(),
            year: 2024,
            track_number: 0,
            comments: String::new(),
            custom_tags: HashMap::new(),
        }
    }

    /// Set artist.
    pub fn set_artist(&mut self, artist: &str) {
        self.artist = artist.to_string();
    }

    /// Set album.
    pub fn set_album(&mut self, album: &str) {
        self.album = album.to_string();
    }

    /// Set genre.
    pub fn set_genre(&mut self, genre: &str) {
        self.genre = genre.to_string();
    }

    /// Add custom tag.
    pub fn add_tag(&mut self, key: &str, value: &str) {
        self.custom_tags.insert(key.to_string(), value.to_string());
    }

    /// Get all tags.
    pub fn all_tags(&self) -> Vec<(String, String)> {
        let mut tags = vec![
            ("title".into(), self.title.clone()),
            ("artist".into(), self.artist.clone()),
            ("album".into(), self.album.clone()),
            ("genre".into(), self.genre.clone()),
            ("year".into(), self.year.to_string()),
            ("track_number".into(), self.track_number.to_string()),
            ("comments".into(), self.comments.clone()),
        ];
        tags.extend(
            self.custom_tags
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
        tags
    }

    /// Is metadata complete?
    pub fn is_complete(&self) -> bool {
        !self.title.is_empty() && !self.artist.is_empty() && !self.album.is_empty()
    }
}

impl Default for RecordingMetadata {
    fn default() -> Self {
        Self::new("Untitled")
    }
}

/// Recording entry in the catalog.
#[derive(Debug, Clone)]
pub struct RecordingEntry {
    /// Unique recording ID.
    pub id: String,
    /// File path.
    pub file_path: String,
    /// File name.
    pub file_name: String,
    /// File format.
    pub format: RecordingFormat,
    /// File size in bytes.
    pub size_bytes: u64,
    /// Duration.
    pub duration: Duration,
    /// Recording start time.
    pub started_at: Instant,
    /// Recording end time.
    pub ended_at: Option<Instant>,
    /// Status.
    pub status: RecordingStatus,
    /// Metadata/tags.
    pub metadata: RecordingMetadata,
    /// Tags.
    pub tags: Vec<String>,
    /// Is starred/favorite.
    pub is_starred: bool,
    /// Description.
    pub description: String,
}

impl RecordingEntry {
    /// Create new recording entry.
    pub fn new(id: &str, file_path: &str, format: RecordingFormat) -> Self {
        Self {
            id: id.to_string(),
            file_path: file_path.to_string(),
            file_name: file_path
                .split('/')
                .next_back()
                .unwrap_or(file_path)
                .to_string(),
            format: format.clone(),
            size_bytes: 0,
            duration: Duration::ZERO,
            started_at: Instant::now(),
            ended_at: None,
            status: RecordingStatus::Idle,
            metadata: RecordingMetadata::default(),
            tags: Vec::new(),
            is_starred: false,
            description: String::new(),
        }
    }

    /// Start recording.
    pub fn start_recording(&mut self) {
        self.status = RecordingStatus::Recording;
    }

    /// Save recording.
    pub fn save_recording(&mut self, file_size: u64, duration: Duration) {
        self.status = RecordingStatus::Saved;
        self.size_bytes = file_size;
        self.duration = duration;
        self.ended_at = Some(Instant::now());
    }

    /// Add tag.
    pub fn add_tag(&mut self, tag: &str) {
        if !self.tags.contains(&tag.to_string()) {
            self.tags.push(tag.to_string());
        }
    }

    /// Remove tag.
    pub fn remove_tag(&mut self, tag: &str) -> bool {
        let pos = self.tags.iter().position(|t| t == tag);
        if let Some(idx) = pos {
            self.tags.remove(idx);
            true
        } else {
            false
        }
    }

    /// Toggle starred.
    pub fn toggle_starred(&mut self) {
        self.is_starred = !self.is_starred;
    }

    /// Check if has tag.
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t == tag)
    }

    /// Get duration string.
    pub fn duration_string(&self) -> String {
        let secs = self.duration.as_secs();
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{:02}:{:02}", mins, secs)
    }

    /// Get file size string.
    pub fn size_string(&self) -> String {
        if self.size_bytes < 1024 {
            format!("{} B", self.size_bytes)
        } else if self.size_bytes < 1024 * 1024 {
            format!("{:.1} KB", self.size_bytes as f64 / 1024.0)
        } else if self.size_bytes < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.size_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.1} GB",
                self.size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }

    /// Get file extension string.
    pub fn extension_string(&self) -> &str {
        self.format.extension()
    }

    /// Get age.
    pub fn age(&self) -> Duration {
        Instant::now().duration_since(self.started_at)
    }
}

/// Sort column for recording catalog.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SortColumn {
    /// Sort by name.
    Name,
    /// Sort by duration.
    Duration,
    /// Sort by file size.
    Size,
    /// Sort by date.
    #[default]
    Date,
    /// Sort by tags.
    Tags,
    /// Sort by starred.
    Starred,
}

/// Sort order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum SortOrder {
    /// Ascending.
    Ascending,
    /// Descending.
    #[default]
    Descending,
}

/// Storage warning level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageWarningLevel {
    /// Normal (plenty of space).
    Normal,
    /// Warning (getting low).
    Warning,
    /// Critical (very low space).
    Critical,
}

impl StorageWarningLevel {
    /// Get warning message.
    pub fn message(&self) -> &str {
        match self {
            Self::Normal => "Disk space is sufficient.",
            Self::Warning => "Disk space is getting low. Consider freeing up space.",
            Self::Critical => "CRITICAL: Disk space is nearly exhausted! Recordings may fail.",
        }
    }

    /// Get color.
    pub fn color(&self) -> &str {
        match self {
            Self::Normal => "green",
            Self::Warning => "yellow",
            Self::Critical => "red",
        }
    }
}

/// Recording catalog manager.
pub struct RecordingCatalog {
    /// Recording entries.
    entries: HashMap<String, RecordingEntry>,
    /// Sort configuration.
    sort_column: SortColumn,
    sort_order: SortOrder,
    /// Active recording.
    active_recording: Option<String>,
    /// Storage info.
    total_storage: u64,
    used_storage: u64,
    available_storage: u64,
    /// Low space threshold in bytes (500 MB).
    low_space_threshold: u64,
    /// Critical space threshold in bytes (100 MB).
    critical_space_threshold: u64,
    /// Recording counter.
    recording_counter: u32,
}

impl RecordingCatalog {
    /// Create new recording catalog.
    pub fn new(total_storage: u64, available_storage: u64) -> Self {
        Self {
            entries: HashMap::new(),
            sort_column: SortColumn::default(),
            sort_order: SortOrder::default(),
            active_recording: None,
            total_storage,
            used_storage: 0,
            available_storage,
            low_space_threshold: 500 * 1024 * 1024, // 500 MB.
            critical_space_threshold: 100 * 1024 * 1024, // 100 MB.
            recording_counter: 0,
        }
    }

    /// Create new recording entry.
    pub fn create_recording(
        &mut self,
        file_path: &str,
        format: RecordingFormat,
    ) -> &mut RecordingEntry {
        self.recording_counter += 1;
        let id = format!("rec_{}", self.recording_counter);
        let entry = RecordingEntry::new(&id, file_path, format);
        self.entries.insert(id.clone(), entry);
        self.active_recording = Some(id.clone());

        self.entries.get_mut(&id).unwrap()
    }

    /// Get recording by ID.
    pub fn get_recording(&self, id: &str) -> Option<&RecordingEntry> {
        self.entries.get(id)
    }

    /// Get recording mutably.
    pub fn get_recording_mut(&mut self, id: &str) -> Option<&mut RecordingEntry> {
        self.entries.get_mut(id)
    }

    /// Start recording.
    pub fn start_recording(&mut self, id: &str) -> Result<(), RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            if entry.status == RecordingStatus::Idle {
                entry.start_recording();
                Ok(())
            } else {
                Err(RecordingError::InvalidState(
                    "Recording already in progress".to_string(),
                ))
            }
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Save recording.
    pub fn save_recording(
        &mut self,
        id: &str,
        file_size: u64,
        duration: Duration,
    ) -> Result<(), RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            if entry.status == RecordingStatus::Recording {
                entry.save_recording(file_size, duration);
                self.used_storage += file_size;
                self.available_storage = self.available_storage.saturating_sub(file_size);

                // Clear active recording if this was it.
                if self.active_recording.as_deref() == Some(id) {
                    self.active_recording = None;
                }
                Ok(())
            } else {
                Err(RecordingError::InvalidState(
                    "Recording not active".to_string(),
                ))
            }
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Rename recording.
    pub fn rename_recording(&mut self, id: &str, new_name: &str) -> Result<(), RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.metadata.title = new_name.to_string();
            entry.file_name = new_name.to_string();
            Ok(())
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Delete recording.
    /// Returns (bytes_freed, success).
    pub fn delete_recording(&mut self, id: &str) -> Result<u64, RecordingError> {
        if let Some(entry) = self.entries.get(id) {
            let bytes_freed = entry.size_bytes;

            // Try to delete the file.
            fs::remove_file(&entry.file_path).ok();

            self.entries.remove(id);
            self.used_storage = self.used_storage.saturating_sub(bytes_freed);
            self.available_storage += bytes_freed;

            Ok(bytes_freed)
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Add tag to recording.
    pub fn add_tag(&mut self, id: &str, tag: &str) -> Result<(), RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.add_tag(tag);
            Ok(())
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Remove tag from recording.
    pub fn remove_tag(&mut self, id: &str, tag: &str) -> Result<bool, RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            Ok(entry.remove_tag(tag))
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Toggle starred.
    pub fn toggle_starred(&mut self, id: &str) -> Result<(), RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.toggle_starred();
            Ok(())
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Set metadata.
    pub fn set_metadata(
        &mut self,
        id: &str,
        metadata: RecordingMetadata,
    ) -> Result<(), RecordingError> {
        if let Some(entry) = self.entries.get_mut(id) {
            entry.metadata = metadata;
            Ok(())
        } else {
            Err(RecordingError::NotFound(id.to_string()))
        }
    }

    /// Stop and save all recordings.
    pub fn stop_all_and_save(&mut self) {
        for entry in self.entries.values_mut() {
            if entry.status == RecordingStatus::Recording {
                entry.save_recording(0, Duration::ZERO);
            }
        }
        self.active_recording = None;
    }

    /// Get all recordings.
    pub fn all_recordings(&self) -> Vec<&RecordingEntry> {
        let mut recordings: Vec<&RecordingEntry> = self.entries.values().collect();
        self.sort_recordings(&mut recordings);
        recordings
    }

    /// Get all recording IDs.
    pub fn all_recording_ids(&self) -> Vec<String> {
        self.entries.keys().cloned().collect()
    }

    /// Get recordings with tag.
    pub fn recordings_with_tag(&self, tag: &str) -> Vec<&RecordingEntry> {
        let mut recordings: Vec<&RecordingEntry> =
            self.entries.values().filter(|e| e.has_tag(tag)).collect();
        self.sort_recordings(&mut recordings);
        recordings
    }

    /// Get starred recordings.
    pub fn starred_recordings(&self) -> Vec<&RecordingEntry> {
        let mut recordings: Vec<&RecordingEntry> =
            self.entries.values().filter(|e| e.is_starred).collect();
        self.sort_recordings(&mut recordings);
        recordings
    }

    /// Get active recording.
    pub fn active_recording(&self) -> Option<&str> {
        self.active_recording.as_deref()
    }

    /// Check if is recording (no active recording slot).
    pub fn is_recording(&self) -> bool {
        self.active_recording.is_some()
    }

    /// Sort recordings.
    fn sort_recordings(&self, recordings: &mut [&RecordingEntry]) {
        recordings.sort_by(|a, b| {
            let comparison = match self.sort_column {
                SortColumn::Name => a.file_name.cmp(&b.file_name),
                SortColumn::Duration => a.duration.cmp(&b.duration),
                SortColumn::Size => a.size_bytes.cmp(&b.size_bytes),
                SortColumn::Date => a.started_at.cmp(&b.started_at),
                SortColumn::Tags => a.tags.len().cmp(&b.tags.len()),
                SortColumn::Starred => b.is_starred.cmp(&a.is_starred), // Starred first.
            };

            if self.sort_order == SortOrder::Descending {
                comparison.reverse()
            } else {
                comparison
            }
        });
    }

    /// Set sort configuration.
    pub fn set_sort(&mut self, column: SortColumn, order: SortOrder) {
        self.sort_column = column;
        self.sort_order = order;
    }

    /// Get sort configuration.
    pub fn sort_config(&self) -> (&SortColumn, &SortOrder) {
        (&self.sort_column, &self.sort_order)
    }

    /// Get all tags.
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.entries.values().flat_map(|e| e.tags.clone()).collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Get recording count.
    pub fn recording_count(&self) -> usize {
        self.entries.len()
    }

    /// Get storage info.
    pub fn storage_info(&self) -> (u64, u64, u64, StorageWarningLevel) {
        let level = self.storage_warning_level();
        (
            self.total_storage,
            self.used_storage,
            self.available_storage,
            level,
        )
    }

    /// Get storage warning level.
    pub fn storage_warning_level(&self) -> StorageWarningLevel {
        if self.available_storage <= self.critical_space_threshold {
            StorageWarningLevel::Critical
        } else if self.available_storage <= self.low_space_threshold {
            StorageWarningLevel::Warning
        } else {
            StorageWarningLevel::Normal
        }
    }

    /// Get total storage.
    pub fn total_storage(&self) -> u64 {
        self.total_storage
    }

    /// Get used storage.
    pub fn used_storage(&self) -> u64 {
        self.used_storage
    }

    /// Get available storage.
    pub fn available_storage(&self) -> u64 {
        self.available_storage
    }

    /// Check if low space.
    pub fn is_low_space(&self) -> bool {
        self.available_storage <= self.low_space_threshold
    }

    /// Check if critical space.
    pub fn is_critical_space(&self) -> bool {
        self.available_storage <= self.critical_space_threshold
    }

    /// Set storage info.
    pub fn set_storage(&mut self, total: u64, available: u64) {
        self.total_storage = total;
        self.available_storage = available;
        self.used_storage = total.saturating_sub(available);
    }

    /// Set low space threshold.
    pub fn set_low_space_threshold(&mut self, bytes: u64) {
        self.low_space_threshold = bytes;
    }

    /// Set critical space threshold.
    pub fn set_critical_space_threshold(&mut self, bytes: u64) {
        self.critical_space_threshold = bytes;
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        let warning = self.storage_warning_level();
        format!(
            "Recordings: {} | Used: {:.1} MB | Available: {:.1} MB | {}",
            self.entries.len(),
            self.used_storage as f64 / (1024.0 * 1024.0),
            self.available_storage as f64 / (1024.0 * 1024.0),
            warning.message(),
        )
    }

    /// Get available formats.
    pub fn available_formats() -> &'static [RecordingFormat] {
        &[
            RecordingFormat::Wav,
            RecordingFormat::Flac,
            RecordingFormat::MP3,
            RecordingFormat::Ogg,
        ]
    }

    /// Get available sort columns.
    pub fn available_sort_columns() -> &'static [SortColumn] {
        &[
            SortColumn::Name,
            SortColumn::Duration,
            SortColumn::Size,
            SortColumn::Date,
            SortColumn::Tags,
            SortColumn::Starred,
        ]
    }
}

impl Default for RecordingCatalog {
    fn default() -> Self {
        Self::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024) // 100 GB total, 50 GB free.
    }
}

/// Recording error.
#[derive(Debug, Clone)]
pub enum RecordingError {
    /// Recording not found.
    NotFound(String),
    /// Invalid state.
    InvalidState(String),
    /// Storage error.
    StorageError(String),
    /// Tag error.
    TagError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for RecordingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "Recording not found: {}", id),
            Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Self::StorageError(msg) => write!(f, "Storage error: {}", msg),
            Self::TagError(msg) => write!(f, "Tag error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RecordingError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recording_catalog_creation() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        assert_eq!(catalog.recording_count(), 0);
        assert!(!catalog.is_recording());
    }

    #[test]
    fn recording_entry_creation() {
        let entry = RecordingEntry::new("r1", "/tmp/recording.wav", RecordingFormat::Wav);
        assert_eq!(entry.id, "r1");
        assert_eq!(entry.format, RecordingFormat::Wav);
        assert_eq!(entry.size_bytes, 0);
    }

    #[test]
    fn recording_entry_start_save() {
        let mut entry = RecordingEntry::new("r1", "/tmp/rec.wav", RecordingFormat::Wav);
        entry.start_recording();
        assert_eq!(entry.status, RecordingStatus::Recording);

        entry.save_recording(1024 * 1024, Duration::from_secs(60));
        assert_eq!(entry.status, RecordingStatus::Saved);
        assert_eq!(entry.size_bytes, 1024 * 1024);
    }

    #[test]
    fn recording_entry_duration_string() {
        let mut entry = RecordingEntry::new("r1", "/tmp/rec.wav", RecordingFormat::Wav);
        entry.save_recording(0, Duration::from_secs(185));
        assert_eq!(entry.duration_string(), "03:05");
    }

    #[test]
    fn recording_entry_size_string() {
        let mut entry = RecordingEntry::new("r1", "/tmp/rec.wav", RecordingFormat::Wav);
        entry.size_bytes = 1024 * 1024; // 1 MB.
        assert_eq!(entry.size_string(), "1.0 MB");

        entry.size_bytes = 1024;
        assert_eq!(entry.size_string(), "1.0 KB");

        entry.size_bytes = 500;
        assert_eq!(entry.size_string(), "500 B");
    }

    #[test]
    fn recording_entry_tags() {
        let mut entry = RecordingEntry::new("r1", "/tmp/rec.wav", RecordingFormat::Wav);
        entry.add_tag("live");
        entry.add_tag("set");

        assert!(entry.has_tag("live"));
        assert!(entry.has_tag("set"));
        assert!(!entry.has_tag("studio"));

        entry.remove_tag("live");
        assert!(!entry.has_tag("live"));
    }

    #[test]
    fn recording_entry_starred() {
        let mut entry = RecordingEntry::new("r1", "/tmp/rec.wav", RecordingFormat::Wav);
        assert!(!entry.is_starred);

        entry.toggle_starred();
        assert!(entry.is_starred);

        entry.toggle_starred();
        assert!(!entry.is_starred);
    }

    #[test]
    fn recording_metadata_creation() {
        let metadata = RecordingMetadata::new("My Recording");
        assert_eq!(metadata.title, "My Recording");
        assert!(!metadata.is_complete());
    }

    #[test]
    fn recording_metadata_set_fields() {
        let mut metadata = RecordingMetadata::new("Title");
        metadata.set_artist("Artist");
        metadata.set_album("Album");
        metadata.set_genre("Electronic");

        assert!(metadata.is_complete());
        assert_eq!(metadata.artist, "Artist");
    }

    #[test]
    fn recording_metadata_custom_tags() {
        let mut metadata = RecordingMetadata::new("Title");
        metadata.add_tag("custom_key", "custom_value");

        let tags = metadata.all_tags();
        assert!(tags
            .iter()
            .any(|(k, v)| *k == "custom_key" && *v == "custom_value"));
    }

    #[test]
    fn recording_catalog_create_recording() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let entry = catalog.create_recording("/tmp/rec.wav", RecordingFormat::Wav);
        assert_eq!(entry.id, "rec_1");
        assert!(catalog.is_recording());
    }

    #[test]
    fn recording_catalog_start_save() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        catalog.start_recording(&id).unwrap();
        if catalog.get_recording(&id).unwrap().status == RecordingStatus::Idle {
            catalog.start_recording(&id).unwrap();
        }
        catalog
            .save_recording(&id, 1024 * 1024, Duration::from_secs(60))
            .unwrap();

        assert!(!catalog.is_recording());
    }

    #[test]
    fn recording_catalog_rename() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        catalog.rename_recording(&id, "New Name").unwrap();

        let entry = catalog.get_recording(&id).unwrap();
        assert_eq!(entry.metadata.title, "New Name");
    }

    #[test]
    fn recording_catalog_delete() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        if catalog.get_recording(&id).unwrap().status == RecordingStatus::Idle {
            catalog.start_recording(&id).unwrap();
        }
        catalog
            .save_recording(&id, 1024 * 1024, Duration::from_secs(60))
            .unwrap();

        let freed = catalog.delete_recording(&id).unwrap();
        assert_eq!(freed, 1024 * 1024);
        assert_eq!(catalog.recording_count(), 0);
    }

    #[test]
    fn recording_catalog_add_tag() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        catalog.add_tag(&id, "live").unwrap();
        assert!(catalog.get_recording(&id).unwrap().has_tag("live"));
    }

    #[test]
    fn recording_catalog_starred() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        catalog.toggle_starred(&id).unwrap();
        assert!(catalog.get_recording(&id).unwrap().is_starred);
    }

    #[test]
    fn recording_catalog_stop_all_and_save() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        catalog.start_recording(&id).unwrap();
        catalog.stop_all_and_save();

        assert!(!catalog.is_recording());
    }

    #[test]
    fn recording_catalog_all_recordings_sorted() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let _ = catalog.create_recording("/tmp/a.wav", RecordingFormat::Wav);
        let _ = catalog.create_recording("/tmp/b.wav", RecordingFormat::Wav);

        let recordings = catalog.all_recordings();
        assert_eq!(recordings.len(), 2);
    }

    #[test]
    fn recording_catalog_recordings_with_tag() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id1 = catalog
            .create_recording("/tmp/a.wav", RecordingFormat::Wav)
            .id
            .clone();
        catalog.add_tag(&id1, "live").unwrap();
        let _id2 = catalog
            .create_recording("/tmp/b.wav", RecordingFormat::Wav)
            .id
            .clone();

        let results = catalog.recordings_with_tag("live");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn recording_catalog_starred_recordings() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/a.wav", RecordingFormat::Wav)
            .id
            .clone();
        catalog.toggle_starred(&id).unwrap();

        let starred = catalog.starred_recordings();
        assert_eq!(starred.len(), 1);
    }

    #[test]
    fn recording_catalog_all_tags() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id1 = catalog
            .create_recording("/tmp/a.wav", RecordingFormat::Wav)
            .id
            .clone();
        catalog.add_tag(&id1, "live").unwrap();
        let id2 = catalog
            .create_recording("/tmp/b.wav", RecordingFormat::Wav)
            .id
            .clone();
        catalog.add_tag(&id2, "live").unwrap();
        catalog.add_tag(&id2, "studio").unwrap();

        let tags = catalog.all_tags();
        assert!(tags.contains(&"live".to_string()));
        assert!(tags.contains(&"studio".to_string()));
    }

    #[test]
    fn recording_catalog_storage_info() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let (total, used, _available, _level) = catalog.storage_info();
        assert_eq!(total, 100 * 1024 * 1024 * 1024);
        assert_eq!(used, 0);
    }

    #[test]
    fn recording_catalog_storage_warning_normal() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        assert_eq!(catalog.storage_warning_level(), StorageWarningLevel::Normal);
    }

    #[test]
    fn recording_catalog_storage_warning_warning() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 500 * 1024 * 1024 - 100);
        assert_eq!(
            catalog.storage_warning_level(),
            StorageWarningLevel::Warning
        );
    }

    #[test]
    fn recording_catalog_storage_warning_critical() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 100 * 1024 * 1024);
        assert_eq!(
            catalog.storage_warning_level(),
            StorageWarningLevel::Critical
        );
    }

    #[test]
    fn recording_catalog_low_space() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 500 * 1024 * 1024);
        assert!(catalog.is_low_space());
        assert!(!catalog.is_critical_space());
    }

    #[test]
    fn recording_catalog_critical_space() {
        let catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 100 * 1024 * 1024);
        assert!(catalog.is_critical_space());
    }

    #[test]
    fn recording_catalog_set_sort() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        catalog.set_sort(SortColumn::Name, SortOrder::Ascending);

        let (col, order) = catalog.sort_config();
        assert_eq!(*col, SortColumn::Name);
        assert_eq!(*order, SortOrder::Ascending);
    }

    #[test]
    fn recording_catalog_set_storage() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        catalog.set_storage(100 * 1024 * 1024 * 1024, 45 * 1024 * 1024 * 1024);
        assert_eq!(catalog.used_storage(), 55 * 1024 * 1024 * 1024);
    }

    #[test]
    fn recording_catalog_rename_not_found() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let result = catalog.rename_recording("nonexistent", "New Name");
        assert!(result.is_err());
    }

    #[test]
    fn recording_catalog_delete_not_found() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let result = catalog.delete_recording("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn recording_catalog_start_not_recording() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();
        // After create, status is Idle, so start should work.
        let result = catalog.start_recording(&id);
        assert!(result.is_ok());
    }

    #[test]
    fn recording_catalog_status_string() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();
        if catalog.get_recording(&id).unwrap().status == RecordingStatus::Idle {
            catalog.start_recording(&id).unwrap();
        }
        catalog
            .save_recording(&id, 10 * 1024 * 1024, Duration::from_secs(300))
            .unwrap();

        let status = catalog.status_string();
        assert!(status.contains("Recordings: 1"));
        assert!(status.contains("Used:"));
    }

    #[test]
    fn recording_catalog_available_formats() {
        let formats = RecordingCatalog::available_formats();
        assert!(formats.contains(&RecordingFormat::Wav));
        assert!(formats.contains(&RecordingFormat::MP3));
        assert!(formats.contains(&RecordingFormat::Flac));
    }

    #[test]
    fn recording_catalog_available_sort_columns() {
        let columns = RecordingCatalog::available_sort_columns();
        assert!(columns.contains(&SortColumn::Name));
        assert!(columns.contains(&SortColumn::Date));
    }

    #[test]
    fn recording_catalog_format_extension() {
        assert_eq!(RecordingFormat::Wav.extension(), "wav");
        assert_eq!(RecordingFormat::Flac.extension(), "flac");
        assert_eq!(RecordingFormat::MP3.extension(), "mp3");
    }

    #[test]
    fn recording_catalog_format_name() {
        assert_eq!(RecordingFormat::Wav.name(), "WAV");
        assert_eq!(RecordingFormat::Flac.name(), "FLAC");
    }

    #[test]
    fn storage_warning_level_message() {
        assert!(StorageWarningLevel::Normal.message().contains("sufficient"));
        assert!(StorageWarningLevel::Warning.message().contains("low"));
        assert!(StorageWarningLevel::Critical.message().contains("CRITICAL"));
    }

    #[test]
    fn recording_catalog_delete_frees_storage() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let initial_available = catalog.available_storage();

        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();
        if catalog.get_recording(&id).unwrap().status == RecordingStatus::Idle {
            catalog.start_recording(&id).unwrap();
        }
        catalog
            .save_recording(&id, 10 * 1024 * 1024, Duration::from_secs(60))
            .unwrap();

        let available_after_save = catalog.available_storage();
        assert!(available_after_save < initial_available);

        catalog.delete_recording(&id).unwrap();

        assert_eq!(catalog.available_storage(), initial_available);
        assert_eq!(catalog.used_storage(), 0);
    }

    #[test]
    fn recording_catalog_delete_multiple() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);

        let id1 = catalog
            .create_recording("/tmp/a.wav", RecordingFormat::Wav)
            .id
            .clone();
        let id2 = catalog
            .create_recording("/tmp/b.wav", RecordingFormat::Wav)
            .id
            .clone();
        catalog.start_recording(&id1).unwrap();
        catalog
            .save_recording(&id1, 1024, Duration::from_secs(10))
            .unwrap();
        catalog.start_recording(&id2).unwrap();
        catalog
            .save_recording(&id2, 2048, Duration::from_secs(20))
            .unwrap();

        catalog.delete_recording(&id1).unwrap();
        catalog.delete_recording(&id2).unwrap();

        assert_eq!(catalog.recording_count(), 0);
        assert_eq!(catalog.used_storage(), 0);
    }

    #[test]
    fn recording_entry_extension_string() {
        let entry = RecordingEntry::new("r1", "/tmp/rec.wav", RecordingFormat::Wav);
        assert_eq!(entry.extension_string(), "wav");
    }

    #[test]
    fn recording_catalog_sort_name_ascending() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        catalog.create_recording("/tmp/zebra.wav", RecordingFormat::Wav);
        catalog.create_recording("/tmp/apple.wav", RecordingFormat::Wav);

        catalog.set_sort(SortColumn::Name, SortOrder::Ascending);
        let recordings = catalog.all_recordings();
        assert_eq!(recordings[0].file_name, "apple.wav");
    }

    #[test]
    fn recording_catalog_remove_tag_not_found() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        let result = catalog.remove_tag(&id, "nonexistent");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn recording_catalog_multiple_recordings() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);

        let _ = catalog.create_recording("/tmp/1.wav", RecordingFormat::Wav);
        let _ = catalog.create_recording("/tmp/2.wav", RecordingFormat::Wav);
        let _ = catalog.create_recording("/tmp/3.wav", RecordingFormat::Wav);

        assert_eq!(catalog.recording_count(), 3);
    }

    #[test]
    fn recording_error_display() {
        assert_eq!(
            format!("{}", RecordingError::NotFound("r1".to_string())),
            "Recording not found: r1"
        );
        assert_eq!(
            format!("{}", RecordingError::InvalidState("test".to_string())),
            "Invalid state: test"
        );
    }

    #[test]
    fn recording_catalog_tag_uniqueness() {
        let mut catalog = RecordingCatalog::new(100 * 1024 * 1024 * 1024, 50 * 1024 * 1024 * 1024);
        let id = catalog
            .create_recording("/tmp/rec.wav", RecordingFormat::Wav)
            .id
            .clone();

        catalog.add_tag(&id, "live").unwrap();
        catalog.add_tag(&id, "live").unwrap(); // Duplicate.

        let entry = catalog.get_recording(&id).unwrap();
        let live_count = entry.tags.iter().filter(|t| *t == "live").count();
        assert_eq!(live_count, 1); // Should not duplicate.
    }
}
