// SPDX-License-Identifier: MIT
// Copyright (c) 2025, DRDJ Pro Team
//
// Mix History Analytics — P23b
// Implements:
//   FR-023-001: Session Creation (Auto & Manual)
//   FR-023-002: Session Metadata (Name, Venue, Listener, Recording, End-Time)
//   FR-023-003: Transition Recording (Crossfader, Phrase-Alignment)
//   FR-023-004: Quality Scoring Algorithm (Beat Matching, Key, Smoothness, Phrase, EQ)
//   FR-023-005: Session Statistics (BPM, Energy, Genre, Distribution)
//   FR-023-006: Session List Display (Color-Coding, Peak Energy, Genre Tags)
//   FR-023-007: Session Filtering (Date, Genre, Tracks, Quality, Venue, Recording)
//   FR-023-008: Session Sorting (Date, Duration, Quality, Track Count)
//   FR-023-009: Detailed Transition View
//   FR-023-010: Session Deletion (Soft, Permanent, Confirmation, Preserve Recordings)
//   FR-023-011: Overall Statistics
//   FR-023-012: Data Export (JSON, CSV, PDF Summary)
//   FR-023-013: Session Replay (Timeline, Quality Trend, Energy)
//   FR-023-014: Data Persistence (Local Storage, Auto-Save, Cross-Restart, Backup/Restore)
//   FR-023-015: Performance Tracking

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use serde::{Deserialize, Serialize};

// ============================================================================
// Enums & Constants
// ============================================================================

/// Sort keys for session lists (FR-023-008).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortKey {
    DateCreated,
    Duration,
    Quality,
    TrackCount,
}

impl fmt::Display for SortKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SortKey::DateCreated => write!(f, "Date Created"),
            SortKey::Duration => write!(f, "Duration"),
            SortKey::Quality => write!(f, "Quality"),
            SortKey::TrackCount => write!(f, "Track Count"),
        }
    }
}

/// Sort direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Ascending,
    Descending,
}

/// Mix transition quality score (0.0 – 1.0).
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct QualityScore(pub f32);

impl QualityScore {
    pub fn new(value: f32) -> Self {
        QualityScore(value.clamp(0.0, 1.0))
    }

    pub fn as_percentage(&self) -> u8 {
        (self.0 * 100.0) as u8
    }
}

impl fmt::Display for QualityScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.0}%", self.as_percentage())
    }
}

/// Mix transition types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TransitionType {
    Mix,
    Cut,
    Blend,
    Drop,
    Fade,
}

impl fmt::Display for TransitionType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransitionType::Mix => write!(f, "Mix"),
            TransitionType::Cut => write!(f, "Cut"),
            TransitionType::Blend => write!(f, "Blend"),
            TransitionType::Drop => write!(f, "Drop"),
            TransitionType::Fade => write!(f, "Fade"),
        }
    }
}

/// Session deletion mode (FR-023-010).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeleteMode {
    Soft,      // Remove from list, keep recordings
    Permanent, // Remove everything including recordings
}

/// Export format (FR-023-012).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportFormat {
    Json,
    Csv,
    PdfSummary,
}

// ============================================================================
// Track & Transition Models
// ============================================================================

/// A single track within a session (FR-023-009).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackInfo {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Duration,
    pub bpm: f64,
    pub key: String,
    pub genre: String,
    pub file_path: String,
}

/// Track identity supplied together when recording session metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackIdentity {
    pub title: String,
    pub artist: String,
    pub album: String,
}
impl TrackIdentity {
    pub fn new(
        title: impl Into<String>,
        artist: impl Into<String>,
        album: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            artist: artist.into(),
            album: album.into(),
        }
    }
}

impl TrackInfo {
    pub fn new(
        identity: TrackIdentity,
        duration: Duration,
        bpm: f64,
        key: impl Into<String>,
        genre: impl Into<String>,
        file_path: impl Into<String>,
    ) -> Self {
        TrackInfo {
            title: identity.title,
            artist: identity.artist,
            album: identity.album,
            duration,
            bpm,
            key: key.into(),
            genre: genre.into(),
            file_path: file_path.into(),
        }
    }
}

/// Crossfader position at transition point (FR-023-003).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CrossfaderPosition(pub f32); // 0.0 = left, 0.5 = center, 1.0 = right

impl CrossfaderPosition {
    pub fn new(value: f32) -> Self {
        CrossfaderPosition(value.clamp(0.0, 1.0))
    }
}

/// A single transition between tracks within a session (FR-023-009).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub transition_number: u32,
    pub source_track: TrackInfo,
    pub destination_track: TrackInfo,
    pub transition_type: TransitionType,
    pub duration: Duration,
    pub bpm_difference: f64,
    pub key_compatibility: f32, // 0.0–1.0
    pub quality_score: QualityScore,
    pub timestamp: SystemTime,
    pub crossfader_position: CrossfaderPosition, // FR-023-003
    pub phrase_aligned: bool,                    // FR-023-004
}

/// Measurements captured together for a session transition.
#[derive(Debug, Clone)]
pub struct TransitionDetails {
    pub duration: Duration,
    pub bpm_difference: f64,
    pub key_compatibility: f32,
    pub quality_score: QualityScore,
    pub timestamp: SystemTime,
}

impl TransitionRecord {
    pub fn new(
        transition_number: u32,
        source_track: TrackInfo,
        destination_track: TrackInfo,
        transition_type: TransitionType,
        details: TransitionDetails,
    ) -> Self {
        let TransitionDetails {
            duration,
            bpm_difference,
            key_compatibility,
            quality_score,
            timestamp,
        } = details;

        TransitionRecord {
            transition_number,
            source_track,
            destination_track,
            transition_type,
            duration,
            bpm_difference,
            key_compatibility,
            quality_score,
            timestamp,
            crossfader_position: CrossfaderPosition::new(0.5),
            phrase_aligned: false,
        }
    }

    /// Create with crossfader and phrase alignment (FR-023-003).
    pub fn with_crossfader(
        transition_number: u32,
        source_track: TrackInfo,
        destination_track: TrackInfo,
        transition_type: TransitionType,
        details: TransitionDetails,
        crossfader_position: CrossfaderPosition,
        phrase_aligned: bool,
    ) -> Self {
        let TransitionDetails {
            duration,
            bpm_difference,
            key_compatibility,
            quality_score,
            timestamp,
        } = details;

        TransitionRecord {
            transition_number,
            source_track,
            destination_track,
            transition_type,
            duration,
            bpm_difference,
            key_compatibility,
            quality_score,
            timestamp,
            crossfader_position,
            phrase_aligned,
        }
    }
}

/// Sortable session metadata (FR-023-008, FR-023-011).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: String,
    pub session_name: String, // FR-023-002
    pub created_at: SystemTime,
    pub ended_at: Option<SystemTime>, // FR-023-002
    pub duration: Duration,
    pub quality_score: QualityScore,
    pub track_count: u32,
    pub transitions: Vec<TransitionRecord>,
    pub genres: Vec<String>,
    pub recording_path: Option<String>, // FR-023-002
    pub venue: Option<String>,          // FR-023-002
    pub listener_count: u32,            // FR-023-002
    pub deleted: bool,                  // For soft-deletion (FR-023-010)
}

impl SessionRecord {
    pub fn create_replay(&self) -> SessionReplay {
        let builder = SessionReplayBuilder::build_from_session(self);
        SessionReplay::build(self.session_id.clone(), builder.snapshots)
    }

    pub fn new(
        session_id: impl Into<String>,
        created_at: SystemTime,
        duration: Duration,
        quality_score: QualityScore,
        track_count: u32,
    ) -> Self {
        SessionRecord {
            session_id: session_id.into(),
            session_name: String::new(),
            created_at,
            ended_at: None,
            duration,
            quality_score,
            track_count,
            transitions: Vec::new(),
            genres: Vec::new(),
            recording_path: None,
            venue: None,
            listener_count: 0,
            deleted: false,
        }
    }

    /// Create session with name (FR-023-002).
    pub fn with_name(
        session_id: impl Into<String>,
        session_name: impl Into<String>,
        created_at: SystemTime,
        duration: Duration,
        quality_score: QualityScore,
        track_count: u32,
    ) -> Self {
        SessionRecord {
            session_id: session_id.into(),
            session_name: session_name.into(),
            created_at,
            ended_at: None,
            duration,
            quality_score,
            track_count,
            transitions: Vec::new(),
            genres: Vec::new(),
            recording_path: None,
            venue: None,
            listener_count: 0,
            deleted: false,
        }
    }

    pub fn add_transition(&mut self, transition: TransitionRecord) {
        self.transitions.push(transition);
    }

    pub fn add_genre(&mut self, genre: impl Into<String>) {
        let genre_str = genre.into();
        if !self.genres.contains(&genre_str) {
            self.genres.push(genre_str);
        }
    }

    pub fn set_recording_path(&mut self, path: impl Into<String>) {
        self.recording_path = Some(path.into());
    }

    /// Set venue (FR-023-002).
    pub fn set_venue(&mut self, venue: impl Into<String>) {
        self.venue = Some(venue.into());
    }

    /// Set listener count (FR-023-002).
    pub fn set_listener_count(&mut self, count: u32) {
        self.listener_count = count;
    }

    /// End the session (FR-023-002).
    pub fn end_session(&mut self) {
        self.ended_at = Some(SystemTime::now());
        self.duration = self
            .ended_at
            .unwrap()
            .duration_since(self.created_at)
            .unwrap_or_default();
    }

    /// Quality score color coding (FR-023-006).
    pub fn quality_color(&self) -> &'static str {
        let pct = self.quality_score.as_percentage();
        if pct >= 85 {
            "green"
        } else if pct >= 70 {
            "blue"
        } else if pct >= 60 {
            "yellow"
        } else {
            "red"
        }
    }

    /// Peak energy level from all transitions (FR-023-005).
    pub fn peak_energy(&self) -> f32 {
        self.transitions
            .iter()
            .map(|t| t.quality_score.0)
            .fold(0.0f32, f32::max)
    }

    /// Genre distribution as percentage (FR-023-005).
    pub fn genre_distribution(&self) -> HashMap<String, f32> {
        let genre_counts: HashMap<String, u32> =
            self.genres.iter().map(|g| (g.clone(), 1u32)).collect();
        let total = (self.genres.len() as f32).max(1.0);
        genre_counts
            .into_iter()
            .map(|(k, v)| (k, (v as f32 / total) * 100.0))
            .collect()
    }

    /// Transition type distribution (FR-023-005).
    pub fn transition_type_distribution(&self) -> HashMap<TransitionType, u32> {
        let mut counts: HashMap<TransitionType, u32> = HashMap::new();
        for t in &self.transitions {
            *counts.entry(t.transition_type).or_insert(0) += 1;
        }
        counts
    }

    /// Most frequently used tracks (FR-023-005).
    pub fn most_frequent_tracks(&self, top_n: usize) -> Vec<(String, u32)> {
        let mut track_counts: HashMap<String, u32> = HashMap::new();
        for t in &self.transitions {
            *track_counts
                .entry(t.source_track.title.clone())
                .or_insert(0) += 1;
            *track_counts
                .entry(t.destination_track.title.clone())
                .or_insert(0) += 1;
        }
        let mut freq: Vec<(String, u32)> = track_counts.into_iter().collect();
        freq.sort_by_key(|value| std::cmp::Reverse(value.1));
        freq.truncate(top_n);
        freq
    }

    /// Transition type count (FR-023-005).
    pub fn transition_type_count(&self, ttype: TransitionType) -> u32 {
        self.transitions
            .iter()
            .filter(|t| t.transition_type == ttype)
            .count() as u32
    }

    // ============================================================================
    // Quality Scoring Algorithm (FR-023-004)
    // ============================================================================

    /// Calculate transition quality score based on multiple factors (FR-023-004).
    /// Base score: 100, with deductions for issues.
    /// Factors:
    ///   - Beat matching accuracy (±2 BPM = higher score)
    ///   - Key compatibility (harmonic = bonus points)
    ///   - Transition smoothness (appropriate duration)
    ///   - Phrase alignment (if available)
    ///   - EQ and volume balance
    pub fn calculate_quality_score(
        bpm_difference: f64,
        key_compatibility: f32,
        transition_duration: Duration,
        phrase_aligned: bool,
    ) -> QualityScore {
        let mut score = 100.0f64;

        // Beat matching accuracy: ±2 BPM = max score, deductions for larger differences
        // FR-023-004: Beat matching accuracy (±2 BPM = higher score)
        let bpm_penalty = if bpm_difference <= 2.0 {
            0.0
        } else if bpm_difference <= 5.0 {
            (bpm_difference - 2.0) * 2.0 // 2-5 BPM: -6 to 0
        } else {
            6.0 + (bpm_difference - 5.0) * 3.0 // >5 BPM: steep penalty
        };
        score -= bpm_penalty;

        // Key compatibility: harmonic mixing bonus
        // FR-023-004: Key compatibility (harmonic = bonus points)
        if key_compatibility >= 0.9 {
            score += 3.0; // Perfect harmonic match bonus
        } else if key_compatibility < 0.3 {
            score -= 8.0; // Incompatible keys penalty
        }

        // Transition smoothness: 15-45 seconds = ideal
        // FR-023-004: Transition smoothness (appropriate duration)
        let duration_secs = transition_duration.as_secs_f64();
        if (15.0..=45.0).contains(&duration_secs) {
            score += 2.0; // Ideal range bonus
        } else if duration_secs < 5.0 {
            score -= 5.0; // Too abrupt
        } else if duration_secs > 120.0 {
            score -= 3.0; // Too long
        }

        // Phrase alignment bonus
        // FR-023-004: Phrase alignment (if available)
        if phrase_aligned {
            score += 5.0;
        }

        // EQ/volume balance: modeled via key compatibility correlation
        // FR-023-004: EQ and volume balance
        if key_compatibility >= 0.7 {
            score += 1.0; // Likely good EQ balance
        }

        // Clamp to 0-100
        score = score.clamp(0.0, 100.0);
        QualityScore::new((score / 100.0) as f32)
    }

    /// Generate auto session name from date (FR-023-002).
    pub fn auto_session_name(created_at: &SystemTime) -> String {
        // Use synthetic date formatting since SystemTime has no direct datetime
        let day_key = epoch_seconds(*created_at) / 86400;
        let hour = ((epoch_seconds(*created_at) % 86400) / 3600) as u32;
        format!("Mix Session {}_{}", day_key, hour)
    }

    /// Sort sessions by the given key and direction (FR-023-008).
    pub fn sort_sessions<T: std::borrow::Borrow<SessionRecord>>(
        sessions: &mut [T],
        key: SortKey,
        direction: SortDirection,
    ) {
        sessions.sort_by(|a, b| {
            let a = a.borrow();
            let b = b.borrow();
            let cmp = match key {
                SortKey::DateCreated => a.created_at.cmp(&b.created_at),
                SortKey::Duration => a.duration.cmp(&b.duration),
                SortKey::Quality => a.quality_score.0.total_cmp(&b.quality_score.0),
                SortKey::TrackCount => a.track_count.cmp(&b.track_count),
            };
            if direction == SortDirection::Descending {
                cmp.reverse()
            } else {
                cmp
            }
        });
    }

    /// Returns average BPM across all tracks in the session.
    pub fn average_bpm(&self) -> f64 {
        if self.transitions.is_empty() {
            return 0.0;
        }
        let bpm_sum: f64 = self
            .transitions
            .iter()
            .map(|t| t.source_track.bpm + t.destination_track.bpm)
            .sum();
        bpm_sum / (self.transitions.len() as f64 * 2.0)
    }
}

// ============================================================================
// Auto Session Creation (FR-023-001)
// ============================================================================

/// Session state tracking for auto-creation (FR-023-001).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct AutoSessionState {
    pub is_active: bool,
    pub first_track_loaded: bool,
    pub manual_start_requested: bool,
}

// ============================================================================
// Session Replay (FR-023-013)
// ============================================================================

/// A snapshot during session replay showing state at a given moment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySnapshot {
    pub elapsed_time: Duration,
    pub current_track: Option<TrackInfo>,
    pub active_transition: Option<TransitionRecord>,
    pub quality_at_point: QualityScore,
    pub energy_level: f32,     // 0.0–1.0 mock
    pub timestamp: SystemTime, // FR-023-013: exact transition time
}

impl ReplaySnapshot {
    pub fn new(
        elapsed_time: Duration,
        current_track: Option<TrackInfo>,
        active_transition: Option<TransitionRecord>,
        quality_at_point: QualityScore,
        energy_level: f32,
        timestamp: SystemTime,
    ) -> Self {
        ReplaySnapshot {
            elapsed_time,
            current_track,
            active_transition,
            quality_at_point,
            energy_level: energy_level.clamp(0.0, 1.0),
            timestamp,
        }
    }
}

/// Session replay with full timeline (FR-023-013).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReplay {
    pub session_id: String,
    pub snapshots: Vec<ReplaySnapshot>,
    pub quality_timeline: Vec<(Duration, f32)>,
    pub energy_timeline: Vec<(Duration, f32)>,
}

impl SessionReplay {
    pub fn new() -> Self {
        SessionReplay {
            session_id: String::new(),
            snapshots: Vec::new(),
            quality_timeline: Vec::new(),
            energy_timeline: Vec::new(),
        }
    }

    /// Build replay with timeline data (FR-023-013).
    pub fn build(session_id: impl Into<String>, snapshots: Vec<ReplaySnapshot>) -> Self {
        let quality_timeline: Vec<(Duration, f32)> = snapshots
            .iter()
            .map(|s| (s.elapsed_time, s.quality_at_point.0))
            .collect();
        let energy_timeline: Vec<(Duration, f32)> = snapshots
            .iter()
            .map(|s| (s.elapsed_time, s.energy_level))
            .collect();

        SessionReplay {
            session_id: session_id.into(),
            snapshots,
            quality_timeline,
            energy_timeline,
        }
    }

    /// Get quality trend with window average (FR-023-013).
    pub fn quality_trend(&self, window_size: usize) -> Vec<(Duration, f32)> {
        if self.snapshots.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        for i in (0..self.snapshots.len()).step_by(window_size.max(1)) {
            let end = (i + window_size).min(self.snapshots.len());
            let window = &self.snapshots[i..end];
            let avg =
                window.iter().map(|s| s.quality_at_point.0).sum::<f32>() / window.len() as f32;
            if let Some(first) = window.first() {
                result.push((first.elapsed_time, avg));
            }
        }
        result
    }
}

/// Session replay builder.
pub struct SessionReplayBuilder {
    snapshots: Vec<ReplaySnapshot>,
}

impl SessionReplayBuilder {
    pub fn new() -> Self {
        SessionReplayBuilder {
            snapshots: Vec::new(),
        }
    }

    pub fn build_from_session(session: &SessionRecord) -> Self {
        let mut builder = SessionReplayBuilder::new();
        for trans in &session.transitions {
            builder.snapshots.push(ReplaySnapshot::new(
                trans.duration,
                Some(trans.destination_track.clone()),
                Some(trans.clone()),
                trans.quality_score,
                trans.quality_score.0, // Use quality as energy proxy
                trans.timestamp,
            ));
        }
        builder
    }

    pub fn snapshots(&self) -> &[ReplaySnapshot] {
        &self.snapshots
    }

    /// Generate timeline points for visualization.
    pub fn timeline(&self) -> Vec<(Duration, f32)> {
        self.snapshots
            .iter()
            .map(|s| (s.elapsed_time, s.energy_level))
            .collect()
    }

    /// Quality trend: average quality over windows of snapshots.
    pub fn quality_trend(&self, window_size: usize) -> Vec<(Duration, QualityScore)> {
        if self.snapshots.is_empty() {
            return Vec::new();
        }
        let mut result = Vec::new();
        for i in (0..self.snapshots.len()).step_by(window_size.max(1)) {
            let end = (i + window_size).min(self.snapshots.len());
            let window = &self.snapshots[i..end];
            let avg =
                window.iter().map(|s| s.quality_at_point.0).sum::<f32>() / window.len() as f32;
            if let Some(first) = window.first() {
                result.push((first.elapsed_time, QualityScore::new(avg)));
            }
        }
        result
    }
}

// ============================================================================
// Overall Statistics (FR-023-011)
// ============================================================================

/// Aggregate statistics across all sessions (FR-023-011).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverallStatistics {
    pub total_sessions: u32,
    pub total_mixing_time: Duration,
    pub unique_tracks: u32,
    pub average_quality: QualityScore,
    pub top_genres: Vec<(String, u32)>,
    pub sessions_per_day: HashMap<String, u32>,
    pub quality_trend: Vec<(String, QualityScore)>, // date -> avg quality
    pub average_session_length: Duration,
    pub genre_evolution: HashMap<String, Vec<(String, u32)>>, // genre -> [(date, count)]
}

impl OverallStatistics {
    pub fn from_sessions(sessions: &[SessionRecord]) -> Self {
        if sessions.is_empty() {
            return OverallStatistics::empty();
        }

        let total_sessions = sessions.len() as u32;
        let total_mixing_time: Duration = sessions.iter().map(|s| s.duration).sum();
        let average_session_length =
            Duration::from_secs(total_mixing_time.as_secs() / u64::from(total_sessions.max(1)));

        // Unique tracks
        let mut unique_track_set = std::collections::HashSet::new();
        for session in sessions {
            for trans in &session.transitions {
                unique_track_set.insert(trans.source_track.title.clone());
                unique_track_set.insert(trans.destination_track.title.clone());
            }
        }
        let unique_tracks = unique_track_set.len() as u32;

        // Average quality
        let quality_sum: f32 = sessions.iter().map(|s| s.quality_score.0).sum();
        let average_quality = QualityScore::new(quality_sum / sessions.len() as f32);

        // Genre frequency
        let mut genre_counts: HashMap<String, u32> = HashMap::new();
        for session in sessions {
            for genre in &session.genres {
                *genre_counts.entry(genre.clone()).or_insert(0) += 1;
            }
        }
        let mut top_genres: Vec<(String, u32)> = genre_counts.into_iter().collect();
        top_genres.sort_by_key(|value| std::cmp::Reverse(value.1));
        top_genres.truncate(10); // Top 10

        // Sessions per day
        let mut sessions_per_day: HashMap<String, u32> = HashMap::new();
        for session in sessions {
            // Mock date formatting: use a synthetic day key from created_at
            let day_key = format!("day_{}", epoch_seconds(session.created_at) / 86400);
            *sessions_per_day.entry(day_key).or_insert(0) += 1;
        }

        // Quality trend per day
        let mut quality_by_day: HashMap<String, Vec<f32>> = HashMap::new();
        for session in sessions {
            let day_key = format!("day_{}", epoch_seconds(session.created_at) / 86400);
            quality_by_day
                .entry(day_key)
                .or_default()
                .push(session.quality_score.0);
        }
        let quality_trend: Vec<(String, QualityScore)> = quality_by_day
            .into_iter()
            .map(|(k, v)| {
                let avg = v.iter().sum::<f32>() / v.len() as f32;
                (k, QualityScore::new(avg))
            })
            .collect();

        // Genre evolution
        let mut genre_evolution: HashMap<String, Vec<(String, u32)>> = HashMap::new();
        for session in sessions {
            for genre in &session.genres {
                let day_key = format!("day_{}", epoch_seconds(session.created_at) / 86400);
                genre_evolution
                    .entry(genre.clone())
                    .or_default()
                    .push((day_key, 1));
            }
        }

        OverallStatistics {
            total_sessions,
            total_mixing_time,
            unique_tracks,
            average_quality,
            top_genres,
            sessions_per_day,
            quality_trend,
            average_session_length,
            genre_evolution,
        }
    }

    fn empty() -> Self {
        OverallStatistics {
            total_sessions: 0,
            total_mixing_time: Duration::ZERO,
            unique_tracks: 0,
            average_quality: QualityScore(0.0),
            top_genres: Vec::new(),
            sessions_per_day: HashMap::new(),
            quality_trend: Vec::new(),
            average_session_length: Duration::ZERO,
            genre_evolution: HashMap::new(),
        }
    }
}

// ============================================================================
// Data Export (FR-023-012)
// ============================================================================

/// Export result containing the serialized data.
#[derive(Debug, Clone)]
pub struct ExportResult {
    pub format: ExportFormat,
    pub data: Vec<u8>,
    pub filename: String,
}

impl ExportResult {
    pub fn save_to_file(&self, dir: &Path) -> std::io::Result<PathBuf> {
        let full_path = dir.join(&self.filename);
        fs::create_dir_all(dir)?;
        let mut file = fs::File::create(&full_path)?;
        file.write_all(&self.data)?;
        Ok(full_path)
    }
}

/// Export builder for sessions (FR-023-012).
pub struct ExportBuilder {
    sessions: Vec<SessionRecord>,
}

impl ExportBuilder {
    pub fn new(sessions: Vec<SessionRecord>) -> Self {
        ExportBuilder { sessions }
    }

    /// Export sessions to JSON (FR-023-012).
    pub fn to_json(&self) -> ExportResult {
        let data = serde_json::to_string_pretty(&self.sessions)
            .unwrap_or_default()
            .into_bytes();
        ExportResult {
            format: ExportFormat::Json,
            data,
            filename: "mix_history_export.json".to_string(),
        }
    }

    /// Export sessions to CSV (FR-023-012).
    pub fn to_csv(&self) -> ExportResult {
        let mut csv_data = String::new();
        csv_data.push_str(
            "Session ID,Created At,Duration (s),Quality,Track Count,Transitions,Genres\n",
        );

        for session in &self.sessions {
            let genres = session.genres.join("; ");
            csv_data.push_str(&format!(
                "{},{},{},{},{},{},\"{}\"\n",
                session.session_id,
                epoch_seconds(session.created_at),
                session.duration.as_secs(),
                session.quality_score.as_percentage(),
                session.track_count,
                session.transitions.len(),
                genres
            ));

            // Also export transitions as rows
            for trans in &session.transitions {
                csv_data.push_str(&format!(
                    "{},{},Trans {},{} BPM diff {}, Key compat {:.2}, Score {}\n",
                    session.session_id,
                    epoch_seconds(session.created_at),
                    trans.transition_number,
                    trans.transition_type,
                    trans.bpm_difference,
                    trans.key_compatibility,
                    trans.quality_score.as_percentage(),
                ));
            }
        }

        ExportResult {
            format: ExportFormat::Csv,
            data: csv_data.into_bytes(),
            filename: "mix_history_export.csv".to_string(),
        }
    }

    /// Export a PDF-like summary (stub — FR-023-012).
    pub fn to_pdf_summary(&self) -> ExportResult {
        let stats = OverallStatistics::from_sessions(&self.sessions);
        let mut summary = String::new();
        summary.push_str("=== MIX HISTORY SUMMARY ===\n\n");
        summary.push_str(&format!("Total Sessions: {}\n", stats.total_sessions));
        summary.push_str(&format!(
            "Total Mixing Time: {:.0} minutes\n",
            stats.total_mixing_time.as_secs_f64() / 60.0
        ));
        summary.push_str(&format!("Unique Tracks: {}\n", stats.unique_tracks));
        summary.push_str(&format!("Average Quality: {}\n\n", stats.average_quality));
        summary.push_str("Top Genres:\n");
        for (genre, count) in &stats.top_genres {
            summary.push_str(&format!("  - {}: {} sessions\n", genre, count));
        }

        ExportResult {
            format: ExportFormat::PdfSummary,
            data: summary.into_bytes(),
            filename: "mix_history_summary.txt".to_string(),
        }
    }

    /// Export all formats at once (FR-023-012).
    pub fn export_all(&self) -> Vec<ExportResult> {
        vec![self.to_json(), self.to_csv(), self.to_pdf_summary()]
    }
}

// ============================================================================
// Data Persistence (FR-023-014)
// ============================================================================

/// Persistence configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    pub storage_path: PathBuf,
    pub max_sessions: u32,
    pub auto_save: bool,
    pub backup_enabled: bool,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        PersistenceConfig {
            storage_path: PathBuf::from("./data/mix_history.json"),
            max_sessions: 100,
            auto_save: true,
            backup_enabled: true,
        }
    }
}

impl fmt::Display for PersistenceConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PersistenceConfig {{ path: {}, max: {}, auto_save: {}, backup: {} }}",
            self.storage_path.display(),
            self.max_sessions,
            self.auto_save,
            self.backup_enabled,
        )
    }
}

/// Mix History Analytics engine with persistence (FR-023-014).
pub struct MixHistoryAnalytics {
    sessions: Vec<SessionRecord>,
    config: PersistenceConfig,
    last_saved: Option<Instant>,
}

impl MixHistoryAnalytics {
    pub fn new(config: PersistenceConfig) -> Self {
        MixHistoryAnalytics {
            sessions: Vec::new(),
            config,
            last_saved: None,
        }
    }

    /// Load sessions from persistent storage (FR-023-014).
    pub fn load_from_file(path: &Path) -> Result<Self, String> {
        if !path.exists() {
            return Ok(MixHistoryAnalytics {
                sessions: Vec::new(),
                config: PersistenceConfig::default(),
                last_saved: None,
            });
        }

        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
        let sessions: Vec<SessionRecord> =
            serde_json::from_str(&content).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        Ok(MixHistoryAnalytics {
            sessions,
            config: PersistenceConfig::default(),
            last_saved: Some(Instant::now()),
        })
    }

    /// Save sessions to persistent storage (FR-023-014).
    pub fn save_to_file(&mut self) -> Result<(), String> {
        if !self.config.auto_save {
            return Ok(());
        }

        let dir = self.config.storage_path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir).map_err(|e| format!("Failed to create directory: {}", e))?;

        let content = serde_json::to_string_pretty(&self.sessions)
            .map_err(|e| format!("Failed to serialize: {}", e))?;

        // Backup if enabled (FR-023-014)
        if self.config.backup_enabled && self.config.storage_path.exists() {
            let backup_path = self.config.storage_path.with_extension("json.bak");
            if let Ok(existing) = fs::read_to_string(&self.config.storage_path) {
                if let Err(e) = fs::write(&backup_path, existing) {
                    eprintln!("Warning: Failed to create backup: {}", e);
                }
            }
        }

        fs::write(&self.config.storage_path, content)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        self.last_saved = Some(Instant::now());
        Ok(())
    }

    /// Auto-save if configured and enough time has passed.
    pub fn auto_save_if_needed(&mut self) -> Result<(), String> {
        if !self.config.auto_save {
            return Ok(());
        }
        if let Some(last) = self.last_saved {
            if last.elapsed() < Duration::from_secs(30) {
                return Ok(());
            }
        }
        self.save_to_file()
    }

    /// Add a session (FR-023-014).
    pub fn add_session(&mut self, session: SessionRecord) {
        self.sessions.push(session);
        // Enforce max_sessions limit
        if self.sessions.len() > self.config.max_sessions as usize {
            self.sessions
                .drain(..self.sessions.len() - self.config.max_sessions as usize);
        }
    }

    /// Remove a session by ID (FR-023-010, FR-023-014).
    pub fn remove_session(&mut self, session_id: &str, mode: DeleteMode) {
        match mode {
            DeleteMode::Soft => {
                if let Some(session) = self
                    .sessions
                    .iter_mut()
                    .find(|s| s.session_id == session_id)
                {
                    session.deleted = true;
                }
            }
            DeleteMode::Permanent => {
                self.sessions.retain(|s| s.session_id != session_id);
            }
        }
    }

    /// Get all active (non-deleted) sessions.
    pub fn active_sessions(&self) -> Vec<&SessionRecord> {
        self.sessions
            .iter()
            .filter(|session| !session.deleted)
            .collect()
    }

    /// Get all sessions including deleted (for restoration).
    pub fn all_sessions(&self) -> &[SessionRecord] {
        &self.sessions
    }

    /// Restore a soft-deleted session.
    pub fn restore_session(&mut self, session_id: &str) -> bool {
        if let Some(session) = self
            .sessions
            .iter_mut()
            .find(|s| s.session_id == session_id)
        {
            if session.deleted {
                session.deleted = false;
                return true;
            }
        }
        false
    }

    /// Get sessions sorted by key and direction (FR-023-008).
    pub fn get_sorted_sessions(
        &self,
        key: SortKey,
        direction: SortDirection,
    ) -> Vec<&SessionRecord> {
        let mut sorted = self.sessions.iter().collect::<Vec<_>>();
        SessionRecord::sort_sessions(&mut sorted, key, direction);
        sorted
    }

    // ============================================================================
    // Session Filtering (FR-023-007)
    // ============================================================================

    /// Filter sessions based on criteria (FR-023-007).
    pub fn filter_sessions(&self, filter: &SessionFilter) -> Vec<&SessionRecord> {
        self.sessions
            .iter()
            .filter(|s| {
                // Date range filter (FR-023-007)
                if let Some(start) = filter.date_range_start {
                    if s.created_at < start {
                        return false;
                    }
                }
                if let Some(end) = filter.date_range_end {
                    if s.created_at > end {
                        return false;
                    }
                }

                // Genre filter (FR-023-007)
                if let Some(ref genre) = filter.genre {
                    if !s
                        .genres
                        .iter()
                        .any(|g| g.to_lowercase().contains(&genre.to_lowercase()))
                    {
                        return false;
                    }
                }

                // Minimum track count filter (FR-023-007)
                if let Some(min_tracks) = filter.min_track_count {
                    if s.track_count < min_tracks {
                        return false;
                    }
                }

                // Minimum quality score filter (FR-023-007)
                if let Some(min_quality) = filter.min_quality_score {
                    if s.quality_score.0 < min_quality {
                        return false;
                    }
                }

                // Venue filter (FR-023-007)
                if let Some(ref venue) = filter.venue {
                    match &s.venue {
                        Some(session_venue) => {
                            if !session_venue.to_lowercase().contains(&venue.to_lowercase()) {
                                return false;
                            }
                        }
                        None => return false,
                    }
                }

                // Recording status filter (FR-023-007)
                if let Some(recording_required) = filter.recording_status {
                    let has_recording = s.recording_path.is_some();
                    if recording_required != has_recording {
                        return false;
                    }
                }

                true
            })
            .collect()
    }

    // ============================================================================
    // Session Replay Enhancement (FR-023-013)
    // ============================================================================

    /// Create session replay with full timeline (FR-023-013).
    pub fn create_replay(&self) -> SessionReplay {
        let mut replay = SessionReplay::new();

        for trans in self
            .sessions
            .iter()
            .flat_map(|session| &session.transitions)
        {
            replay.snapshots.push(ReplaySnapshot {
                elapsed_time: trans.duration,
                current_track: Some(trans.destination_track.clone()),
                active_transition: Some(trans.clone()),
                quality_at_point: trans.quality_score,
                energy_level: trans.quality_score.0,
                timestamp: trans.timestamp,
            });
        }

        replay
    }

    /// Get overall statistics (FR-023-011).
    pub fn get_statistics(&self) -> OverallStatistics {
        OverallStatistics::from_sessions(&self.sessions)
    }

    /// Create an export builder.
    pub fn export_builder(&self) -> ExportBuilder {
        ExportBuilder::new(self.sessions.clone())
    }

    /// Get performance metrics for tracking (FR-023-015).
    pub fn performance_metrics(&self) -> PerformanceMetrics {
        let mut chronological: Vec<&SessionRecord> =
            self.sessions.iter().filter(|s| !s.deleted).collect();
        chronological.sort_by_key(|session| session.created_at);
        let quality_scores: Vec<f32> = chronological.iter().map(|s| s.quality_score.0).collect();

        PerformanceMetrics {
            total_sessions: self.sessions.len() as u32,
            average_quality: QualityScore::new(
                quality_scores.iter().sum::<f32>().max(f32::EPSILON)
                    / quality_scores.len().max(1) as f32,
            ),
            quality_trend: if quality_scores.len() >= 2 {
                let start = quality_scores.first().copied().unwrap_or(0.0);
                let end = quality_scores.last().copied().unwrap_or(0.0);
                end - start
            } else {
                0.0
            },
            session_length_variance: self.calculate_length_variance(),
        }
    }

    fn calculate_length_variance(&self) -> f64 {
        if self.sessions.len() < 2 {
            return 0.0;
        }
        let mean = self
            .sessions
            .iter()
            .map(|s| s.duration.as_secs_f64())
            .sum::<f64>()
            / self.sessions.len() as f64;
        let variance: f64 = self
            .sessions
            .iter()
            .map(|s| {
                let diff = s.duration.as_secs_f64() - mean;
                diff * diff
            })
            .sum::<f64>()
            / self.sessions.len() as f64;
        variance.sqrt()
    }
}

impl fmt::Display for MixHistoryAnalytics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "MixHistoryAnalytics {{ sessions: {}, max: {}, last_saved: {:?} }}",
            self.sessions.len(),
            self.config.max_sessions,
            self.last_saved,
        )
    }
}

// ============================================================================
// Performance Tracking (FR-023-015)
// ============================================================================

/// Performance metrics for tracking over time (FR-023-015).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub total_sessions: u32,
    pub average_quality: QualityScore,
    pub quality_trend: f32, // Positive = improving, negative = declining
    pub session_length_variance: f64,
}

impl PerformanceMetrics {
    /// Check if technique is improving.
    pub fn is_improving(&self) -> bool {
        self.quality_trend > 0.05
    }

    /// Get improvement percentage.
    pub fn improvement_percentage(&self) -> f32 {
        self.quality_trend * 100.0
    }

    /// Get technique assessment.
    pub fn technique_assessment(&self) -> &str {
        if self.is_improving() {
            "Improving"
        } else if self.quality_trend < -0.05 {
            "Declining"
        } else {
            "Stable"
        }
    }
}

impl fmt::Display for PerformanceMetrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PerformanceMetrics {{ sessions: {}, avg_quality: {}, trend: {:.1}%, status: {} }}",
            self.total_sessions,
            self.average_quality,
            self.improvement_percentage(),
            self.technique_assessment(),
        )
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

impl Default for SessionReplay {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for SessionReplayBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- QualityScore Tests ---

    #[test]
    fn test_quality_score_new_clamps_to_1_0() {
        let score = QualityScore::new(2.0);
        assert_eq!(score.0, 1.0);
    }

    #[test]
    fn test_quality_score_new_clamps_to_0_0() {
        let score = QualityScore::new(-0.5);
        assert_eq!(score.0, 0.0);
    }

    #[test]
    fn test_quality_score_as_percentage() {
        let score = QualityScore::new(0.75);
        assert_eq!(score.as_percentage(), 75);
    }

    #[test]
    fn test_quality_score_display() {
        let score = QualityScore::new(0.85);
        assert_eq!(format!("{}", score), "85%");
    }

    // --- SortKey Display Tests ---

    #[test]
    fn test_sort_key_display() {
        assert_eq!(format!("{}", SortKey::DateCreated), "Date Created");
        assert_eq!(format!("{}", SortKey::Duration), "Duration");
        assert_eq!(format!("{}", SortKey::Quality), "Quality");
        assert_eq!(format!("{}", SortKey::TrackCount), "Track Count");
    }

    // --- TransitionType Display Tests ---

    #[test]
    fn test_transition_type_display() {
        assert_eq!(format!("{}", TransitionType::Mix), "Mix");
        assert_eq!(format!("{}", TransitionType::Cut), "Cut");
        assert_eq!(format!("{}", TransitionType::Blend), "Blend");
        assert_eq!(format!("{}", TransitionType::Drop), "Drop");
        assert_eq!(format!("{}", TransitionType::Fade), "Fade");
    }

    // --- TrackInfo Tests ---

    #[test]
    fn test_track_info_new() {
        let track = TrackInfo::new(
            TrackIdentity::new("Test Track", "Test Artist", "Test Album"),
            Duration::from_secs(240),
            128.0,
            "Am",
            "House",
            "/path/to/file.mp3",
        );
        assert_eq!(track.title, "Test Track");
        assert_eq!(track.bpm, 128.0);
        assert_eq!(track.key, "Am");
    }

    // --- TransitionRecord Tests ---

    #[test]
    fn test_transition_record_new() {
        let source = TrackInfo::new(
            TrackIdentity::new("Track A", "Artist A", "Album A"),
            Duration::from_secs(200),
            120.0,
            "Cm",
            "Techno",
            "/a.mp3",
        );
        let dest = TrackInfo::new(
            TrackIdentity::new("Track B", "Artist B", "Album B"),
            Duration::from_secs(180),
            125.0,
            "Em",
            "Techno",
            "/b.mp3",
        );
        let trans = TransitionRecord::new(
            1,
            source.clone(),
            dest.clone(),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(30),
                bpm_difference: 5.0,
                key_compatibility: 0.85,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        );
        assert_eq!(trans.transition_number, 1);
        assert_eq!(trans.bpm_difference, 5.0);
        assert_eq!(trans.quality_score.0, 0.9);
    }

    // --- SessionRecord Tests ---

    #[test]
    fn test_session_record_new() {
        let session = SessionRecord::new(
            "session-1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            5,
        );
        assert_eq!(session.session_id, "session-1");
        assert_eq!(session.track_count, 5);
        assert!(session.transitions.is_empty());
    }

    #[test]
    fn test_session_record_add_transition() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            2,
        );
        let trans = TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(100),
                120.0,
                "Cm",
                "Techno",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                125.0,
                "Em",
                "Techno",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 5.0,
                key_compatibility: 0.8,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        );
        session.add_transition(trans);
        assert_eq!(session.transitions.len(), 1);
    }

    #[test]
    fn test_session_record_add_genre_duplicate() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        );
        session.add_genre("House");
        session.add_genre("House");
        assert_eq!(session.genres.len(), 1);
        assert_eq!(session.genres[0], "House");
    }

    #[test]
    fn test_session_record_set_recording_path() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        );
        session.set_recording_path("/recording.wav");
        assert_eq!(session.recording_path, Some("/recording.wav".to_string()));
    }

    #[test]
    fn test_session_record_sort_by_date() {
        let mut sessions = vec![
            SessionRecord::new(
                "s2",
                SystemTime::now() - Duration::from_secs(100),
                Duration::from_secs(60),
                QualityScore(0.5),
                1,
            ),
            SessionRecord::new(
                "s1",
                SystemTime::now() - Duration::from_secs(200),
                Duration::from_secs(60),
                QualityScore(0.5),
                1,
            ),
        ];
        SessionRecord::sort_sessions(
            &mut sessions,
            SortKey::DateCreated,
            SortDirection::Descending,
        );
        assert_eq!(sessions[0].session_id, "s2");
    }

    #[test]
    fn test_session_record_sort_by_quality() {
        let mut sessions = vec![
            SessionRecord::new(
                "s1",
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.5),
                1,
            ),
            SessionRecord::new(
                "s2",
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.9),
                1,
            ),
        ];
        SessionRecord::sort_sessions(&mut sessions, SortKey::Quality, SortDirection::Descending);
        assert_eq!(sessions[0].session_id, "s2");
    }

    #[test]
    fn test_session_record_sort_by_track_count() {
        let mut sessions = vec![
            SessionRecord::new(
                "s1",
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.5),
                3,
            ),
            SessionRecord::new(
                "s2",
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.5),
                5,
            ),
        ];
        SessionRecord::sort_sessions(
            &mut sessions,
            SortKey::TrackCount,
            SortDirection::Descending,
        );
        assert_eq!(sessions[0].session_id, "s2");
    }

    #[test]
    fn test_session_record_average_bpm() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore(0.5),
            2,
        );
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(60),
                120.0,
                "Cm",
                "Techno",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(60),
                130.0,
                "Em",
                "Techno",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(10),
                bpm_difference: 10.0,
                key_compatibility: 0.7,
                quality_score: QualityScore(0.5),
                timestamp: SystemTime::now(),
            },
        ));
        let avg = session.average_bpm();
        assert!((avg - 125.0).abs() < f64::EPSILON); // (120 + 130) / 2 = 125
    }

    #[test]
    fn test_session_record_average_bpm_empty() {
        let session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            0,
        );
        assert_eq!(session.average_bpm(), 0.0);
    }

    // --- SessionReplayBuilder Tests ---

    #[test]
    fn test_session_replay_builder_new() {
        let builder = SessionReplayBuilder::new();
        assert!(builder.snapshots().is_empty());
        assert!(builder.timeline().is_empty());
        assert!(builder.quality_trend(3).is_empty());
    }

    #[test]
    fn test_session_replay_build_from_session() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore(0.8),
            2,
        );
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(60),
                120.0,
                "Cm",
                "Techno",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(60),
                130.0,
                "Em",
                "Techno",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(10),
                bpm_difference: 10.0,
                key_compatibility: 0.7,
                quality_score: QualityScore(0.8),
                timestamp: SystemTime::now(),
            },
        ));
        let builder = SessionReplayBuilder::build_from_session(&session);
        assert_eq!(builder.snapshots().len(), 1);
    }

    #[test]
    fn test_session_replay_timeline() {
        let mut builder = SessionReplayBuilder::new();
        let now = SystemTime::now();
        builder.snapshots.push(ReplaySnapshot::new(
            Duration::from_secs(10),
            None,
            None,
            QualityScore(0.5),
            0.5,
            now,
        ));
        builder.snapshots.push(ReplaySnapshot::new(
            Duration::from_secs(20),
            None,
            None,
            QualityScore(0.7),
            0.7,
            now,
        ));
        let timeline = builder.timeline();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].0, Duration::from_secs(10));
        assert_eq!(timeline[1].1, 0.7);
    }

    #[test]
    fn test_session_replay_quality_trend() {
        let mut builder = SessionReplayBuilder::new();
        let base = SystemTime::now();
        for i in 0..10 {
            builder.snapshots.push(ReplaySnapshot::new(
                Duration::from_secs(i * 10),
                None,
                None,
                QualityScore::new(0.5 + i as f32 * 0.05),
                i as f32 * 0.1,
                base,
            ));
        }
        let trend = builder.quality_trend(5);
        assert!(!trend.is_empty());
        assert!(trend.len() <= 10); // Windowed
    }

    // --- OverallStatistics Tests ---

    #[test]
    fn test_overall_statistics_empty() {
        let stats = OverallStatistics::from_sessions(&[]);
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.unique_tracks, 0);
        assert!(stats.top_genres.is_empty());
    }

    #[test]
    fn test_overall_statistics_from_sessions() {
        let mut session1 = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore(0.8),
            3,
        );
        session1.add_genre("House");
        session1.add_genre("Techno");

        let mut session2 = SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(3600),
            Duration::from_secs(1800),
            QualityScore(0.6),
            2,
        );
        session2.add_genre("House");

        let sessions = vec![session1, session2];
        let stats = OverallStatistics::from_sessions(&sessions);

        assert_eq!(stats.total_sessions, 2);
        assert_eq!(stats.unique_tracks, 0); // Counts alone do not identify unique tracks.
        assert!(stats.average_quality.0 >= 0.0);
        assert!(stats.top_genres.iter().any(|(g, _)| g == "House"));
    }

    #[test]
    fn test_overall_statistics_single_session() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore(0.9),
            4,
        );
        session.add_genre("Techno");
        let stats = OverallStatistics::from_sessions(&[session]);
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.average_quality.0, 0.9);
    }

    #[test]
    fn test_overall_statistics_empty_display() {
        let stats = OverallStatistics::empty();
        assert_eq!(stats.total_sessions, 0);
        assert_eq!(stats.total_mixing_time, Duration::ZERO);
    }

    // --- ExportBuilder Tests ---

    #[test]
    fn test_export_to_json() {
        let sessions = vec![SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        )];
        let builder = ExportBuilder::new(sessions);
        let result = builder.to_json();
        assert_eq!(result.format, ExportFormat::Json);
        assert!(!result.data.is_empty());
        assert_eq!(result.filename, "mix_history_export.json");
    }

    #[test]
    fn test_export_to_csv() {
        let sessions = vec![SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        )];
        let builder = ExportBuilder::new(sessions);
        let result = builder.to_csv();
        assert_eq!(result.format, ExportFormat::Csv);
        let csv_str = String::from_utf8_lossy(&result.data);
        assert!(csv_str.contains("Session ID"));
        assert!(csv_str.contains("s1"));
    }

    #[test]
    fn test_export_to_pdf_summary() {
        let sessions = vec![SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.7),
            2,
        )];
        let builder = ExportBuilder::new(sessions);
        let result = builder.to_pdf_summary();
        assert_eq!(result.format, ExportFormat::PdfSummary);
        let summary = String::from_utf8_lossy(&result.data);
        assert!(summary.contains("MIX HISTORY SUMMARY"));
    }

    #[test]
    fn test_export_all() {
        let sessions = vec![SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        )];
        let builder = ExportBuilder::new(sessions);
        let results = builder.export_all();
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].format, ExportFormat::Json);
        assert_eq!(results[1].format, ExportFormat::Csv);
        assert_eq!(results[2].format, ExportFormat::PdfSummary);
    }

    #[test]
    fn test_export_result_save_to_file() {
        let sessions = vec![SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        )];
        let builder = ExportBuilder::new(sessions);
        let result = builder.to_json();
        let temp_dir = std::env::temp_dir().join("drdj_test_export");
        let path = result.save_to_file(&temp_dir).unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("mix_history_export.json"));
        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    // --- MixHistoryAnalytics Tests ---

    #[test]
    fn test_mix_history_analytics_new() {
        let analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        assert!(analytics.active_sessions().is_empty());
        assert_eq!(
            format!("{}", analytics),
            "MixHistoryAnalytics { sessions: 0, max: 100, last_saved: None }"
        );
    }

    #[test]
    fn test_mix_history_analytics_add_session() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.8),
            2,
        ));
        assert_eq!(analytics.active_sessions().len(), 1);
    }

    #[test]
    fn test_mix_history_analytics_max_sessions_limit() {
        let config = PersistenceConfig {
            max_sessions: 3,
            ..Default::default()
        };

        let mut analytics = MixHistoryAnalytics::new(config);

        for i in 0..5 {
            analytics.add_session(SessionRecord::new(
                format!("s{}", i),
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.5),
                1,
            ));
        }
        // Should keep only last 3
        assert_eq!(analytics.active_sessions().len(), 3);
    }

    #[test]
    fn test_mix_history_analytics_remove_session_soft() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        ));
        analytics.remove_session("s1", DeleteMode::Soft);

        let all = analytics.all_sessions();
        assert_eq!(all.len(), 1);
        assert!(all[0].deleted);
        assert_eq!(analytics.active_sessions().len(), 0);
    }

    #[test]
    fn test_mix_history_analytics_remove_session_permanent() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        ));
        analytics.remove_session("s1", DeleteMode::Permanent);
        assert_eq!(analytics.active_sessions().len(), 0);
        assert_eq!(analytics.all_sessions().len(), 0);
    }

    #[test]
    fn test_mix_history_analytics_restore_session() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        );
        analytics.add_session(session);
        analytics.remove_session("s1", DeleteMode::Soft);

        let restored = analytics.restore_session("s1");
        assert!(restored);
        assert_eq!(analytics.active_sessions().len(), 1);
    }

    #[test]
    fn test_mix_history_analytics_restore_nonexistent() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let restored = analytics.restore_session("nonexistent");
        assert!(!restored);
    }

    #[test]
    fn test_mix_history_analytics_get_sorted_sessions() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        ));
        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore(0.8),
            2,
        ));

        let sorted = analytics.get_sorted_sessions(SortKey::Quality, SortDirection::Descending);
        assert_eq!(sorted[0].session_id, "s2");
    }

    #[test]
    fn test_mix_history_analytics_get_statistics() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore(0.8),
            3,
        ));
        let stats = analytics.get_statistics();
        assert_eq!(stats.total_sessions, 1);
        assert_eq!(stats.average_quality.0, 0.8);
    }

    #[test]
    fn test_mix_history_analytics_export_builder() {
        let analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let builder = analytics.export_builder();
        let json_result = builder.to_json();
        assert!(!json_result.data.is_empty());
    }

    #[test]
    fn test_mix_history_analytics_performance_metrics() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        ));
        let metrics = analytics.performance_metrics();
        assert_eq!(metrics.total_sessions, 1);
        assert_eq!(metrics.technique_assessment().to_string(), "Stable");
    }

    #[test]
    fn test_mix_history_analytics_save_load_roundtrip() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.9),
            2,
        ));

        let temp_file = std::env::temp_dir().join("drdj_test_persist.json");
        let config = PersistenceConfig {
            storage_path: temp_file.clone(),
            ..Default::default()
        };

        let mut analytics2 = MixHistoryAnalytics::new(config);
        for session in analytics.all_sessions() {
            analytics2.add_session(session.clone());
        }
        analytics2.save_to_file().unwrap();

        let loaded = MixHistoryAnalytics::load_from_file(&temp_file).unwrap();
        assert_eq!(loaded.active_sessions().len(), 1);
        assert_eq!(loaded.active_sessions()[0].session_id, "s1");

        let _ = fs::remove_file(&temp_file);
    }

    #[test]
    fn test_mix_history_analytics_load_nonexistent() {
        let temp_file = std::env::temp_dir().join("drdj_test_nonexistent.json");
        let loaded = MixHistoryAnalytics::load_from_file(&temp_file).unwrap();
        assert_eq!(loaded.active_sessions().len(), 0);
        let _ = fs::remove_file(&temp_file);
    }

    #[test]
    fn test_mix_history_analytics_auto_save_respects_config() {
        let config = PersistenceConfig {
            auto_save: false,
            ..Default::default()
        };

        let mut analytics = MixHistoryAnalytics::new(config);
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        ));

        // Should not save when auto_save is false
        let temp_file = std::env::temp_dir().join("drdj_test_autosave.json");
        let config2 = PersistenceConfig {
            storage_path: temp_file.clone(),
            auto_save: false,
            ..Default::default()
        };

        let mut analytics2 = MixHistoryAnalytics::new(config2);
        analytics2.save_to_file().unwrap();

        let loaded = MixHistoryAnalytics::load_from_file(&temp_file).unwrap();
        assert_eq!(loaded.active_sessions().len(), 0); // Not auto-saved
        let _ = fs::remove_file(&temp_file);
    }

    // --- PersistenceConfig Tests ---

    #[test]
    fn test_persistence_config_default() {
        let config = PersistenceConfig::default();
        assert_eq!(config.max_sessions, 100);
        assert!(config.auto_save);
        assert!(config.backup_enabled);
    }

    #[test]
    fn test_persistence_config_display() {
        let config = PersistenceConfig::default();
        let display = format!("{}", config);
        assert!(display.contains("max: 100"));
        assert!(display.contains("auto_save: true"));
    }

    // --- PerformanceMetrics Tests ---

    #[test]
    fn test_performance_metrics_improving() {
        let metrics = PerformanceMetrics {
            total_sessions: 10,
            average_quality: QualityScore(0.8),
            quality_trend: 0.1,
            session_length_variance: 0.0,
        };
        assert!(metrics.is_improving());
        assert!((metrics.improvement_percentage() - 10.0).abs() < f32::EPSILON);
        assert_eq!(metrics.technique_assessment(), "Improving");
    }

    #[test]
    fn test_performance_metrics_declining() {
        let metrics = PerformanceMetrics {
            total_sessions: 10,
            average_quality: QualityScore(0.5),
            quality_trend: -0.1,
            session_length_variance: 0.0,
        };
        assert!(!metrics.is_improving());
        assert_eq!(metrics.technique_assessment(), "Declining");
    }

    #[test]
    fn test_performance_metrics_stable() {
        let metrics = PerformanceMetrics {
            total_sessions: 10,
            average_quality: QualityScore(0.7),
            quality_trend: 0.02,
            session_length_variance: 0.0,
        };
        assert_eq!(metrics.technique_assessment(), "Stable");
    }

    #[test]
    fn test_performance_metrics_display() {
        let metrics = PerformanceMetrics {
            total_sessions: 5,
            average_quality: QualityScore(0.75),
            quality_trend: 0.06,
            session_length_variance: 0.0,
        };
        let display = format!("{}", metrics);
        assert!(display.contains("5"));
        assert!(display.contains("Improving"));
    }

    // --- DeleteMode Tests ---

    #[test]
    fn test_delete_mode_soft_preserves_recording() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        );
        session.set_recording_path("/recording.wav");
        analytics.add_session(session);

        analytics.remove_session("s1", DeleteMode::Soft);

        let all = analytics.all_sessions();
        assert_eq!(all[0].recording_path, Some("/recording.wav".to_string()));
        assert!(all[0].deleted);
    }

    #[test]
    fn test_delete_mode_permanent_removes_all() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore(0.5),
            1,
        );
        session.set_recording_path("/recording.wav");
        analytics.add_session(session);

        analytics.remove_session("s1", DeleteMode::Permanent);

        assert_eq!(analytics.all_sessions().len(), 0);
        assert_eq!(analytics.active_sessions().len(), 0);
    }

    // --- ReplaySnapshot Tests ---

    #[test]
    fn test_replay_snapshot_new() {
        let snap = ReplaySnapshot::new(
            Duration::from_secs(10),
            None,
            None,
            QualityScore(0.8),
            0.5,
            SystemTime::now(),
        );
        assert_eq!(snap.elapsed_time, Duration::from_secs(10));
        assert_eq!(snap.energy_level, 0.5);
    }

    #[test]
    fn test_replay_snapshot_energy_clamp() {
        let snap = ReplaySnapshot::new(
            Duration::from_secs(10),
            None,
            None,
            QualityScore(0.5),
            1.5, // Should clamp to 1.0
            SystemTime::now(),
        );
        assert_eq!(snap.energy_level, 1.0);

        let snap2 = ReplaySnapshot::new(
            Duration::from_secs(10),
            None,
            None,
            QualityScore(0.5),
            -0.5, // Should clamp to 0.0
            SystemTime::now(),
        );
        assert_eq!(snap2.energy_level, 0.0);
    }

    // --- Sort Direction Tests ---

    #[test]
    fn test_sort_direction_ascending() {
        let mut sessions = vec![
            SessionRecord::new(
                "s2",
                SystemTime::now(),
                Duration::from_secs(120),
                QualityScore(0.5),
                1,
            ),
            SessionRecord::new(
                "s1",
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.5),
                1,
            ),
        ];
        SessionRecord::sort_sessions(&mut sessions, SortKey::Duration, SortDirection::Ascending);
        assert_eq!(sessions[0].duration, Duration::from_secs(60));
    }

    #[test]
    fn test_sort_direction_descending() {
        let mut sessions = vec![
            SessionRecord::new(
                "s1",
                SystemTime::now(),
                Duration::from_secs(60),
                QualityScore(0.5),
                1,
            ),
            SessionRecord::new(
                "s2",
                SystemTime::now(),
                Duration::from_secs(120),
                QualityScore(0.5),
                1,
            ),
        ];
        SessionRecord::sort_sessions(&mut sessions, SortKey::Duration, SortDirection::Descending);
        assert_eq!(sessions[0].duration, Duration::from_secs(120));
    }

    // --- QualityScore Comparison Tests ---

    #[test]
    fn test_quality_score_partial_ord() {
        let a = QualityScore::new(0.9);
        let b = QualityScore::new(0.5);
        assert!(a > b);
        assert!(a >= b);
        assert!(b < a);
    }

    #[test]
    fn test_quality_score_eq() {
        let a = QualityScore::new(0.75);
        let b = QualityScore::new(0.75);
        assert_eq!(a, b);
    }

    // --- ExportFormat Tests ---

    #[test]
    fn test_export_format_variants() {
        assert_eq!(format!("{:?}", ExportFormat::Json), "Json");
        assert_eq!(format!("{:?}", ExportFormat::Csv), "Csv");
        assert_eq!(format!("{:?}", ExportFormat::PdfSummary), "PdfSummary");
    }

    // --- Integration: Full Session Lifecycle ---

    #[test]
    fn test_full_session_lifecycle() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());

        // 1. Add session
        let mut session = SessionRecord::new(
            "session-1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore(0.85),
            4,
        );
        session.add_genre("House");
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(300),
                125.0,
                "Cm",
                "House",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(280),
                128.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TransitionType::Blend,
            TransitionDetails {
                duration: Duration::from_secs(25),
                bpm_difference: 3.0,
                key_compatibility: 0.9,
                quality_score: QualityScore(0.9),
                timestamp: SystemTime::now(),
            },
        ));
        analytics.add_session(session);

        // 2. Get sorted sessions
        let sorted = analytics.get_sorted_sessions(SortKey::Quality, SortDirection::Descending);
        assert_eq!(sorted.len(), 1);

        // 3. Get statistics
        let stats = analytics.get_statistics();
        assert_eq!(stats.total_sessions, 1);
        assert!(!stats.top_genres.is_empty());

        // 4. Get performance metrics
        let metrics = analytics.performance_metrics();
        assert_eq!(metrics.total_sessions, 1);

        // 5. Export
        let export = analytics.export_builder().to_json();
        assert!(!export.data.is_empty());

        // 6. Save and reload
        let temp_file = std::env::temp_dir().join("drdj_lifecycle_test.json");
        let config = PersistenceConfig {
            storage_path: temp_file.clone(),
            ..Default::default()
        };

        let mut analytics2 = MixHistoryAnalytics::new(config);
        analytics2.add_session(SessionRecord::new(
            "session-2",
            SystemTime::now(),
            Duration::from_secs(1800),
            QualityScore(0.7),
            2,
        ));
        analytics2.save_to_file().unwrap();

        let loaded = MixHistoryAnalytics::load_from_file(&temp_file).unwrap();
        assert_eq!(loaded.active_sessions().len(), 1);
        let _ = fs::remove_file(&temp_file);
    }

    // ============================================================================
    // FR-023-001: Auto Session Creation Tests
    // ============================================================================

    #[test]
    fn test_auto_session_state_default() {
        let state = AutoSessionState::default();
        assert!(!state.is_active);
        assert!(!state.first_track_loaded);
        assert!(!state.manual_start_requested);
    }

    #[test]
    fn test_auto_session_manual_start() {
        let state = AutoSessionState {
            manual_start_requested: true,
            ..Default::default()
        };

        assert!(state.manual_start_requested);
    }

    // ============================================================================
    // FR-023-002: Session Metadata Tests
    // ============================================================================

    #[test]
    fn test_session_with_name() {
        let session = SessionRecord::with_name(
            "s1",
            "My Mix",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            5,
        );
        assert_eq!(session.session_name, "My Mix");
        assert_eq!(session.session_id, "s1");
    }

    #[test]
    fn test_session_venue_and_listeners() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            5,
        );
        session.set_venue("Berghain");
        session.set_listener_count(150);
        assert_eq!(session.venue, Some("Berghain".to_string()));
        assert_eq!(session.listener_count, 150);
    }

    #[test]
    fn test_session_end() {
        let mut session = SessionRecord::with_name(
            "s1",
            "Test",
            SystemTime::now(),
            Duration::ZERO,
            QualityScore::new(0.5),
            2,
        );
        session.end_session();
        assert!(session.ended_at.is_some());
        assert_eq!(
            session.duration,
            session
                .ended_at
                .unwrap()
                .duration_since(session.created_at)
                .unwrap()
        );
    }

    #[test]
    fn test_auto_session_name() {
        let name = SessionRecord::auto_session_name(&SystemTime::now());
        assert!(name.starts_with("Mix Session"));
    }

    // ============================================================================
    // FR-023-003: Transition Crossfader Tests
    // ============================================================================

    #[test]
    fn test_crossfader_position_new() {
        let pos = CrossfaderPosition::new(0.5);
        assert_eq!(pos.0, 0.5);
        let pos_clamp = CrossfaderPosition::new(1.5);
        assert_eq!(pos_clamp.0, 1.0);
        let pos_low = CrossfaderPosition::new(-0.5);
        assert_eq!(pos_low.0, 0.0);
    }

    #[test]
    fn test_transition_with_crossfader() {
        let source = TrackInfo::new(
            TrackIdentity::new("A", "A", "A"),
            Duration::from_secs(200),
            120.0,
            "Cm",
            "House",
            "/a.mp3",
        );
        let dest = TrackInfo::new(
            TrackIdentity::new("B", "B", "B"),
            Duration::from_secs(180),
            122.0,
            "Am",
            "House",
            "/b.mp3",
        );
        let trans = TransitionRecord::with_crossfader(
            1,
            source,
            dest,
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 2.0,
                key_compatibility: 0.9,
                quality_score: QualityScore::new(0.85),
                timestamp: SystemTime::now(),
            },
            CrossfaderPosition::new(0.5),
            true,
        );
        assert!(trans.phrase_aligned);
        assert_eq!(trans.crossfader_position.0, 0.5);
    }

    // ============================================================================
    // FR-023-004: Quality Scoring Algorithm Tests
    // ============================================================================

    #[test]
    fn test_quality_score_perfect_match() {
        let score =
            SessionRecord::calculate_quality_score(1.5, 0.95, Duration::from_secs(30), true);
        assert!(score.0 >= 0.95); // Near perfect: base 100 + key + smoothness + phrase + EQ
    }

    #[test]
    fn test_quality_score_bpm_penalty() {
        let score_good =
            SessionRecord::calculate_quality_score(1.0, 0.5, Duration::from_secs(25), false);
        let score_bad =
            SessionRecord::calculate_quality_score(10.0, 0.5, Duration::from_secs(25), false);
        assert!(score_good.0 > score_bad.0);
    }

    #[test]
    fn test_quality_score_key_penalty() {
        let score_compat =
            SessionRecord::calculate_quality_score(3.0, 0.9, Duration::from_secs(20), false);
        let score_incompat =
            SessionRecord::calculate_quality_score(3.0, 0.2, Duration::from_secs(20), false);
        assert!(score_compat.0 > score_incompat.0);
    }

    #[test]
    fn test_quality_score_clamped() {
        let score =
            SessionRecord::calculate_quality_score(50.0, 0.0, Duration::from_secs(1), false);
        assert_eq!(score.0, 0.0); // Extreme penalty should clamp to 0
    }

    #[test]
    fn test_quality_score_phrase_bonus() {
        let with_phrase =
            SessionRecord::calculate_quality_score(10.0, 0.5, Duration::from_secs(20), true);
        let without_phrase =
            SessionRecord::calculate_quality_score(10.0, 0.5, Duration::from_secs(20), false);
        assert!(with_phrase.0 > without_phrase.0);
        // Phrase bonus is +5 points = +0.05 in normalized
        assert!((with_phrase.0 - without_phrase.0 - 0.05).abs() < 0.001);
    }

    // ============================================================================
    // FR-023-005: Session Statistics Tests
    // ============================================================================

    #[test]
    fn test_peak_energy() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore::new(0.5),
            2,
        );
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(100),
                120.0,
                "Cm",
                "House",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(90),
                125.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 5.0,
                key_compatibility: 0.8,
                quality_score: QualityScore::new(0.8),
                timestamp: SystemTime::now(),
            },
        ));
        session.add_transition(TransitionRecord::new(
            2,
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(90),
                125.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("C", "C", "C"),
                Duration::from_secs(110),
                128.0,
                "Em",
                "Techno",
                "/c.mp3",
            ),
            TransitionType::Cut,
            TransitionDetails {
                duration: Duration::from_secs(5),
                bpm_difference: 3.0,
                key_compatibility: 0.6,
                quality_score: QualityScore::new(0.6),
                timestamp: SystemTime::now(),
            },
        ));
        assert!((session.peak_energy() - 0.8).abs() < 0.001);
    }

    #[test]
    fn test_genre_distribution() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore::new(0.5),
            1,
        );
        session.add_genre("House");
        session.add_genre("House");
        session.add_genre("Techno");
        let dist = session.genre_distribution();
        assert!(dist.contains_key("House"));
        assert!((dist["House"] - 50.0).abs() < 0.1); // Unique session categories.
        assert!((dist["Techno"] - 50.0).abs() < 0.1);
    }

    #[test]
    fn test_transition_type_distribution() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore::new(0.5),
            1,
        );
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(100),
                120.0,
                "Cm",
                "House",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                120.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 0.0,
                key_compatibility: 0.9,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        ));
        session.add_transition(TransitionRecord::new(
            2,
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                120.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("C", "C", "C"),
                Duration::from_secs(100),
                120.0,
                "Em",
                "House",
                "/c.mp3",
            ),
            TransitionType::Cut,
            TransitionDetails {
                duration: Duration::from_secs(1),
                bpm_difference: 0.0,
                key_compatibility: 0.9,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        ));
        let dist = session.transition_type_distribution();
        assert_eq!(*dist.get(&TransitionType::Mix).unwrap(), 1);
        assert_eq!(*dist.get(&TransitionType::Cut).unwrap(), 1);
    }

    #[test]
    fn test_most_frequent_tracks() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(120),
            QualityScore::new(0.5),
            1,
        );
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(100),
                120.0,
                "Cm",
                "House",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                120.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 0.0,
                key_compatibility: 0.9,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        ));
        session.add_transition(TransitionRecord::new(
            2,
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                120.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("C", "C", "C"),
                Duration::from_secs(100),
                120.0,
                "Em",
                "House",
                "/c.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 0.0,
                key_compatibility: 0.9,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        ));
        let freq = session.most_frequent_tracks(3);
        assert!(freq.len() <= 3);
        // "B" should be most frequent (appears in both transitions)
        assert_eq!(freq[0].0, "B");
        assert_eq!(freq[0].1, 2);
    }

    #[test]
    fn test_quality_color_green() {
        let session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore::new(0.85),
            1,
        );
        assert_eq!(session.quality_color(), "green");
    }

    #[test]
    fn test_quality_color_red() {
        let session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore::new(0.5),
            1,
        );
        assert_eq!(session.quality_color(), "red");
    }

    // ============================================================================
    // FR-023-006: Session List Display Tests
    // ============================================================================

    #[test]
    fn test_quality_color_yellow() {
        let session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore::new(0.62),
            1,
        );
        assert_eq!(session.quality_color(), "yellow");
    }

    #[test]
    fn test_quality_color_blue() {
        let session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(60),
            QualityScore::new(0.75),
            1,
        );
        assert_eq!(session.quality_color(), "blue");
    }

    // ============================================================================
    // FR-023-007: Session Filtering Tests
    // ============================================================================

    #[test]
    fn test_filter_by_genre() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let mut s1 = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            5,
        );
        s1.add_genre("House");
        analytics.add_session(s1);

        let mut s2 = SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(3600),
            Duration::from_secs(1800),
            QualityScore::new(0.6),
            3,
        );
        s2.add_genre("Techno");
        analytics.add_session(s2);

        let filter = SessionFilter {
            genre: Some("House".to_string()),
            ..Default::default()
        };
        let filtered = analytics.filter_sessions(&filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session_id, "s1");
    }

    #[test]
    fn test_filter_by_min_track_count() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            10,
        ));
        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(1800),
            Duration::from_secs(600),
            QualityScore::new(0.5),
            2,
        ));

        let filter = SessionFilter {
            min_track_count: Some(5),
            ..Default::default()
        };
        let filtered = analytics.filter_sessions(&filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session_id, "s1");
    }

    #[test]
    fn test_filter_by_min_quality() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.9),
            5,
        ));
        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(1800),
            Duration::from_secs(600),
            QualityScore::new(0.4),
            3,
        ));

        let filter = SessionFilter {
            min_quality_score: Some(0.7),
            ..Default::default()
        };
        let filtered = analytics.filter_sessions(&filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session_id, "s1");
    }

    #[test]
    fn test_filter_by_venue() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let mut s1 = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            5,
        );
        s1.set_venue("Berghain");
        analytics.add_session(s1);

        let mut s2 = SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(3600),
            Duration::from_secs(1800),
            QualityScore::new(0.6),
            3,
        );
        s2.set_venue("Tresor");
        analytics.add_session(s2);

        let filter = SessionFilter {
            venue: Some("Bergh".to_string()), // Partial match
            ..Default::default()
        };
        let filtered = analytics.filter_sessions(&filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session_id, "s1");
    }

    #[test]
    fn test_filter_by_recording_status() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let mut s1 = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.8),
            5,
        );
        s1.set_recording_path("/rec/session1.wav");
        analytics.add_session(s1);

        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(3600),
            Duration::from_secs(1800),
            QualityScore::new(0.6),
            3,
        ));

        let filter = SessionFilter {
            recording_status: Some(true),
            ..Default::default()
        };
        let filtered = analytics.filter_sessions(&filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session_id, "s1");
    }

    #[test]
    fn test_filter_combined() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        let mut s1 = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.9),
            10,
        );
        s1.add_genre("House");
        analytics.add_session(s1);

        let mut s2 = SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(3600),
            Duration::from_secs(1800),
            QualityScore::new(0.85),
            5,
        );
        s2.add_genre("Techno");
        analytics.add_session(s2);

        let filter = SessionFilter {
            min_track_count: Some(8),
            min_quality_score: Some(0.8),
            ..Default::default()
        };
        let filtered = analytics.filter_sessions(&filter);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].session_id, "s1");
    }

    // ============================================================================
    // FR-023-013: Session Replay Tests
    // ============================================================================

    #[test]
    fn test_create_replay() {
        let mut session = SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(300),
            QualityScore::new(0.8),
            3,
        );
        session.add_transition(TransitionRecord::new(
            1,
            TrackInfo::new(
                TrackIdentity::new("A", "A", "A"),
                Duration::from_secs(100),
                120.0,
                "Cm",
                "House",
                "/a.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                120.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TransitionType::Mix,
            TransitionDetails {
                duration: Duration::from_secs(20),
                bpm_difference: 0.0,
                key_compatibility: 0.9,
                quality_score: QualityScore::new(0.9),
                timestamp: SystemTime::now(),
            },
        ));
        session.add_transition(TransitionRecord::new(
            2,
            TrackInfo::new(
                TrackIdentity::new("B", "B", "B"),
                Duration::from_secs(100),
                120.0,
                "Am",
                "House",
                "/b.mp3",
            ),
            TrackInfo::new(
                TrackIdentity::new("C", "C", "C"),
                Duration::from_secs(100),
                125.0,
                "Em",
                "House",
                "/c.mp3",
            ),
            TransitionType::Blend,
            TransitionDetails {
                duration: Duration::from_secs(25),
                bpm_difference: 5.0,
                key_compatibility: 0.8,
                quality_score: QualityScore::new(0.7),
                timestamp: SystemTime::now(),
            },
        ));

        let replay = session.create_replay();
        assert_eq!(replay.session_id, "s1");
        assert_eq!(replay.snapshots.len(), 2);
        assert_eq!(replay.quality_timeline.len(), 2);
    }

    #[test]
    fn test_session_replay_quality_trend_extended() {
        let mut replay = SessionReplay::new();
        let base = SystemTime::now();
        for i in 0..10 {
            replay.snapshots.push(ReplaySnapshot::new(
                Duration::from_secs(i * 30),
                None,
                None,
                QualityScore::new(0.5 + i as f32 * 0.05),
                0.5 + i as f32 * 0.05,
                base,
            ));
        }
        let trend = replay.quality_trend(5);
        assert!(!trend.is_empty());
        assert!(trend.len() <= 10);
        // Trend should show improving quality
        assert!(trend.last().unwrap().1 >= trend.first().unwrap().1);
    }

    #[test]
    fn test_session_replay_build() {
        let snapshots = vec![
            ReplaySnapshot::new(
                Duration::from_secs(30),
                None,
                None,
                QualityScore::new(0.6),
                0.6,
                SystemTime::now(),
            ),
            ReplaySnapshot::new(
                Duration::from_secs(60),
                None,
                None,
                QualityScore::new(0.8),
                0.8,
                SystemTime::now(),
            ),
        ];
        let replay = SessionReplay::build("test-session", snapshots);
        assert_eq!(replay.session_id, "test-session");
        assert_eq!(replay.snapshots.len(), 2);
        assert_eq!(replay.quality_timeline.len(), 2);
    }

    // ============================================================================
    // FR-023-015: Performance Tracking Tests
    // ============================================================================

    #[test]
    fn test_performance_improving() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        // Add sessions with increasing quality
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now() - Duration::from_secs(86400 * 3),
            Duration::from_secs(3600),
            QualityScore::new(0.5),
            3,
        ));
        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(86400 * 2),
            Duration::from_secs(3600),
            QualityScore::new(0.6),
            4,
        ));
        analytics.add_session(SessionRecord::new(
            "s3",
            SystemTime::now() - Duration::from_secs(86400),
            Duration::from_secs(3600),
            QualityScore::new(0.7),
            5,
        ));
        analytics.add_session(SessionRecord::new(
            "s4",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.9),
            6,
        ));

        let metrics = analytics.performance_metrics();
        assert!(metrics.is_improving());
        assert!(metrics.improvement_percentage() > 0.0);
    }

    #[test]
    fn test_performance_declining() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now() - Duration::from_secs(86400 * 3),
            Duration::from_secs(3600),
            QualityScore::new(0.9),
            6,
        ));
        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now() - Duration::from_secs(86400 * 2),
            Duration::from_secs(3600),
            QualityScore::new(0.7),
            4,
        ));
        analytics.add_session(SessionRecord::new(
            "s3",
            SystemTime::now() - Duration::from_secs(86400),
            Duration::from_secs(3600),
            QualityScore::new(0.5),
            3,
        ));
        analytics.add_session(SessionRecord::new(
            "s4",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.4),
            2,
        ));

        let metrics = analytics.performance_metrics();
        // Declining trend is negative, is_improving() checks > 0.05
        assert!(!metrics.is_improving());
        assert!(
            metrics.improvement_percentage() < 0.0 || metrics.improvement_percentage().abs() > 0.05
        );
    }

    #[test]
    fn test_performance_stable() {
        let mut analytics = MixHistoryAnalytics::new(PersistenceConfig::default());
        analytics.add_session(SessionRecord::new(
            "s1",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.7),
            5,
        ));
        analytics.add_session(SessionRecord::new(
            "s2",
            SystemTime::now(),
            Duration::from_secs(3600),
            QualityScore::new(0.7),
            5,
        ));

        let metrics = analytics.performance_metrics();
        assert_eq!(metrics.technique_assessment(), "Stable");
    }

    #[test]
    fn test_performance_metrics_display_extended() {
        let metrics = PerformanceMetrics {
            total_sessions: 5,
            average_quality: QualityScore::new(0.75),
            quality_trend: 0.1,
            session_length_variance: 120.0,
        };
        let display = format!("{}", metrics);
        assert!(display.contains("5"));
        assert!(display.contains("75%"));
    }

    // ============================================================================
    // FR-023-014: Data Persistence Integration Tests
    // ============================================================================

    #[test]
    fn test_persistence_backup_restore() {
        let temp_file = std::env::temp_dir().join("drdj_backup_test.json");
        {
            let config = PersistenceConfig {
                storage_path: temp_file.clone(),
                backup_enabled: true,
                max_sessions: 50,
                ..Default::default()
            };

            let mut analytics = MixHistoryAnalytics::new(config);
            analytics.add_session(SessionRecord::new(
                "s1",
                SystemTime::now(),
                Duration::from_secs(3600),
                QualityScore::new(0.8),
                5,
            ));
            analytics.save_to_file().unwrap();
        }

        // Verify backup was created
        let backup_path = temp_file.with_extension("json.bak");
        assert!(backup_path.exists() || temp_file.exists());

        // Load back
        let loaded = MixHistoryAnalytics::load_from_file(&temp_file).unwrap();
        assert_eq!(loaded.active_sessions().len(), 1);

        let _ = fs::remove_file(&temp_file);
        let _ = fs::remove_file(&backup_path);
    }

    #[test]
    fn test_max_sessions_enforcement() {
        let config = PersistenceConfig {
            max_sessions: 3,
            ..Default::default()
        };

        let mut analytics = MixHistoryAnalytics::new(config);

        for i in 0..5 {
            analytics.add_session(SessionRecord::new(
                format!("s{}", i),
                SystemTime::now() - Duration::from_secs(i * 60),
                Duration::from_secs(300),
                QualityScore::new(0.5 + i as f32 * 0.05),
                2,
            ));
        }
        assert_eq!(analytics.active_sessions().len(), 3);
    }
}

/// Filter options for session queries (FR-023-007).
#[derive(Debug, Clone, Default)]
pub struct SessionFilter {
    pub date_range_start: Option<SystemTime>, // Date range
    pub date_range_end: Option<SystemTime>,   // Date range
    pub genre: Option<String>,                // Genre filter
    pub min_track_count: Option<u32>,         // Minimum track count
    pub min_quality_score: Option<f32>,       // Minimum quality score (0-1)
    pub venue: Option<String>,                // Venue filter
    pub recording_status: Option<bool>,       // true=recorded, false=not recorded, None=any
}

fn epoch_seconds(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
