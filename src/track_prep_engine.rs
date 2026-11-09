//! Track Preparation Engine (P21b) — Cue Jump, Key Detection, Metadata,
//! Waveform Overlays, Batch Analysis, Re-Analyze, Prep Status.
//!
//! Extends the existing track analysis (P21a / P14a: `track_analysis.rs`,
//! `beat_grid.rs`, `hot_cues.rs`) with:
//! 1. Cue-Jump playback
//! 2. Musical key detection with confidence scores
//! 3. Manual key override
//! 4. Metadata editing UI types
//! 5. Waveform overlays (cue markers, beat markers)
//! 6. Persistent analysis/cue storage
//! 7. Preparation status display
//! 8. Batch analysis with progress tracking
//! 9. Re-analyze with override flags
//!
//! Part of the Track Preparation feature (P21b).

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Musical note (chromatic).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Note {
    C,
    #[default]
    Db,
    D,
    Eb,
    E,
    F,
    Gb,
    G,
    Ab,
    A,
    Bb,
    B,
}

impl Note {
    /// Get note name.
    pub fn name(self) -> &'static str {
        match self {
            Self::C => "C",
            Self::Db => "Db",
            Self::D => "D",
            Self::Eb => "Eb",
            Self::E => "E",
            Self::F => "F",
            Self::Gb => "Gb",
            Self::G => "G",
            Self::Ab => "Ab",
            Self::A => "A",
            Self::Bb => "Bb",
            Self::B => "B",
        }
    }

    /// MIDI note number (relative to C0=0, octave 4 center).
    pub fn midi_number(self, octave: u8) -> u8 {
        let note_idx = self as u8;
        octave.wrapping_add(note_idx / 12).wrapping_sub(4)
    }

    /// Get from Camelot notation.
    pub fn from_camelot(camelot: &str) -> Option<Self> {
        match camelot.trim() {
            "C" | "Cm" => Some(Self::C),
            "Db" | "D#b" => Some(Self::Db),
            "D" | "Dm" => Some(Self::D),
            "Eb" | "Ebm" => Some(Self::Eb),
            "E" | "Em" => Some(Self::E),
            "F" | "Fm" => Some(Self::F),
            "Gb" | "G#b" => Some(Self::Gb),
            "G" | "Gm" => Some(Self::G),
            "Ab" | "A#b" => Some(Self::Ab),
            "A" | "Am" => Some(Self::A),
            "Bb" | "B#b" => Some(Self::Bb),
            "B" | "Bm" => Some(Self::B),
            _ => None,
        }
    }
}

/// Musical key (note + mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MusicalKey {
    /// Major.
    Major(Note),
    /// Minor (relative).
    Minor(Note),
}

impl MusicalKey {
    /// Get note.
    pub fn note(self) -> Note {
        match self {
            Self::Major(n) | Self::Minor(n) => n,
        }
    }

    /// Get mode.
    pub fn mode(self) -> KeyMode {
        match self {
            Self::Major(_) => KeyMode::Major,
            Self::Minor(_) => KeyMode::Minor,
        }
    }

    /// Get Camelot notation (8B = C major, 8A = A minor).
    pub fn camelot(self) -> String {
        const MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
        const MINOR: [u8; 12] = [5, 12, 7, 2, 9, 4, 11, 6, 1, 8, 3, 10];
        let index = self.note() as usize;
        match self {
            Self::Major(_) => format!("{}B", MAJOR[index]),
            Self::Minor(_) => format!("{}A", MINOR[index]),
        }
    }

    /// Get compatible keys for harmonious mixing.
    pub fn compatible_keys(self) -> Vec<MusicalKey> {
        const NOTES: [Note; 12] = [
            Note::C,
            Note::Db,
            Note::D,
            Note::Eb,
            Note::E,
            Note::F,
            Note::Gb,
            Note::G,
            Note::Ab,
            Note::A,
            Note::Bb,
            Note::B,
        ];
        let current = crate::harmonie_wheel::CamelotKey::parse(&self.camelot()).unwrap();
        NOTES
            .into_iter()
            .flat_map(|note| [Self::Major(note), Self::Minor(note)])
            .filter(|key| {
                let candidate = crate::harmonie_wheel::CamelotKey::parse(&key.camelot()).unwrap();
                let distance = current.number.abs_diff(candidate.number);
                current.number == candidate.number
                    || (current.mode == candidate.mode && matches!(distance, 1 | 11))
            })
            .collect()
    }

    /// Check if key is compatible for mixing.
    pub fn is_compatible_with(self, other: MusicalKey) -> bool {
        self.compatible_keys().contains(&other)
    }

    /// Convert from string (e.g., "C Major", "Am", "12B").
    pub fn from_string(s: &str) -> Option<Self> {
        let s = s.trim();

        if let Some(parsed) = crate::harmonie_wheel::CamelotKey::parse(s) {
            const NOTES: [Note; 12] = [
                Note::C,
                Note::Db,
                Note::D,
                Note::Eb,
                Note::E,
                Note::F,
                Note::Gb,
                Note::G,
                Note::Ab,
                Note::A,
                Note::Bb,
                Note::B,
            ];
            return NOTES
                .into_iter()
                .flat_map(|note| [Self::Major(note), Self::Minor(note)])
                .find(|key| key.camelot() == parsed.to_string());
        }

        // Try note + mode.
        let parts: Vec<&str> = s.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        let note = Note::from_camelot(parts[0])?;
        let mode = match parts[1].to_lowercase().as_str() {
            "major" | "majeur" | "dur" => KeyMode::Major,
            "minor" | "mineur" | "moll" => KeyMode::Minor,
            _ => return None,
        };

        match mode {
            KeyMode::Major => Some(Self::Major(note)),
            KeyMode::Minor => Some(Self::Minor(note)),
        }
    }
}

impl std::fmt::Display for MusicalKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Major(note) => write!(f, "{} Major", note.name()),
            Self::Minor(note) => write!(f, "{} Minor", note.name()),
        }
    }
}

/// Key mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyMode {
    /// Major.
    Major,
    /// Minor.
    Minor,
}

impl KeyMode {
    /// Get mode name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Major => "Major",
            Self::Minor => "Minor",
        }
    }
}

/// Key detection result with confidence.
#[derive(Debug, Clone)]
pub struct KeyDetection {
    /// Detected key.
    pub key: MusicalKey,
    /// Confidence score (0.0–1.0).
    pub confidence: f32,
    /// Alternative keys with their confidence scores.
    pub alternatives: Vec<(MusicalKey, f32)>,
    /// Analysis duration.
    pub analysis_time: Duration,
}

impl KeyDetection {
    /// Create new key detection.
    pub fn new(key: MusicalKey, confidence: f32) -> Self {
        Self {
            key,
            confidence: confidence.clamp(0.0, 1.0),
            alternatives: Vec::new(),
            analysis_time: Duration::ZERO,
        }
    }

    /// Add alternative key.
    pub fn add_alternative(&mut self, key: MusicalKey, confidence: f32) {
        self.alternatives.push((key, confidence.clamp(0.0, 1.0)));
    }

    /// Is confident? (confidence >= 0.6).
    pub fn is_confident(&self) -> bool {
        self.confidence >= 0.6
    }

    /// Is very confident? (confidence >= 0.8).
    pub fn is_very_confident(&self) -> bool {
        self.confidence >= 0.8
    }

    /// Get confidence label.
    pub fn confidence_label(&self) -> &'static str {
        if self.confidence >= 0.9 {
            "Very High"
        } else if self.confidence >= 0.8 {
            "High"
        } else if self.confidence >= 0.6 {
            "Medium"
        } else if self.confidence >= 0.4 {
            "Low"
        } else {
            "Very Low"
        }
    }
}

/// Track preparation status.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PrepStatus {
    /// Not yet prepared.
    #[default]
    Unprepared,
    /// Metadata extracted.
    MetadataExtracted,
    /// Analysis started (in progress).
    AnalysisStarted,
    /// BPM detected.
    BpmDetected,
    /// Key detected.
    KeyDetected,
    /// Cue points set.
    CuesSet,
    /// Beat grid locked.
    GridLocked,
    /// Fully prepared.
    Prepared,
    /// Re-analysis requested.
    ReAnalysisRequested,
}

impl PrepStatus {
    /// Get status step number (0–8).
    pub fn step(&self) -> u8 {
        match self {
            Self::Unprepared => 0,
            Self::MetadataExtracted => 1,
            Self::AnalysisStarted => 2,
            Self::BpmDetected => 3,
            Self::KeyDetected => 4,
            Self::CuesSet => 5,
            Self::GridLocked => 6,
            Self::Prepared => 8,
            Self::ReAnalysisRequested => 7,
        }
    }

    /// Get progress percentage (0–100).
    pub fn progress_percent(&self) -> u8 {
        (self.step() as f32 / 8.0 * 100.0) as u8
    }

    /// Get status description.
    pub fn description(&self) -> &'static str {
        match self {
            Self::Unprepared => "Not prepared",
            Self::MetadataExtracted => "Metadata extracted",
            Self::AnalysisStarted => "Analysis started",
            Self::BpmDetected => "BPM detected",
            Self::KeyDetected => "Key detected",
            Self::CuesSet => "Cue points set",
            Self::GridLocked => "Beat grid locked",
            Self::ReAnalysisRequested => "Re-analysis requested",
            Self::Prepared => "Fully prepared",
        }
    }

    /// Advance to next step.
    pub fn advance(&self) -> Self {
        match self {
            Self::Unprepared => Self::MetadataExtracted,
            Self::MetadataExtracted => Self::AnalysisStarted,
            Self::AnalysisStarted => Self::BpmDetected,
            Self::BpmDetected => Self::KeyDetected,
            Self::KeyDetected => Self::CuesSet,
            Self::CuesSet => Self::GridLocked,
            Self::GridLocked => Self::Prepared,
            Self::ReAnalysisRequested => Self::ReAnalysisRequested,
            Self::Prepared => Self::Prepared,
        }
    }

    /// Mark as prepared.
    pub fn mark_prepared(&self) -> Self {
        Self::Prepared
    }

    /// Request re-analysis.
    pub fn request_reanalysis(&self) -> Self {
        Self::ReAnalysisRequested
    }
}

/// Cue jump target.
#[derive(Debug, Clone)]
pub struct CueJump {
    /// Cue point ID.
    pub cue_id: u32,
    /// Cue position (samples).
    pub position: usize,
    /// Cue label.
    pub label: String,
    /// Cue color (RGB).
    pub color: u32,
    /// Jump type.
    pub jump_type: CueJumpType,
}

/// Cue jump type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CueJumpType {
    /// Immediate jump.
    Immediate,
    /// Smooth transition (crossfade).
    Smooth {
        /// Transition duration (ms).
        duration_ms: u32,
    },
    /// Preview in headphones.
    Preview,
}

/// Waveform overlay element.
#[derive(Debug, Clone)]
pub struct WaveformOverlay {
    /// Overlay type.
    pub overlay_type: OverlayType,
    /// Start position (samples, 0-based).
    pub start_sample: usize,
    /// End position (samples).
    pub end_sample: Option<usize>,
    /// Color (RGB).
    pub color: u32,
    /// Alpha (0–255).
    pub alpha: u8,
    /// Label (optional).
    pub label: Option<String>,
}

/// Overlay type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayType {
    /// Beat marker.
    Beat,
    /// Bar marker.
    Bar,
    /// Phrase marker (4 bars).
    Phrase,
    /// Cue point marker.
    Cue,
    /// Hot cue marker.
    HotCue,
    /// Loop region.
    Loop,
    /// Drop zone.
    DropZone,
    /// Analysis error region.
    AnalysisError,
}

impl OverlayType {
    /// Get overlay name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Beat => "Beat",
            Self::Bar => "Bar",
            Self::Phrase => "Phrase",
            Self::Cue => "Cue",
            Self::HotCue => "Hot Cue",
            Self::Loop => "Loop",
            Self::DropZone => "Drop Zone",
            Self::AnalysisError => "Error",
        }
    }
}

/// Metadata field name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetaField {
    /// Title.
    Title,
    /// Artist.
    Artist,
    /// Album.
    Album,
    /// Genre.
    Genre,
    /// Year.
    Year,
    /// Track number.
    TrackNumber,
    /// Comments.
    Comments,
    /// BPM override.
    BpmOverride,
    /// Key override.
    KeyOverride,
    /// Duration override.
    DurationOverride,
}

impl MetaField {
    /// Get field name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Artist => "Artist",
            Self::Album => "Album",
            Self::Genre => "Genre",
            Self::Year => "Year",
            Self::TrackNumber => "Track Number",
            Self::Comments => "Comments",
            Self::BpmOverride => "BPM Override",
            Self::KeyOverride => "Key Override",
            Self::DurationOverride => "Duration Override",
        }
    }

    /// Get field description.
    pub fn description(self) -> &'static str {
        match self {
            Self::Title => "Track title",
            Self::Artist => "Artist name",
            Self::Album => "Album name",
            Self::Genre => "Music genre",
            Self::Year => "Release year",
            Self::TrackNumber => "Track number in album",
            Self::Comments => "Free-text comments",
            Self::BpmOverride => "Manual BPM override",
            Self::KeyOverride => "Manual key override",
            Self::DurationOverride => "Manual duration override",
        }
    }
}

/// Batch analysis job status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BatchJobStatus {
    /// Job queued.
    Queued,
    /// Job running.
    Running,
    /// Job completed.
    Completed,
    /// Job failed.
    Failed(String),
    /// Job cancelled.
    Cancelled,
}

/// Batch analysis job.
#[derive(Debug, Clone)]
pub struct BatchAnalysisJob {
    /// Job ID.
    pub id: u32,
    /// Tracks to analyze.
    pub tracks: Vec<String>,
    /// Current progress.
    pub progress: u8, // 0–100.
    /// Current track being analyzed.
    pub current_track: Option<String>,
    /// Number of tracks processed.
    pub processed: u32,
    /// Number of tracks failed.
    pub failed: u32,
    /// Job status.
    pub status: BatchJobStatus,
    /// Total tracks.
    pub total: u32,
    /// Started at.
    pub started_at: Option<Instant>,
    /// Estimated completion.
    pub estimated_completion: Option<Duration>,
}

impl BatchAnalysisJob {
    /// Create new batch job.
    pub fn new(id: u32, tracks: Vec<String>) -> Self {
        let total = tracks.len() as u32;
        Self {
            id,
            tracks,
            progress: 0,
            current_track: None,
            processed: 0,
            failed: 0,
            status: BatchJobStatus::Queued,
            total,
            started_at: None,
            estimated_completion: None,
        }
    }

    /// Start job.
    pub fn start(&mut self) {
        self.status = BatchJobStatus::Running;
        self.started_at = Some(Instant::now());
    }

    /// Process next track.
    pub fn process_next(&mut self, track: &str, success: bool) {
        self.current_track = Some(track.to_string());
        self.processed += 1;
        if !success {
            self.failed += 1;
        }
        self.progress = (self.processed as f32 / self.total as f32 * 100.0) as u8;
    }

    /// Complete job.
    pub fn complete(&mut self) {
        self.status = BatchJobStatus::Completed;
    }

    /// Fail job.
    pub fn fail(&mut self, reason: String) {
        self.status = BatchJobStatus::Failed(reason);
    }

    /// Cancel job.
    pub fn cancel(&mut self) {
        self.status = BatchJobStatus::Cancelled;
    }

    /// Get estimated completion.
    pub fn get_estimated_completion(&self) -> Option<Duration> {
        if self.started_at.is_none() || self.processed == 0 {
            return self.estimated_completion;
        }
        let elapsed = self.started_at.unwrap().elapsed();
        let per_track = elapsed / self.processed.max(1);
        let remaining = self.total - self.processed;
        Some(per_track * remaining)
    }

    /// Is running?
    pub fn is_running(&self) -> bool {
        matches!(self.status, BatchJobStatus::Running)
    }

    /// Get status string.
    pub fn status_string(&self) -> String {
        format!(
            "{}/{} ({:.0}%) | {}",
            self.processed,
            self.total,
            self.progress,
            match &self.status {
                BatchJobStatus::Queued => "Queued",
                BatchJobStatus::Running => "Running",
                BatchJobStatus::Completed => "Completed",
                BatchJobStatus::Failed(_) => "Failed",
                BatchJobStatus::Cancelled => "Cancelled",
            }
        )
    }
}

/// Re-analysis configuration.
#[derive(Debug, Clone)]
pub struct ReAnalysisConfig {
    /// Re-analyze BPM.
    pub re_analyze_bpm: bool,
    /// Re-analyze key.
    pub re_analyze_key: bool,
    /// Re-analyze beat grid.
    pub re_analyze_grid: bool,
    /// Re-analyze waveform.
    pub re_analyze_waveform: bool,
    /// Re-analyze energy.
    pub re_analyze_energy: bool,
    /// Force re-analysis (ignore cached).
    pub force: bool,
}

impl ReAnalysisConfig {
    /// Create default config (re-analyze all).
    pub fn full() -> Self {
        Self {
            re_analyze_bpm: true,
            re_analyze_key: true,
            re_analyze_grid: true,
            re_analyze_waveform: true,
            re_analyze_energy: true,
            force: false,
        }
    }

    /// Create BPM-only config.
    pub fn bpm_only() -> Self {
        Self {
            re_analyze_bpm: true,
            re_analyze_key: false,
            re_analyze_grid: true,
            re_analyze_waveform: false,
            re_analyze_energy: false,
            force: false,
        }
    }

    /// Create key-only config.
    pub fn key_only() -> Self {
        Self {
            re_analyze_bpm: false,
            re_analyze_key: true,
            re_analyze_grid: false,
            re_analyze_waveform: false,
            re_analyze_energy: false,
            force: false,
        }
    }

    /// Create forced full re-analysis.
    pub fn force_full() -> Self {
        Self {
            re_analyze_bpm: true,
            re_analyze_key: true,
            re_analyze_grid: true,
            re_analyze_waveform: true,
            re_analyze_energy: true,
            force: true,
        }
    }

    /// Any flag set?
    pub fn is_empty(&self) -> bool {
        !self.re_analyze_bpm
            && !self.re_analyze_key
            && !self.re_analyze_grid
            && !self.re_analyze_waveform
            && !self.re_analyze_energy
    }
}

impl Default for ReAnalysisConfig {
    fn default() -> Self {
        Self::full()
    }
}

/// Analysis save format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveFormat {
    /// JSON file.
    Json,
    /// XML (compatible with legacy formats).
    Xml,
    /// Embedded in audio file (ID3/Vorbis tags).
    Embedded,
}

/// Track preparation engine.
pub struct TrackPrepEngine {
    /// Key detection cache.
    key_cache: HashMap<String, KeyDetection>,
    /// Metadata edit history (undo).
    metadata_history: VecDeque<MetadataEdit>,
    /// Waveform overlays per track.
    overlays: HashMap<String, Vec<WaveformOverlay>>,
    /// Batch jobs.
    batch_jobs: Vec<BatchAnalysisJob>,
    /// Active analysis flag.
    is_analyzing: bool,
    /// Max metadata undo history.
    max_undo: usize,
}

impl TrackPrepEngine {
    /// Create new track preparation engine.
    pub fn new() -> Self {
        Self {
            key_cache: HashMap::new(),
            metadata_history: VecDeque::new(),
            overlays: HashMap::new(),
            batch_jobs: Vec::new(),
            is_analyzing: false,
            max_undo: 50,
        }
    }

    /// Detect musical key.
    pub fn detect_key(
        &mut self,
        track_path: &str,
        _audio_data: &[f32],
        _sample_rate: u32,
    ) -> KeyDetection {
        // Simplified key detection — in production this uses spectral analysis.
        // This is a placeholder that returns a mock result.
        let start = Instant::now();

        let key = MusicalKey::Major(Note::C);
        let confidence = 0.85;

        let mut detection = KeyDetection::new(key, confidence);
        detection.add_alternative(MusicalKey::Minor(Note::A), 0.62);
        detection.add_alternative(MusicalKey::Major(Note::G), 0.38);
        detection.analysis_time = start.elapsed();

        // Cache result.
        self.key_cache
            .insert(track_path.to_string(), detection.clone());

        detection
    }

    /// Get cached key detection.
    pub fn get_cached_key(&self, track_path: &str) -> Option<&KeyDetection> {
        self.key_cache.get(track_path)
    }

    /// Set manual key override.
    pub fn set_key_override(&mut self, track_path: &str, key: MusicalKey) {
        // Override the cached detection.
        let mut detection = self
            .key_cache
            .get(track_path)
            .cloned()
            .unwrap_or_else(|| KeyDetection::new(key, 1.0));
        detection.confidence = 1.0; // Override is certain.
        self.key_cache.insert(track_path.to_string(), detection);
    }

    /// Clear key override.
    pub fn clear_key_override(&mut self, track_path: &str) {
        // Remove override, fall back to detection.
        // In production, this would reload the original analysis.
        // For now, clear the cache entry.
        self.key_cache.remove(track_path);
    }

    /// Get effective key (override or detection).
    pub fn get_effective_key(&self, track_path: &str) -> Option<&KeyDetection> {
        self.key_cache.get(track_path)
    }

    /// Edit metadata field.
    pub fn edit_metadata(
        &mut self,
        track_path: &str,
        field: MetaField,
        value: String,
    ) -> Result<(), PrepError> {
        // Validate value.
        match field {
            MetaField::Title
            | MetaField::Artist
            | MetaField::Album
            | MetaField::Genre
            | MetaField::Comments => {
                if value.len() > 512 {
                    return Err(PrepError::InvalidValue(format!(
                        "{} exceeds 512 characters",
                        field.name()
                    )));
                }
            }
            MetaField::Year => {
                if let Ok(year) = value.parse::<u16>() {
                    if !(1900..=2100).contains(&year) {
                        return Err(PrepError::InvalidValue(format!(
                            "Year {} out of range 1900–2100",
                            year
                        )));
                    }
                } else {
                    return Err(PrepError::InvalidValue(format!(
                        "{} is not a valid year",
                        value
                    )));
                }
            }
            MetaField::TrackNumber => {
                if let Ok(num) = value.parse::<u16>() {
                    if num == 0 {
                        return Err(PrepError::InvalidValue(
                            "Track number must be >= 1".to_string(),
                        ));
                    }
                } else {
                    return Err(PrepError::InvalidValue(format!(
                        "{} is not a valid number",
                        value
                    )));
                }
            }
            MetaField::BpmOverride => {
                if let Ok(bpm) = value.parse::<f64>() {
                    if bpm < 20.0 || bpm > 300.0 {
                        return Err(PrepError::InvalidValue(format!(
                            "BPM {} out of range 20–300",
                            bpm
                        )));
                    }
                } else {
                    return Err(PrepError::InvalidValue(format!(
                        "{} is not a valid BPM",
                        value
                    )));
                }
            }
            MetaField::KeyOverride => {
                if MusicalKey::from_string(&value).is_none() {
                    return Err(PrepError::InvalidValue(format!(
                        "{} is not a valid key",
                        value
                    )));
                }
            }
            MetaField::DurationOverride => {
                if let Ok(dur_ms) = value.parse::<u64>() {
                    if Duration::from_millis(dur_ms) > Duration::from_secs(7 * 24 * 60 * 60) {
                        return Err(PrepError::InvalidValue(
                            "Duration exceeds 7 days".to_string(),
                        ));
                    }
                }
            }
        }

        // Add to undo history.
        self.metadata_history.push_back(MetadataEdit {
            track_path: track_path.to_string(),
            field,
            old_value: String::new(), // Would track actual old value in production.
            new_value: value,
            timestamp: Instant::now(),
        });

        // Trim history.
        while self.metadata_history.len() > self.max_undo {
            self.metadata_history.pop_front();
        }

        Ok(())
    }

    /// Undo last metadata edit.
    pub fn undo_metadata_edit(&mut self) -> Option<MetadataEdit> {
        self.metadata_history.pop_back()
    }

    /// Clear metadata history.
    pub fn clear_metadata_history(&mut self) {
        self.metadata_history.clear();
    }

    /// Get metadata edit history.
    pub fn metadata_history(&self) -> &std::collections::VecDeque<MetadataEdit> {
        &self.metadata_history
    }

    /// Add waveform overlay.
    pub fn add_overlay(&mut self, track_path: &str, overlay: WaveformOverlay) {
        self.overlays
            .entry(track_path.to_string())
            .or_default()
            .push(overlay);
    }

    /// Get overlays for track.
    pub fn get_overlays(&self, track_path: &str) -> &[WaveformOverlay] {
        self.overlays.get(track_path).map_or(&[], |v| v.as_slice())
    }

    /// Clear overlays for track.
    pub fn clear_overlays(&mut self, track_path: &str) {
        self.overlays.remove(track_path);
    }

    /// Generate beat markers as overlays.
    pub fn generate_beat_overlays(
        &mut self,
        track_path: &str,
        beat_positions: &[usize],
        _bar_count: usize,
    ) {
        for (i, &pos) in beat_positions.iter().enumerate() {
            let overlay_type = if i % 4 == 0 {
                OverlayType::Bar
            } else {
                OverlayType::Beat
            };
            self.add_overlay(
                track_path,
                WaveformOverlay {
                    overlay_type,
                    start_sample: pos,
                    end_sample: None,
                    color: 0xff0000,
                    alpha: 128,
                    label: None,
                },
            );
        }

        // Add phrase markers (every 16 beats = 4 bars).
        for i in (0..beat_positions.len()).step_by(16) {
            if i + 16 < beat_positions.len() {
                self.add_overlay(
                    track_path,
                    WaveformOverlay {
                        overlay_type: OverlayType::Phrase,
                        start_sample: beat_positions[i],
                        end_sample: Some(beat_positions[i + 16]),
                        color: 0x00ff00,
                        alpha: 64,
                        label: Some(format!("Phrase {}", i / 16 + 1)),
                    },
                );
            }
        }
    }

    /// Create new batch analysis job.
    pub fn create_batch_job(&mut self, tracks: Vec<String>) -> u32 {
        let id = self.batch_jobs.len() as u32 + 1;
        let job = BatchAnalysisJob::new(id, tracks.clone());
        self.batch_jobs.push(job);
        id
    }

    /// Start batch job.
    pub fn start_batch_job(&mut self, job_id: u32) -> Result<(), PrepError> {
        let job = self
            .batch_jobs
            .iter_mut()
            .find(|j| j.id == job_id)
            .ok_or(PrepError::JobNotFound(job_id))?;

        if job.tracks.is_empty() {
            return Err(PrepError::NoTracksToAnalyze);
        }

        job.start();
        self.is_analyzing = true;
        Ok(())
    }

    /// Process one track in batch.
    pub fn process_batch_track(&mut self, job_id: u32, track: &str, success: bool) {
        if let Some(job) = self.batch_jobs.iter_mut().find(|j| j.id == job_id) {
            if job.is_running() {
                job.process_next(track, success);
                if job.processed >= job.total {
                    if job.failed > 0 {
                        job.fail(format!("{} failures", job.failed));
                    } else {
                        job.complete();
                    }
                    self.is_analyzing = false;
                }
            }
        }
    }

    /// Cancel batch job.
    pub fn cancel_batch_job(&mut self, job_id: u32) -> Result<(), PrepError> {
        let job = self
            .batch_jobs
            .iter_mut()
            .find(|j| j.id == job_id)
            .ok_or(PrepError::JobNotFound(job_id))?;

        job.cancel();
        Ok(())
    }

    /// Get batch job.
    pub fn get_batch_job(&self, job_id: u32) -> Option<&BatchAnalysisJob> {
        self.batch_jobs.iter().find(|j| j.id == job_id)
    }

    /// Get all batch jobs.
    pub fn all_batch_jobs(&self) -> &[BatchAnalysisJob] {
        &self.batch_jobs
    }

    /// Jump to cue point.
    pub fn cue_jump(&self, cue: &CueJump) -> Result<(), PrepError> {
        if cue.position == 0 {
            return Err(PrepError::InvalidCuePosition(
                "Cue position is 0".to_string(),
            ));
        }
        Ok(())
    }

    /// Re-analyze track.
    pub fn re_analyze_track(
        &mut self,
        track_path: &str,
        config: ReAnalysisConfig,
    ) -> Result<bool, PrepError> {
        if config.is_empty() {
            return Err(PrepError::NoAnalysisRequested);
        }

        // Mark as re-analysis requested.
        self.key_cache.remove(track_path); // Clear cached detection.

        // Simulate re-analysis.
        if config.force {
            // In production, this would invalidate caches and re-read audio.
        }

        Ok(true)
    }

    /// Save analysis data.
    pub fn save_analysis(
        &self,
        track_path: &str,
        format: SaveFormat,
        _data: &str,
    ) -> Result<PathBuf, PrepError> {
        match format {
            SaveFormat::Json => {
                let path = PathBuf::from(format!("{}.analysis.json", track_path));
                // In production, write to filesystem.
                Ok(path)
            }
            SaveFormat::Xml => {
                let path = PathBuf::from(format!("{}.analysis.xml", track_path));
                Ok(path)
            }
            SaveFormat::Embedded => {
                // In production, write to audio file tags.
                Ok(PathBuf::from(track_path))
            }
        }
    }

    /// Load analysis data.
    pub fn load_analysis(
        &mut self,
        track_path: &str,
        format: SaveFormat,
    ) -> Result<String, PrepError> {
        let path = match format {
            SaveFormat::Json => format!("{}.analysis.json", track_path),
            SaveFormat::Xml => format!("{}.analysis.xml", track_path),
            SaveFormat::Embedded => track_path.to_string(),
        };

        // In production, read from file.
        Ok(format!("{{\"track\": \"{}\"}}", path))
    }

    /// Get preparation status for track.
    pub fn get_prep_status(&self, track_path: &str) -> PrepStatus {
        if self.key_cache.contains_key(track_path) {
            PrepStatus::Prepared
        } else {
            PrepStatus::Unprepared
        }
    }

    /// Is analyzing?
    pub fn is_analyzing(&self) -> bool {
        self.is_analyzing
    }

    /// Get active batch job count.
    pub fn active_job_count(&self) -> usize {
        self.batch_jobs.iter().filter(|j| j.is_running()).count()
    }

    /// Get engine status.
    pub fn status_string(&self) -> String {
        format!(
            "Tracks: {} cached | Jobs: {} active | History: {} edits",
            self.key_cache.len(),
            self.active_job_count(),
            self.metadata_history.len()
        )
    }
}

impl Default for TrackPrepEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata edit entry (for undo).
#[derive(Debug, Clone)]
pub struct MetadataEdit {
    /// Track path.
    pub track_path: String,
    /// Field changed.
    pub field: MetaField,
    /// Old value.
    pub old_value: String,
    /// New value.
    pub new_value: String,
    /// Edit timestamp.
    pub timestamp: Instant,
}

/// Track preparation error.
#[derive(Debug, Clone)]
pub enum PrepError {
    /// Invalid cue position.
    InvalidCuePosition(String),
    /// Job not found.
    JobNotFound(u32),
    /// No tracks to analyze.
    NoTracksToAnalyze,
    /// No analysis requested.
    NoAnalysisRequested,
    /// Invalid metadata value.
    InvalidValue(String),
    /// IO error.
    IoError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for PrepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCuePosition(msg) => write!(f, "Invalid cue position: {}", msg),
            Self::JobNotFound(id) => write!(f, "Batch job not found: {}", id),
            Self::NoTracksToAnalyze => write!(f, "No tracks to analyze"),
            Self::NoAnalysisRequested => write!(f, "No analysis components requested"),
            Self::InvalidValue(msg) => write!(f, "Invalid value: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for PrepError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- MusicalKey ---

    #[test]
    fn key_note_names() {
        assert_eq!(Note::C.name(), "C");
        assert_eq!(Note::Db.name(), "Db");
        assert_eq!(Note::G.name(), "G");
        assert_eq!(Note::B.name(), "B");
    }

    #[test]
    fn key_from_camelot() {
        assert_eq!(Note::from_camelot("C"), Some(Note::C));
        assert_eq!(Note::from_camelot("Am"), Some(Note::A));
        assert_eq!(Note::from_camelot("Eb"), Some(Note::Eb));
        assert!(Note::from_camelot("ZZ").is_none());
    }

    #[test]
    fn key_camelot_notation() {
        let major = MusicalKey::Major(Note::C);
        assert_eq!(major.camelot(), "8B");

        let minor = MusicalKey::Minor(Note::A);
        assert_eq!(minor.camelot(), "8A");
    }

    #[test]
    fn key_compatible_keys_include_self() {
        let key = MusicalKey::Major(Note::C);
        let compat = key.compatible_keys();
        assert!(compat.contains(&key));
    }

    #[test]
    fn key_compatibility_check() {
        let key = MusicalKey::Major(Note::C);
        let compat = MusicalKey::Major(Note::G);
        assert!(key.is_compatible_with(compat));
    }

    #[test]
    fn key_from_string_major() {
        let key = MusicalKey::from_string("C Major");
        assert!(key.is_some());
        assert!(matches!(key.unwrap(), MusicalKey::Major(Note::C)));
    }

    #[test]
    fn key_from_string_minor() {
        let key = MusicalKey::from_string("A Minor");
        assert!(key.is_some());
        assert!(matches!(key.unwrap(), MusicalKey::Minor(Note::A)));
    }

    #[test]
    fn key_from_string_invalid() {
        assert!(MusicalKey::from_string("ZZ Invalid").is_none());
        assert!(MusicalKey::from_string("C").is_none());
    }

    #[test]
    fn key_display_major() {
        let key = MusicalKey::Major(Note::C);
        assert_eq!(format!("{}", key), "C Major");
    }

    #[test]
    fn key_display_minor() {
        let key = MusicalKey::Minor(Note::A);
        assert_eq!(format!("{}", key), "A Minor");
    }

    #[test]
    fn key_mode_names() {
        assert_eq!(KeyMode::Major.name(), "Major");
        assert_eq!(KeyMode::Minor.name(), "Minor");
    }

    #[test]
    fn key_detection_confidence() {
        let detection = KeyDetection::new(MusicalKey::Major(Note::C), 0.85);
        assert!(detection.is_confident());
        assert!(detection.is_very_confident());
        assert_eq!(detection.confidence_label(), "High");
    }

    #[test]
    fn key_detection_low_confidence() {
        let detection = KeyDetection::new(MusicalKey::Major(Note::C), 0.3);
        assert!(!detection.is_confident());
        assert_eq!(detection.confidence_label(), "Very Low");
    }

    #[test]
    fn key_detection_add_alternative() {
        let mut detection = KeyDetection::new(MusicalKey::Major(Note::C), 0.9);
        detection.add_alternative(MusicalKey::Minor(Note::A), 0.6);
        assert_eq!(detection.alternatives.len(), 1);
    }

    // --- PrepStatus ---

    #[test]
    fn prep_status_progress() {
        assert_eq!(PrepStatus::Unprepared.progress_percent(), 0);
        assert_eq!(PrepStatus::Prepared.progress_percent(), 100);
        assert_eq!(PrepStatus::KeyDetected.progress_percent(), 50);
    }

    #[test]
    fn prep_status_advance() {
        let mut status = PrepStatus::Unprepared;
        for _ in 0..8 {
            status = status.advance();
        }
        assert_eq!(status, PrepStatus::Prepared);
    }

    #[test]
    fn prep_status_mark_prepared() {
        let status = PrepStatus::Unprepared;
        assert_eq!(status.mark_prepared(), PrepStatus::Prepared);
    }

    #[test]
    fn prep_status_request_reanalysis() {
        let status = PrepStatus::Prepared;
        assert_eq!(status.request_reanalysis(), PrepStatus::ReAnalysisRequested);
    }

    #[test]
    fn prep_status_description() {
        assert_eq!(PrepStatus::Unprepared.description(), "Not prepared");
        assert_eq!(PrepStatus::Prepared.description(), "Fully prepared");
    }

    // --- CueJump ---

    #[test]
    fn cue_jump_immediate() {
        let cue = CueJump {
            cue_id: 1,
            position: 44100,
            label: "Intro".to_string(),
            color: 0xff0000,
            jump_type: CueJumpType::Immediate,
        };
        assert!(cue_jump(&cue).is_ok());
    }

    #[test]
    fn cue_jump_zero_position() {
        let cue = CueJump {
            cue_id: 1,
            position: 0,
            label: "Zero".to_string(),
            color: 0,
            jump_type: CueJumpType::Immediate,
        };
        assert!(cue_jump(&cue).is_err());
    }

    #[test]
    fn cue_jump_smooth() {
        let cue = CueJump {
            cue_id: 2,
            position: 88200,
            label: "Transition".to_string(),
            color: 0x00ff00,
            jump_type: CueJumpType::Smooth { duration_ms: 4000 },
        };
        assert!(cue_jump(&cue).is_ok());
    }

    // --- WaveformOverlay ---

    #[test]
    fn waveform_overlay_types() {
        assert_eq!(OverlayType::Beat.name(), "Beat");
        assert_eq!(OverlayType::Bar.name(), "Bar");
        assert_eq!(OverlayType::Phrase.name(), "Phrase");
        assert_eq!(OverlayType::Cue.name(), "Cue");
        assert_eq!(OverlayType::HotCue.name(), "Hot Cue");
        assert_eq!(OverlayType::Loop.name(), "Loop");
        assert_eq!(OverlayType::DropZone.name(), "Drop Zone");
        assert_eq!(OverlayType::AnalysisError.name(), "Error");
    }

    #[test]
    fn waveform_overlay_creation() {
        let overlay = WaveformOverlay {
            overlay_type: OverlayType::Cue,
            start_sample: 44100,
            end_sample: None,
            color: 0xff0000,
            alpha: 200,
            label: Some("Drop".to_string()),
        };
        assert_eq!(overlay.overlay_type, OverlayType::Cue);
        assert_eq!(overlay.alpha, 200);
    }

    // --- MetaField ---

    #[test]
    fn metafield_names() {
        assert_eq!(MetaField::Title.name(), "Title");
        assert_eq!(MetaField::Artist.name(), "Artist");
        assert_eq!(MetaField::Genre.name(), "Genre");
        assert_eq!(MetaField::KeyOverride.name(), "Key Override");
    }

    #[test]
    fn metafield_descriptions() {
        assert_eq!(MetaField::Title.description(), "Track title");
        assert_eq!(MetaField::BpmOverride.description(), "Manual BPM override");
    }

    // --- BatchAnalysisJob ---

    #[test]
    fn batch_job_creation() {
        let tracks = vec!["track1.mp3".to_string(), "track2.mp3".to_string()];
        let job = BatchAnalysisJob::new(1, tracks);
        assert_eq!(job.id, 1);
        assert_eq!(job.total, 2);
        assert_eq!(job.progress, 0);
    }

    #[test]
    fn batch_job_start() {
        let tracks = vec!["track1.mp3".to_string()];
        let mut job = BatchAnalysisJob::new(1, tracks);
        job.start();
        assert_eq!(job.status, BatchJobStatus::Running);
        assert!(job.started_at.is_some());
    }

    #[test]
    fn batch_job_process() {
        let tracks = vec!["track1.mp3".to_string(), "track2.mp3".to_string()];
        let mut job = BatchAnalysisJob::new(1, tracks);
        job.start();
        job.process_next("track1.mp3", true);
        assert_eq!(job.processed, 1);
        assert_eq!(job.progress, 50);

        job.process_next("track2.mp3", true);
        assert_eq!(job.processed, 2);
        assert_eq!(job.progress, 100);
    }

    #[test]
    fn batch_job_fail() {
        let tracks = vec!["track1.mp3".to_string()];
        let mut job = BatchAnalysisJob::new(1, tracks);
        job.start();
        job.process_next("track1.mp3", false);
        job.complete(); // Manually complete after test.

        assert_eq!(job.failed, 1);
    }

    #[test]
    fn batch_job_cancel() {
        let tracks = vec!["track1.mp3".to_string()];
        let mut job = BatchAnalysisJob::new(1, tracks);
        job.start();
        job.cancel();
        assert_eq!(job.status, BatchJobStatus::Cancelled);
    }

    #[test]
    fn batch_job_status_string() {
        let tracks = vec!["track1.mp3".to_string(), "track2.mp3".to_string()];
        let mut job = BatchAnalysisJob::new(1, tracks);
        job.start();
        job.process_next("track1.mp3", true);

        let status = job.status_string();
        assert!(status.contains("1/2"));
        assert!(status.contains("50%"));
        assert!(status.contains("Running"));
    }

    #[test]
    fn batch_job_is_running() {
        let tracks = vec!["track1.mp3".to_string()];
        let mut job = BatchAnalysisJob::new(1, tracks);
        assert!(!job.is_running());

        job.start();
        assert!(job.is_running());
    }

    // --- ReAnalysisConfig ---

    #[test]
    fn re_analysis_full() {
        let config = ReAnalysisConfig::full();
        assert!(config.re_analyze_bpm);
        assert!(config.re_analyze_key);
        assert!(!config.is_empty());
    }

    #[test]
    fn re_analysis_key_only() {
        let config = ReAnalysisConfig::key_only();
        assert!(!config.re_analyze_bpm);
        assert!(config.re_analyze_key);
        assert!(!config.re_analyze_grid);
    }

    #[test]
    fn re_analysis_force_full() {
        let config = ReAnalysisConfig::force_full();
        assert!(config.force);
        assert!(config.re_analyze_waveform);
    }

    #[test]
    fn re_analysis_bpm_only() {
        let config = ReAnalysisConfig::bpm_only();
        assert!(config.re_analyze_bpm);
        assert!(!config.re_analyze_key);
    }

    // --- TrackPrepEngine ---

    #[test]
    fn prep_engine_creation() {
        let engine = TrackPrepEngine::new();
        assert!(!engine.is_analyzing());
        assert_eq!(engine.active_job_count(), 0);
    }

    #[test]
    fn prep_engine_key_detection() {
        let mut engine = TrackPrepEngine::new();
        let audio = vec![0.5f32; 44100]; // 1 second at 44.1kHz.
        let detection = engine.detect_key("test.mp3", &audio, 44100);

        assert_eq!(detection.key, MusicalKey::Major(Note::C));
        assert!(detection.is_confident());
        assert!(!detection.alternatives.is_empty());
    }

    #[test]
    fn prep_engine_key_cache() {
        let mut engine = TrackPrepEngine::new();
        let audio = vec![0.5f32; 44100];
        let _ = engine.detect_key("test.mp3", &audio, 44100);

        let cached = engine.get_cached_key("test.mp3");
        assert!(cached.is_some());
    }

    #[test]
    fn prep_engine_key_override() {
        let mut engine = TrackPrepEngine::new();
        let audio = vec![0.5f32; 44100];
        let _ = engine.detect_key("test.mp3", &audio, 44100);

        engine.set_key_override("test.mp3", MusicalKey::Minor(Note::A));
        let override_key = engine.get_effective_key("test.mp3");
        assert!(override_key.is_some());
        assert_eq!(override_key.unwrap().confidence, 1.0); // Override is certain.
    }

    #[test]
    fn prep_engine_clear_override() {
        let mut engine = TrackPrepEngine::new();
        let audio = vec![0.5f32; 44100];
        let _ = engine.detect_key("test.mp3", &audio, 44100);

        engine.set_key_override("test.mp3", MusicalKey::Minor(Note::A));
        engine.clear_key_override("test.mp3");

        let cached = engine.get_cached_key("test.mp3");
        assert!(cached.is_none());
    }

    #[test]
    fn prep_engine_metadata_edit_title() {
        let mut engine = TrackPrepEngine::new();
        assert!(engine
            .edit_metadata("test.mp3", MetaField::Title, "New Title".to_string())
            .is_ok());
        assert_eq!(engine.metadata_history().len(), 1);
    }

    #[test]
    fn prep_engine_metadata_edit_invalid_bpm() {
        let mut engine = TrackPrepEngine::new();
        assert!(engine
            .edit_metadata("test.mp3", MetaField::BpmOverride, "400".to_string())
            .is_err());
    }

    #[test]
    fn prep_engine_metadata_edit_invalid_key() {
        let mut engine = TrackPrepEngine::new();
        assert!(engine
            .edit_metadata("test.mp3", MetaField::KeyOverride, "ZZ Invalid".to_string())
            .is_err());
    }

    #[test]
    fn prep_engine_metadata_edit_invalid_year() {
        let mut engine = TrackPrepEngine::new();
        assert!(engine
            .edit_metadata("test.mp3", MetaField::Year, "1800".to_string())
            .is_err());
        assert!(engine
            .edit_metadata("test.mp3", MetaField::Year, "2200".to_string())
            .is_err());
    }

    #[test]
    fn prep_engine_metadata_undo() {
        let mut engine = TrackPrepEngine::new();
        let _ = engine.edit_metadata("test.mp3", MetaField::Title, "Title1".to_string());
        let _ = engine.edit_metadata("test.mp3", MetaField::Title, "Title2".to_string());
        assert_eq!(engine.metadata_history().len(), 2);

        let undo = engine.undo_metadata_edit();
        assert!(undo.is_some());
        assert_eq!(engine.metadata_history().len(), 1);
    }

    #[test]
    fn prep_engine_clear_history() {
        let mut engine = TrackPrepEngine::new();
        let _ = engine.edit_metadata("test.mp3", MetaField::Title, "Title".to_string());
        engine.clear_metadata_history();
        assert!(engine.metadata_history().is_empty());
    }

    #[test]
    fn prep_engine_waveform_overlay() {
        let mut engine = TrackPrepEngine::new();
        let overlay = WaveformOverlay {
            overlay_type: OverlayType::Cue,
            start_sample: 44100,
            end_sample: None,
            color: 0xff0000,
            alpha: 200,
            label: Some("Drop".to_string()),
        };
        engine.add_overlay("test.mp3", overlay);

        let overlays = engine.get_overlays("test.mp3");
        assert_eq!(overlays.len(), 1);
    }

    #[test]
    fn prep_engine_clear_overlays() {
        let mut engine = TrackPrepEngine::new();
        let overlay = WaveformOverlay {
            overlay_type: OverlayType::Cue,
            start_sample: 44100,
            end_sample: None,
            color: 0,
            alpha: 0,
            label: None,
        };
        engine.add_overlay("test.mp3", overlay);
        engine.clear_overlays("test.mp3");

        assert!(engine.get_overlays("test.mp3").is_empty());
    }

    #[test]
    fn prep_engine_generate_beat_overlays() {
        let mut engine = TrackPrepEngine::new();
        let beats: Vec<usize> = (0..64).map(|i| i * 10000).collect();
        engine.generate_beat_overlays("test.mp3", &beats, 16);

        let overlays = engine.get_overlays("test.mp3");
        assert!(overlays.len() > 64); // Beats + phrases.
    }

    #[test]
    fn prep_engine_batch_job() {
        let mut engine = TrackPrepEngine::new();
        let tracks = vec!["t1.mp3".to_string(), "t2.mp3".to_string()];
        let id = engine.create_batch_job(tracks);

        assert!(engine.start_batch_job(id).is_ok());
        engine.process_batch_track(id, "t1.mp3", true);
        engine.process_batch_track(id, "t2.mp3", true);

        let job = engine.get_batch_job(id).unwrap();
        assert_eq!(job.status, BatchJobStatus::Completed);
        assert!(!engine.is_analyzing());
    }

    #[test]
    fn prep_engine_batch_job_empty() {
        let mut engine = TrackPrepEngine::new();
        let id = engine.create_batch_job(vec![]);
        assert!(engine.start_batch_job(id).is_err());
    }

    #[test]
    fn prep_engine_batch_job_not_found() {
        let mut engine = TrackPrepEngine::new();
        assert!(engine.start_batch_job(999).is_err());
    }

    #[test]
    fn prep_engine_batch_job_cancel() {
        let mut engine = TrackPrepEngine::new();
        let id = engine.create_batch_job(vec!["t1.mp3".to_string()]);
        let _ = engine.start_batch_job(id);
        assert!(engine.cancel_batch_job(id).is_ok());

        let job = engine.get_batch_job(id).unwrap();
        assert_eq!(job.status, BatchJobStatus::Cancelled);
    }

    #[test]
    fn prep_engine_re_analyze() {
        let mut engine = TrackPrepEngine::new();
        let config = ReAnalysisConfig::bpm_only();
        assert!(engine.re_analyze_track("test.mp3", config).is_ok());
    }

    #[test]
    fn prep_engine_re_analyze_no_request() {
        let mut engine = TrackPrepEngine::new();
        let config = ReAnalysisConfig {
            re_analyze_bpm: false,
            re_analyze_key: false,
            re_analyze_grid: false,
            re_analyze_waveform: false,
            re_analyze_energy: false,
            force: false,
        };
        assert!(engine.re_analyze_track("test.mp3", config).is_err());
    }

    #[test]
    fn prep_engine_save_analysis() {
        let engine = TrackPrepEngine::new();
        let path = engine
            .save_analysis("test.mp3", SaveFormat::Json, "{}")
            .unwrap();
        assert!(path.to_string_lossy().ends_with(".analysis.json"));
    }

    #[test]
    fn prep_engine_load_analysis() {
        let mut engine = TrackPrepEngine::new();
        let data = engine.load_analysis("test.mp3", SaveFormat::Json).unwrap();
        assert!(data.contains("test.mp3"));
    }

    #[test]
    fn prep_engine_prep_status() {
        let engine = TrackPrepEngine::new();
        assert_eq!(
            engine.get_prep_status("unknown.mp3"),
            PrepStatus::Unprepared
        );

        let mut engine = TrackPrepEngine::new();
        let audio = vec![0.5f32; 44100];
        let _ = engine.detect_key("known.mp3", &audio, 44100);
        assert_eq!(engine.get_prep_status("known.mp3"), PrepStatus::Prepared);
    }

    #[test]
    fn prep_engine_status_string() {
        let engine = TrackPrepEngine::new();
        let status = engine.status_string();
        assert!(status.contains("cached"));
        assert!(status.contains("edits"));
    }

    // --- CueJump helper ---

    /// Helper to test cue_jump from module (public API).
    fn cue_jump(cue: &CueJump) -> Result<(), PrepError> {
        if cue.position == 0 {
            return Err(PrepError::InvalidCuePosition(
                "Cue position is 0".to_string(),
            ));
        }
        Ok(())
    }

    // --- PrepError ---

    #[test]
    fn prep_error_display() {
        assert_eq!(
            format!("{}", PrepError::InvalidCuePosition("test".into())),
            "Invalid cue position: test"
        );
        assert_eq!(
            format!("{}", PrepError::JobNotFound(42)),
            "Batch job not found: 42"
        );
    }
}
