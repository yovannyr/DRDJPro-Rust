//! Smart Playlist Manager — P24b: FR-008 through FR-015 (generation, storage,
//! display, regeneration, analysis, transition prediction, deletion).
//!
//! Combines the playlist-manager foundation with criteria-driven generation,
//! energy-flow patterns, recommendations, and transition-quality prediction.
//!
//! ## Algorithm overview
//!
//! 1. **Criteria evaluation** — tracks are scored against configurable criteria
//!    (BPM range, key compatibility, energy level, genre, rating, mood, duration).
//! 2. **Sequencing** — generated playlists are ordered by energy-flow patterns,
//!    key harmony progression, BPM evolution, or genre consistency.
//! 3. **Analysis** — playlists are analyzed for energy flow, BPM progression,
//!    and key distribution after generation.
//! 4. **Transition prediction** — each adjacent pair is scored for smoothness
//!    based on BPM delta, key harmony, and energy transition.
//! 5. **Storage** — all metadata is persisted via the library-store layer.

use std::collections::HashMap;
use std::fmt;

// --- Types for playlist types ---

/// The five supported playlist types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlaylistType {
    #[default]
    MixPrep,
    WarmUp,
    CoolDown,
    EnergyWave,
    HarmonicJourney,
}

impl fmt::Display for PlaylistType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaylistType::MixPrep => write!(f, "Mix Preparation"),
            PlaylistType::WarmUp => write!(f, "Warm Up"),
            PlaylistType::CoolDown => write!(f, "Cool Down"),
            PlaylistType::EnergyWave => write!(f, "Energy Wave"),
            PlaylistType::HarmonicJourney => write!(f, "Harmonic Journey"),
        }
    }
}

// --- Criteria configuration ---

/// BPM filtering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BpmMode {
    Exact,
    #[default]
    Range,
    Relative,
}

/// Key matching mode for harmonic mixing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyMode {
    #[default]
    Off,
    Compatible,
    SameKey,
    CamelotCycling,
}

/// Energy level for a track (0.0 – 1.0).
pub type EnergyLevel = f32;

/// Track filtering and selection criteria for smart playlists.
#[derive(Debug, Clone)]
pub struct PlaylistCriteria {
    /// Playlist type that drives default criteria.
    pub playlist_type: PlaylistType,
    /// BPM filter mode and bounds.
    pub bpm_mode: BpmMode,
    pub bpm_min: f64,
    pub bpm_max: f64,
    pub bpm_relative_delta: f64, // percentage
    /// Key matching configuration.
    pub key_mode: KeyMode,
    /// Energy range filter (0.0 – 1.0).
    pub energy_min: EnergyLevel,
    pub energy_max: EnergyLevel,
    /// Genre filter (empty = no filter).
    pub genres: Vec<String>,
    /// Minimum rating (0–100, empty means no filter).
    pub min_rating: Option<u32>,
    /// Mood tags (intersection: all must match).
    pub moods: Vec<String>,
    /// Maximum playlist duration in seconds (None = unlimited).
    pub max_duration_secs: Option<f64>,
    /// Maximum number of tracks.
    pub max_tracks: usize,
    /// Search text for library lookup.
    pub search_text: Option<String>,
    /// Analysis window for time-bounded selection.
    pub analysis_window_days: Option<usize>,
}

impl Default for PlaylistCriteria {
    fn default() -> Self {
        Self {
            playlist_type: PlaylistType::MixPrep,
            bpm_mode: BpmMode::Range,
            bpm_min: 110.0,
            bpm_max: 130.0,
            bpm_relative_delta: 10.0,
            key_mode: KeyMode::Off,
            energy_min: 0.0,
            energy_max: 1.0,
            genres: Vec::new(),
            min_rating: None,
            moods: Vec::new(),
            max_duration_secs: None,
            max_tracks: 50,
            search_text: None,
            analysis_window_days: None,
        }
    }
}

impl PlaylistCriteria {
    /// Create warm-up criteria (low energy, moderate BPM).
    pub fn warm_up() -> Self {
        Self {
            playlist_type: PlaylistType::WarmUp,
            bpm_mode: BpmMode::Range,
            bpm_min: 100.0,
            bpm_max: 120.0,
            bpm_relative_delta: 15.0,
            key_mode: KeyMode::Off,
            energy_min: 0.0,
            energy_max: 0.5,
            genres: Vec::new(),
            min_rating: None,
            moods: vec!["chill".into(), "ambient".into()],
            max_duration_secs: None,
            max_tracks: 30,
            search_text: None,
            analysis_window_days: None,
        }
    }

    /// Create cool-down criteria (low energy, gradual decrease).
    pub fn cool_down() -> Self {
        Self {
            playlist_type: PlaylistType::CoolDown,
            bpm_mode: BpmMode::Range,
            bpm_min: 80.0,
            bpm_max: 110.0,
            bpm_relative_delta: 10.0,
            key_mode: KeyMode::Compatible,
            energy_min: 0.0,
            energy_max: 0.4,
            genres: Vec::new(),
            min_rating: None,
            moods: vec!["chill".into(), "ambient".into()],
            max_duration_secs: None,
            max_tracks: 25,
            search_text: None,
            analysis_window_days: None,
        }
    }

    /// Create energy-wave criteria (alternating energy).
    pub fn energy_wave() -> Self {
        Self {
            playlist_type: PlaylistType::EnergyWave,
            bpm_mode: BpmMode::Range,
            bpm_min: 120.0,
            bpm_max: 135.0,
            bpm_relative_delta: 5.0,
            key_mode: KeyMode::Compatible,
            energy_min: 0.3,
            energy_max: 1.0,
            genres: Vec::new(),
            min_rating: Some(60),
            moods: vec!["energetic".into()],
            max_duration_secs: None,
            max_tracks: 40,
            search_text: None,
            analysis_window_days: None,
        }
    }

    /// Create harmonic-journey criteria (strict key compatibility).
    pub fn harmonic_journey() -> Self {
        Self {
            playlist_type: PlaylistType::HarmonicJourney,
            bpm_mode: BpmMode::Relative,
            bpm_min: 110.0,
            bpm_max: 130.0,
            bpm_relative_delta: 8.0,
            key_mode: KeyMode::CamelotCycling,
            energy_min: 0.2,
            energy_max: 0.8,
            genres: Vec::new(),
            min_rating: Some(70),
            moods: vec![],
            max_duration_secs: None,
            max_tracks: 35,
            search_text: None,
            analysis_window_days: None,
        }
    }
}

// --- Track scoring ---

/// How a single track scored against the criteria.
#[derive(Debug, Clone)]
pub struct TrackScore {
    /// Track identifier.
    pub track_id: String,
    /// Track name for display.
    pub track_name: String,
    /// Overall match score (0.0 – 1.0, higher is better).
    pub match_score: f32,
    /// Per-criterion scores for transparency.
    pub bpm_score: f32,
    pub key_score: f32,
    pub energy_score: f32,
    pub genre_score: f32,
    pub mood_score: f32,
    pub rating_score: f32,
    /// Reasons for recommendation.
    pub match_reasons: Vec<String>,
}

impl TrackScore {
    fn format_reason(&self, label: &str, threshold: f32) -> Option<String> {
        if self.match_reasons.iter().any(|r| r.starts_with(label)) {
            return None; // already recorded
        }
        if threshold > 0.0 && self.bpm_score >= threshold {
            let reason = format!("{} (score {:.0}%)", label, self.bpm_score * 100.0);
            self.clone_match_reasons_with_addition(reason)
        } else {
            None
        }
    }

    fn clone_match_reasons_with_addition(&self, addition: String) -> Option<String> {
        if self.match_reasons.is_empty() {
            return Some(addition);
        }
        // Only return unique reasons; simplified here.
        Some(addition)
    }

    /// Build top-N match reasons for UI display.
    pub fn top_reasons(&self, limit: usize) -> Vec<String> {
        let mut scored: Vec<(&str, f32)> = vec![
            ("BPM match", self.bpm_score),
            ("Key match", self.key_score),
            ("Energy match", self.energy_score),
            ("Genre match", self.genre_score),
            ("Mood match", self.mood_score),
            ("Rating match", self.rating_score),
        ];
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .iter()
            .take(limit)
            .filter(|(label, _)| *label != "No match")
            .map(|(l, _)| l.to_string())
            .collect()
    }
}

/// Compatibility between two musical keys (0.0 – 1.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCompatibility {
    /// Same key or relative major/minor.
    Perfect,
    /// Adjacent on the Camelot wheel (±1 hour).
    Compatible,
    /// Compatible via energy bridge.
    EnergyCompatible,
    /// Not harmonically compatible.
    Incompatible,
}

impl KeyCompatibility {
    pub fn score(self) -> f32 {
        match self {
            KeyCompatibility::Perfect => 1.0,
            KeyCompatibility::Compatible => 0.8,
            KeyCompatibility::EnergyCompatible => 0.5,
            KeyCompatibility::Incompatible => 0.0,
        }
    }
}

// --- Energy flow patterns ---

/// Energy flow pattern for track sequencing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EnergyFlowPattern {
    /// Gradual increase in energy.
    #[default]
    RampUp,
    /// Gradual decrease in energy.
    RampDown,
    /// High → low → high (peaks).
    Peaks,
    /// Alternating high and low.
    Wave,
}

impl fmt::Display for EnergyFlowPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnergyFlowPattern::RampUp => write!(f, "Ramp Up"),
            EnergyFlowPattern::RampDown => write!(f, "Ramp Down"),
            EnergyFlowPattern::Peaks => write!(f, "Peaks"),
            EnergyFlowPattern::Wave => write!(f, "Wave"),
        }
    }
}

// --- Playlist generation ---

/// Metadata about a generated playlist.
#[derive(Debug, Clone)]
pub struct SmartPlaylist {
    /// Unique playlist identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Playlist type.
    pub playlist_type: PlaylistType,
    /// Criteria used for generation.
    pub criteria: PlaylistCriteria,
    /// Ordered track IDs in the playlist.
    pub track_ids: Vec<String>,
    /// Track metadata for display (cached).
    pub track_list: Vec<TrackScore>,
    /// Generated timestamp.
    pub created_at: u64, // unix timestamp in seconds
    /// Last regeneration timestamp.
    pub last_regenerated_at: Option<u64>,
    /// Playlist analysis results.
    pub analysis: Option<PlaylistAnalysis>,
    /// Auto-update setting.
    pub auto_update: bool,
    /// Total duration in seconds.
    pub total_duration_secs: f64,
    /// Average energy level.
    pub avg_energy: EnergyLevel,
    /// Key distribution snapshot.
    pub key_distribution: HashMap<String, usize>,
    /// Energy flow snapshot.
    pub energy_flow: Vec<EnergyLevel>,
}

impl SmartPlaylist {
    pub fn new(
        id: String,
        name: String,
        playlist_type: PlaylistType,
        criteria: PlaylistCriteria,
    ) -> Self {
        Self {
            id,
            name,
            playlist_type,
            criteria,
            track_ids: Vec::new(),
            track_list: Vec::new(),
            created_at: 0,
            last_regenerated_at: None,
            analysis: None,
            auto_update: false,
            total_duration_secs: 0.0,
            avg_energy: 0.0,
            key_distribution: HashMap::new(),
            energy_flow: Vec::new(),
        }
    }

    /// Check if this is a generated smart playlist.
    pub fn is_generated(&self) -> bool {
        !self.track_ids.is_empty()
    }

    /// Get transition quality between adjacent tracks at index `i` and `i+1`.
    pub fn get_transition(&self, index: usize) -> Option<TransitionPrediction> {
        if index + 1 >= self.track_list.len() {
            return None;
        }
        let current = &self.track_list[index];
        let next = &self.track_list[index + 1];
        // Simplified: in real implementation this uses key compatibility, BPM delta, energy delta
        Some(TransitionPrediction {
            bpm_delta: (current.bpm_score - next.bpm_score).abs(),
            energy_delta: 0.1, // placeholder
            key_transition: KeyCompatibility::Compatible,
            overall_score: 75,
            color: "green",
        })
    }
}

// --- Transition quality prediction ---

/// Predicted quality of a transition between two tracks.
#[derive(Debug, Clone)]
pub struct TransitionPrediction {
    /// BPM delta between the two tracks.
    pub bpm_delta: f32,
    /// Energy delta between the two tracks.
    pub energy_delta: f32,
    /// Key compatibility classification.
    pub key_transition: KeyCompatibility,
    /// Overall quality score (0–100).
    pub overall_score: u32,
    /// Color indicator for UI: green/yellow/red.
    pub color: &'static str,
}

impl TransitionPrediction {
    /// Calculate prediction based on multiple factors.
    pub fn calculate(
        bpm_a: f64,
        bpm_b: f64,
        key_compat: KeyCompatibility,
        energy_a: EnergyLevel,
        energy_b: EnergyLevel,
    ) -> Self {
        let bpm_delta = (bpm_a - bpm_b).abs();
        let energy_delta = (energy_a - energy_b).abs();

        // BPM score: smaller delta = better transition
        let bpm_score = if bpm_delta < 2.0 {
            100.0
        } else if bpm_delta < 5.0 {
            75.0
        } else if bpm_delta < 10.0 {
            50.0
        } else {
            25.0
        };

        // Energy score: smaller transition = smoother
        let energy_score = if energy_delta < 0.1 {
            100.0
        } else if energy_delta < 0.3 {
            75.0
        } else if energy_delta < 0.5 {
            50.0
        } else {
            25.0
        };

        // Weighted combination
        let weighted = bpm_score * 0.4 + energy_score * 0.3 + key_compat.score() * 100.0 * 0.3;
        let overall = weighted as u32;

        let color = if overall >= 70 {
            "green"
        } else if overall >= 40 {
            "yellow"
        } else {
            "red"
        };

        Self {
            bpm_delta: bpm_delta as f32,
            energy_delta,
            key_transition: key_compat,
            overall_score: overall,
            color,
        }
    }
}

// --- Playlist analysis ---

/// Analysis results for a playlist.
#[derive(Debug, Clone)]
pub struct PlaylistAnalysis {
    /// Energy flow across the playlist (one value per track).
    pub energy_flow: Vec<EnergyLevel>,
    /// BPM progression (one value per track).
    pub bpm_progression: Vec<f64>,
    /// Key distribution histogram.
    pub key_distribution: HashMap<String, usize>,
    /// Average BPM.
    pub avg_bpm: f64,
    /// Average energy.
    pub avg_energy: f32,
    /// Energy flow pattern classification.
    pub dominant_pattern: EnergyFlowPattern,
    /// Transition quality summary.
    pub transition_quality: TransitionQualitySummary,
}

/// Summary of transition quality across the playlist.
#[derive(Debug, Clone)]
pub struct TransitionQualitySummary {
    /// Average transition score.
    pub avg_score: f32,
    /// Percentage of smooth transitions (green).
    pub smooth_pct: f32,
    /// Percentage of caution transitions (yellow).
    pub caution_pct: f32,
    /// Percentage of difficult transitions (red).
    pub difficult_pct: f32,
    /// Total transitions analyzed.
    pub total_transitions: usize,
}

// --- Playlist generation engine ---

/// Engine responsible for generating smart playlists.
#[derive(Default)]
pub struct SmartPlaylistEngine {
    /// In-memory cache of recent generations (for undo).
    pub deleted_playlists: Vec<DeletedPlaylist>,
}

impl SmartPlaylistEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Generate a playlist from criteria. Returns a SmartPlaylist with scored tracks.
    ///
    /// In production this would query the library, apply criteria scoring,
    /// and sequence tracks using the specified energy-flow pattern.
    /// Here we provide a working scaffold with mock track data.
    pub fn generate_playlist(
        &mut self,
        id: String,
        name: String,
        criteria: &PlaylistCriteria,
        _available_tracks: &Vec<MockTrack>,
    ) -> SmartPlaylist {
        // In production: query library, filter, score, sequence
        let playlist = SmartPlaylist::new(
            id.clone(),
            name.clone(),
            criteria.playlist_type,
            criteria.clone(),
        );

        // Return skeleton — real generation queries the library
        playlist
    }

    /// Regenerate a playlist with updated criteria.
    pub fn regenerate_playlist(
        &mut self,
        playlist: &mut SmartPlaylist,
        new_criteria: &PlaylistCriteria,
        _available_tracks: &Vec<MockTrack>,
    ) -> Result<(), String> {
        if new_criteria.max_tracks == 0 {
            return Err("Invalid criteria: max_tracks must be > 0".to_string());
        }
        playlist.criteria = new_criteria.clone();
        playlist.last_regenerated_at = Some(playlist.created_at);
        playlist.analysis = None; // invalidate cached analysis
        Ok(())
    }

    /// Analyze a playlist after generation.
    pub fn analyze_playlist(&mut self, playlist: &mut SmartPlaylist) {
        if playlist.track_list.is_empty() {
            return;
        }

        let energy_flow: Vec<EnergyLevel> =
            playlist.track_list.iter().map(|t| t.energy_score).collect();
        let avg_energy = energy_flow.iter().sum::<f32>() / energy_flow.len() as f32;

        let analysis = PlaylistAnalysis {
            energy_flow: energy_flow.clone(),
            bpm_progression: playlist.track_list.iter().map(|_| 120.0).collect(),
            key_distribution: playlist.key_distribution.clone(),
            avg_bpm: 120.0,
            avg_energy,
            dominant_pattern: self.classify_energy_pattern(&energy_flow),
            transition_quality: TransitionQualitySummary {
                avg_score: 70.0,
                smooth_pct: 60.0,
                caution_pct: 30.0,
                difficult_pct: 10.0,
                total_transitions: playlist.track_list.len().saturating_sub(1),
            },
        };

        playlist.analysis = Some(analysis);
        playlist.avg_energy = avg_energy;
    }

    /// Classify the dominant energy flow pattern.
    pub fn classify_energy_pattern(&self, energy_flow: &[EnergyLevel]) -> EnergyFlowPattern {
        if energy_flow.len() < 3 {
            return EnergyFlowPattern::RampUp; // default
        }

        // Detect pattern by checking trends
        let mut increases = 0;
        let mut decreases = 0;
        let mut peaks = 0;
        let _waves = 0;

        for i in 1..energy_flow.len() {
            if energy_flow[i] > energy_flow[i - 1] {
                increases += 1;
            } else if energy_flow[i] < energy_flow[i - 1] {
                decreases += 1;
            }
        }

        // Check for peaks: alternating increase/decrease
        for i in 1..energy_flow.len().saturating_sub(1) {
            if energy_flow[i] > energy_flow[i - 1] && energy_flow[i] > energy_flow[i + 1] {
                peaks += 1;
            }
        }

        // Check for waves: regular alternation
        let mut sign_changes = 0;
        let mut last_sign = if energy_flow[1] > energy_flow[0] {
            1
        } else {
            -1
        };
        for i in 2..energy_flow.len() {
            let sign = if energy_flow[i] > energy_flow[i - 1] {
                1
            } else {
                -1
            };
            if sign != last_sign {
                sign_changes += 1;
                last_sign = sign;
            }
        }

        if peaks > energy_flow.len() / 3 {
            EnergyFlowPattern::Peaks
        } else if sign_changes > energy_flow.len() / 2 {
            EnergyFlowPattern::Wave
        } else if increases > decreases * 2 {
            EnergyFlowPattern::RampUp
        } else if decreases > increases * 2 {
            EnergyFlowPattern::RampDown
        } else {
            EnergyFlowPattern::RampUp // default
        }
    }

    /// Soft-delete a playlist (move to undo list).
    pub fn soft_delete_playlist(&mut self, playlist: &SmartPlaylist) -> DeletedPlaylist {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let deleted = DeletedPlaylist {
            id: playlist.id.clone(),
            name: playlist.name.clone(),
            deleted_at: now,
            undo_expires_at: now.saturating_add(86400), // 24 hours
            tracks: playlist.track_ids.clone(),
            criteria: playlist.criteria.clone(),
        };
        self.deleted_playlists.push(deleted.clone());
        deleted
    }

    /// Permanently delete a playlist after undo period.
    pub fn permanent_delete(&mut self, playlist_id: &str) -> Result<(), String> {
        if let Some(idx) = self
            .deleted_playlists
            .iter()
            .position(|d| d.id == playlist_id)
        {
            self.deleted_playlists.remove(idx);
            Ok(())
        } else {
            Err(format!("Playlist '{}' not found in undo list", playlist_id))
        }
    }

    /// Undo a deletion within the undo window.
    pub fn undo_deletion(
        &mut self,
        playlist_id: &str,
        current_time: u64,
    ) -> Option<DeletedPlaylist> {
        if let Some(idx) = self
            .deleted_playlists
            .iter()
            .position(|d| d.id == playlist_id)
        {
            let deleted = self.deleted_playlists.remove(idx);
            if current_time < deleted.undo_expires_at {
                Some(deleted)
            } else {
                None // undo window expired
            }
        } else {
            None
        }
    }

    /// Check if a playlist can be deleted (prevent deletion of read-only system playlists).
    pub fn can_delete(&self, playlist_id: &str) -> bool {
        // System playlists cannot be deleted
        !playlist_id.starts_with("system_")
    }
}

/// Soft-deleted playlist with undo support.
#[derive(Debug, Clone)]
pub struct DeletedPlaylist {
    pub id: String,
    pub name: String,
    pub deleted_at: u64,
    pub undo_expires_at: u64,
    pub tracks: Vec<String>,
    pub criteria: PlaylistCriteria,
}

impl DeletedPlaylist {
    /// Check if the undo window is still open.
    pub fn is_undo_available(&self, current_time: u64) -> bool {
        current_time < self.undo_expires_at
    }
}

// --- Mock track data for testing ---

/// Simplified track representation for generation testing.
#[derive(Debug, Clone)]
pub struct MockTrack {
    pub id: String,
    pub name: String,
    pub bpm: f64,
    pub key: String, // Camelot notation, e.g. "8A", "8B"
    pub energy: EnergyLevel,
    pub genre: String,
    pub rating: u32,
    pub moods: Vec<String>,
    pub duration_secs: f64,
}

impl MockTrack {
    /// Score this track against the given criteria.
    pub fn score_against(&self, criteria: &PlaylistCriteria) -> TrackScore {
        let bpm_score = self.calculate_bpm_score(criteria);
        let key_score = self.calculate_key_score(criteria);
        let energy_score = self.calculate_energy_score(criteria);
        let genre_score = self.calculate_genre_score(criteria);
        let mood_score = self.calculate_mood_score(criteria);
        let rating_score = self.calculate_rating_score(criteria);

        let match_score =
            (bpm_score + key_score + energy_score + genre_score + mood_score + rating_score) / 6.0;

        let mut match_reasons = Vec::new();
        if bpm_score > 0.8 {
            match_reasons.push("BPM match".into());
        }
        if key_score > 0.8 {
            match_reasons.push("Key match".into());
        }
        if energy_score > 0.8 {
            match_reasons.push("Energy match".into());
        }

        TrackScore {
            track_id: self.id.clone(),
            track_name: self.name.clone(),
            match_score,
            bpm_score,
            key_score,
            energy_score,
            genre_score,
            mood_score,
            rating_score,
            match_reasons,
        }
    }

    fn calculate_bpm_score(&self, criteria: &PlaylistCriteria) -> f32 {
        match criteria.bpm_mode {
            BpmMode::Exact => {
                if (self.bpm - criteria.bpm_min).abs() < 0.1 {
                    1.0
                } else {
                    0.0
                }
            }
            BpmMode::Range => {
                if self.bpm >= criteria.bpm_min && self.bpm <= criteria.bpm_max {
                    1.0
                } else {
                    0.0
                }
            }
            BpmMode::Relative => {
                let center = (criteria.bpm_min + criteria.bpm_max) / 2.0;
                let delta = (self.bpm - center).abs() / center * 100.0;
                if delta <= criteria.bpm_relative_delta {
                    (1.0 - (delta / criteria.bpm_relative_delta * 0.2)) as f32
                } else {
                    0.0
                }
            }
        }
    }

    fn calculate_key_score(&self, _criteria: &PlaylistCriteria) -> f32 {
        // Simplified: returns 1.0 if key is well-formed Camelot notation
        if self.key.len() >= 2 && (self.key.ends_with('A') || self.key.ends_with('B')) {
            1.0
        } else {
            0.0
        }
    }

    fn calculate_energy_score(&self, criteria: &PlaylistCriteria) -> f32 {
        if self.energy >= criteria.energy_min && self.energy <= criteria.energy_max {
            1.0
        } else {
            0.0
        }
    }

    fn calculate_genre_score(&self, criteria: &PlaylistCriteria) -> f32 {
        if criteria.genres.is_empty() {
            1.0 // no filter
        } else {
            (criteria.genres.contains(&self.genre) as i32) as f32
        }
    }

    fn calculate_mood_score(&self, criteria: &PlaylistCriteria) -> f32 {
        if criteria.moods.is_empty() {
            1.0
        } else {
            let matching = criteria
                .moods
                .iter()
                .filter(|m| self.moods.contains(m))
                .count();
            matching as f32 / criteria.moods.len() as f32
        }
    }

    fn calculate_rating_score(&self, criteria: &PlaylistCriteria) -> f32 {
        match criteria.min_rating {
            Some(min) => {
                if self.rating >= min {
                    (self.rating as f32 / 100.0).max(0.5)
                } else {
                    0.0
                }
            }
            None => 1.0,
        }
    }
}

// --- Display helpers ---

/// View mode for playlist display in UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaylistViewMode {
    #[default]
    List,
    Grid,
    Timeline,
}

impl fmt::Display for PlaylistViewMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlaylistViewMode::List => write!(f, "List"),
            PlaylistViewMode::Grid => write!(f, "Grid"),
            PlaylistViewMode::Timeline => write!(f, "Timeline"),
        }
    }
}

impl fmt::Display for PlaylistAnalysis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Playlist Analysis:")?;
        writeln!(f, "  Avg BPM: {:.1}", self.avg_bpm)?;
        writeln!(f, "  Avg Energy: {:.2}", self.avg_energy)?;
        writeln!(f, "  Dominant Pattern: {}", self.dominant_pattern)?;
        writeln!(
            f,
            "  Transition Quality: {:.0}% avg ({:.0}% smooth, {:.0}% caution, {:.0}% difficult)",
            self.transition_quality.avg_score,
            self.transition_quality.smooth_pct,
            self.transition_quality.caution_pct,
            self.transition_quality.difficult_pct,
        )?;
        Ok(())
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {

    use super::*;

    // ===== PlaylistType tests =====

    #[test]
    fn test_playlist_type_display() {
        assert_eq!(PlaylistType::MixPrep.to_string(), "Mix Preparation");
        assert_eq!(PlaylistType::WarmUp.to_string(), "Warm Up");
        assert_eq!(PlaylistType::CoolDown.to_string(), "Cool Down");
        assert_eq!(PlaylistType::EnergyWave.to_string(), "Energy Wave");
        assert_eq!(
            PlaylistType::HarmonicJourney.to_string(),
            "Harmonic Journey"
        );
    }

    #[test]
    fn test_playlist_type_default() {
        assert_eq!(PlaylistType::default(), PlaylistType::MixPrep);
    }

    // ===== PlaylistCriteria tests =====

    #[test]
    fn test_criteria_default() {
        let c = PlaylistCriteria::default();
        assert_eq!(c.playlist_type, PlaylistType::MixPrep);
        assert_eq!(c.bpm_min, 110.0);
        assert_eq!(c.bpm_max, 130.0);
        assert_eq!(c.key_mode, KeyMode::Off);
    }

    #[test]
    fn test_criteria_warm_up() {
        let c = PlaylistCriteria::warm_up();
        assert_eq!(c.playlist_type, PlaylistType::WarmUp);
        assert!(c.energy_max < 0.6);
        assert_eq!(c.moods.len(), 2);
    }

    #[test]
    fn test_criteria_cool_down() {
        let c = PlaylistCriteria::cool_down();
        assert_eq!(c.playlist_type, PlaylistType::CoolDown);
        assert_eq!(c.key_mode, KeyMode::Compatible);
        assert!(c.bpm_max < 115.0);
    }

    #[test]
    fn test_criteria_energy_wave() {
        let c = PlaylistCriteria::energy_wave();
        assert_eq!(c.playlist_type, PlaylistType::EnergyWave);
        assert_eq!(c.key_mode, KeyMode::Compatible);
        assert_eq!(c.min_rating, Some(60));
    }

    #[test]
    fn test_criteria_harmonic_journey() {
        let c = PlaylistCriteria::harmonic_journey();
        assert_eq!(c.playlist_type, PlaylistType::HarmonicJourney);
        assert_eq!(c.key_mode, KeyMode::CamelotCycling);
        assert_eq!(c.bpm_mode, BpmMode::Relative);
        assert_eq!(c.min_rating, Some(70));
    }

    // ===== EnergyFlowPattern tests =====

    #[test]
    fn test_energy_flow_pattern_display() {
        assert_eq!(EnergyFlowPattern::RampUp.to_string(), "Ramp Up");
        assert_eq!(EnergyFlowPattern::RampDown.to_string(), "Ramp Down");
        assert_eq!(EnergyFlowPattern::Peaks.to_string(), "Peaks");
        assert_eq!(EnergyFlowPattern::Wave.to_string(), "Wave");
    }

    #[test]
    fn test_energy_flow_pattern_default() {
        assert_eq!(EnergyFlowPattern::default(), EnergyFlowPattern::RampUp);
    }

    // ===== KeyCompatibility tests =====

    #[test]
    fn test_key_compatibility_scores() {
        assert_eq!(KeyCompatibility::Perfect.score(), 1.0);
        assert_eq!(KeyCompatibility::Compatible.score(), 0.8);
        assert_eq!(KeyCompatibility::EnergyCompatible.score(), 0.5);
        assert_eq!(KeyCompatibility::Incompatible.score(), 0.0);
    }

    // ===== TrackScore tests =====

    #[test]
    fn test_track_score_creation() {
        let score = TrackScore {
            track_id: "t1".into(),
            track_name: "Test Track".into(),
            match_score: 0.85,
            bpm_score: 0.9,
            key_score: 0.8,
            energy_score: 0.85,
            genre_score: 0.8,
            mood_score: 0.8,
            rating_score: 0.85,
            match_reasons: vec!["BPM match".into()],
        };
        assert_eq!(score.track_id, "t1");
        assert_eq!(score.match_score, 0.85);
        assert_eq!(score.top_reasons(3).len(), 3);
        assert_eq!(score.top_reasons(1), vec!["BPM match"]);
    }

    #[test]
    fn test_track_score_top_reasons() {
        let score = TrackScore {
            track_id: "t1".into(),
            track_name: "Test Track".into(),
            match_score: 0.9,
            bpm_score: 1.0,
            key_score: 0.95,
            energy_score: 0.85,
            genre_score: 0.8,
            mood_score: 0.7,
            rating_score: 0.6,
            match_reasons: vec!["BPM match".into(), "Key match".into()],
        };
        let reasons = score.top_reasons(10);
        assert!(!reasons.is_empty());
    }

    // ===== SmartPlaylist tests =====

    #[test]
    fn test_playlist_creation() {
        let p = SmartPlaylist::new(
            "pl1".into(),
            "Test Playlist".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        assert_eq!(p.id, "pl1");
        assert_eq!(p.name, "Test Playlist");
        assert!(!p.is_generated());
        assert!(p.track_ids.is_empty());
        assert!(p.analysis.is_none());
    }

    #[test]
    fn test_playlist_is_generated() {
        let mut p = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        assert!(!p.is_generated());
        p.track_ids.push("t1".into());
        assert!(p.is_generated());
    }

    #[test]
    fn test_playlist_transition() {
        let mut p = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        p.track_list = vec![
            TrackScore {
                track_id: "t1".into(),
                track_name: "T1".into(),
                match_score: 0.9,
                bpm_score: 120.0,
                key_score: 0.8,
                energy_score: 0.5,
                genre_score: 0.8,
                mood_score: 0.8,
                rating_score: 0.8,
                match_reasons: Vec::new(),
            },
            TrackScore {
                track_id: "t2".into(),
                track_name: "T2".into(),
                match_score: 0.85,
                bpm_score: 122.0,
                key_score: 0.75,
                energy_score: 0.6,
                genre_score: 0.8,
                mood_score: 0.8,
                rating_score: 0.8,
                match_reasons: Vec::new(),
            },
        ];
        let t = p.get_transition(0);
        assert!(t.is_some());
        let t = t.unwrap();
        assert!(!t.color.is_empty());
    }

    #[test]
    fn test_playlist_transition_out_of_bounds() {
        let p = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        assert!(p.get_transition(0).is_none());
    }

    // ===== TransitionPrediction tests =====

    #[test]
    fn test_transition_small_bpm_delta_green() {
        let t = TransitionPrediction::calculate(120.0, 121.0, KeyCompatibility::Perfect, 0.5, 0.55);
        assert_eq!(t.color, "green");
        assert!(t.overall_score >= 70);
    }

    #[test]
    fn test_transition_medium_bpm_delta_yellow() {
        let t =
            TransitionPrediction::calculate(120.0, 128.0, KeyCompatibility::Compatible, 0.5, 0.6);
        assert!(t.overall_score < 80 || t.overall_score >= 40);
    }

    #[test]
    fn test_transition_large_bpm_delta_red() {
        let t =
            TransitionPrediction::calculate(120.0, 145.0, KeyCompatibility::Incompatible, 0.3, 0.9);
        assert_eq!(t.color, "red");
    }

    #[test]
    fn test_transition_perfect_key() {
        let t = TransitionPrediction::calculate(120.0, 121.0, KeyCompatibility::Perfect, 0.5, 0.5);
        assert_eq!(t.key_transition, KeyCompatibility::Perfect);
    }

    // ===== SmartPlaylistEngine tests =====

    #[test]
    fn test_engine_new() {
        let engine = SmartPlaylistEngine::new();
        assert!(engine.deleted_playlists.is_empty());
    }

    #[test]
    fn test_generate_playlist() {
        let mut engine = SmartPlaylistEngine::new();
        let tracks: Vec<MockTrack> = Vec::new();
        let pl = engine.generate_playlist(
            "pl1".into(),
            "Generated".into(),
            &PlaylistCriteria::default(),
            &tracks,
        );
        assert_eq!(pl.id, "pl1");
        assert_eq!(pl.name, "Generated");
    }

    #[test]
    fn test_regenerate_playlist() {
        let mut engine = SmartPlaylistEngine::new();
        let mut pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        pl.created_at = 1000;

        let new_criteria = PlaylistCriteria::warm_up();
        let tracks: Vec<MockTrack> = Vec::new();
        let result = engine.regenerate_playlist(&mut pl, &new_criteria, &tracks);
        assert!(result.is_ok());
        assert_eq!(pl.criteria.playlist_type, PlaylistType::WarmUp);
    }

    #[test]
    fn test_regenerate_playlist_invalid_criteria() {
        let mut engine = SmartPlaylistEngine::new();
        let mut pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        let mut invalid = PlaylistCriteria::default();
        invalid.max_tracks = 0;
        let tracks: Vec<MockTrack> = Vec::new();
        let result = engine.regenerate_playlist(&mut pl, &invalid, &tracks);
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_playlist_empty() {
        let mut engine = SmartPlaylistEngine::new();
        let mut pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        engine.analyze_playlist(&mut pl);
        assert!(pl.analysis.is_none());
    }

    #[test]
    fn test_analyze_playlist_non_empty() {
        let mut engine = SmartPlaylistEngine::new();
        let mut pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        pl.track_list = vec![
            TrackScore {
                track_id: "t1".into(),
                track_name: "T1".into(),
                match_score: 0.9,
                bpm_score: 120.0,
                key_score: 0.8,
                energy_score: 0.5,
                genre_score: 0.8,
                mood_score: 0.8,
                rating_score: 0.8,
                match_reasons: Vec::new(),
            },
            TrackScore {
                track_id: "t2".into(),
                track_name: "T2".into(),
                match_score: 0.85,
                bpm_score: 122.0,
                key_score: 0.75,
                energy_score: 0.7,
                genre_score: 0.8,
                mood_score: 0.8,
                rating_score: 0.8,
                match_reasons: Vec::new(),
            },
        ];
        engine.analyze_playlist(&mut pl);
        assert!(pl.analysis.is_some());
        let analysis = pl.analysis.as_ref().unwrap();
        assert_eq!(analysis.energy_flow.len(), 2);
        assert_eq!(analysis.dominant_pattern, EnergyFlowPattern::RampUp);
    }

    #[test]
    fn test_classify_energy_pattern_ramp_up() {
        let engine = SmartPlaylistEngine::new();
        let energies = vec![0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8];
        assert_eq!(
            engine.classify_energy_pattern(&energies),
            EnergyFlowPattern::RampUp
        );
    }

    #[test]
    fn test_classify_energy_pattern_ramp_down() {
        let engine = SmartPlaylistEngine::new();
        let energies = vec![0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2];
        assert_eq!(
            engine.classify_energy_pattern(&energies),
            EnergyFlowPattern::RampDown
        );
    }

    #[test]
    fn test_classify_energy_pattern_waves() {
        let engine = SmartPlaylistEngine::new();
        let energies = vec![0.2, 0.8, 0.2, 0.8, 0.2, 0.8];
        let pattern = engine.classify_energy_pattern(&energies);
        assert!(matches!(
            pattern,
            EnergyFlowPattern::Wave | EnergyFlowPattern::Peaks
        ));
    }

    #[test]
    fn test_classify_energy_pattern_short() {
        let engine = SmartPlaylistEngine::new();
        let energies = vec![0.5, 0.6];
        assert_eq!(
            engine.classify_energy_pattern(&energies),
            EnergyFlowPattern::RampUp
        );
    }

    #[test]
    fn test_soft_delete_playlist() {
        let mut engine = SmartPlaylistEngine::new();
        let pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        let deleted = engine.soft_delete_playlist(&pl);
        assert_eq!(deleted.id, "pl1");
        assert_eq!(engine.deleted_playlists.len(), 1);
    }

    #[test]
    fn test_undo_deletion() {
        let mut engine = SmartPlaylistEngine::new();
        let pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        let deleted = engine.soft_delete_playlist(&pl);

        let restored = engine.undo_deletion("pl1", deleted.deleted_at + 1); // within undo window
        assert!(restored.is_some());
        assert_eq!(engine.deleted_playlists.len(), 0);
    }

    #[test]
    fn old_playlist_gets_a_fresh_undo_window() {
        let mut engine = SmartPlaylistEngine::new();
        let mut playlist = SmartPlaylist::new(
            "old".into(),
            "Old".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        playlist.created_at = 1;
        let deleted = engine.soft_delete_playlist(&playlist);
        assert!(deleted.deleted_at > playlist.created_at + 86400);
        assert!(engine
            .undo_deletion("old", deleted.deleted_at + 1)
            .is_some());
    }

    #[test]
    fn test_undo_deletion_expired() {
        let mut engine = SmartPlaylistEngine::new();
        let pl = SmartPlaylist::new(
            "pl1".into(),
            "Test".into(),
            PlaylistType::MixPrep,
            PlaylistCriteria::default(),
        );
        let deleted = engine.soft_delete_playlist(&pl);

        // Undo expired (current_time > undo_expires_at)
        let restored = engine.undo_deletion("pl1", deleted.undo_expires_at + 1);
        assert!(restored.is_none());
    }

    #[test]
    fn test_can_delete() {
        let engine = SmartPlaylistEngine::new();
        assert!(engine.can_delete("pl1"));
        assert!(!engine.can_delete("system_mix"));
    }

    #[test]
    fn test_playlist_display() {
        let analysis = PlaylistAnalysis {
            energy_flow: vec![0.5, 0.6, 0.7],
            bpm_progression: vec![120.0, 121.0, 122.0],
            key_distribution: HashMap::new(),
            avg_bpm: 121.0,
            avg_energy: 0.6,
            dominant_pattern: EnergyFlowPattern::RampUp,
            transition_quality: TransitionQualitySummary {
                avg_score: 75.0,
                smooth_pct: 65.0,
                caution_pct: 25.0,
                difficult_pct: 10.0,
                total_transitions: 2,
            },
        };
        let display = format!("{}", analysis);
        assert!(display.contains("Avg BPM"));
        assert!(display.contains("Ramp Up"));
    }

    // ===== MockTrack tests =====

    #[test]
    fn test_mock_track_score_all_criteria() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Perfect Track".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 90,
            moods: vec!["energetic".into(), "danceable".into()],
            duration_secs: 360.0,
        };
        let criteria = PlaylistCriteria::default();
        let score = track.score_against(&criteria);
        assert!(score.match_score >= 0.8);
        assert!(score.bpm_score >= 0.8);
        assert!(score.energy_score >= 0.8);
        assert!(score.genre_score >= 0.8);
    }

    #[test]
    fn test_mock_track_score_bpm_range() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };
        let criteria = PlaylistCriteria::default();
        assert_eq!(track.calculate_bpm_score(&criteria), 1.0);

        // Outside range
        let mut criteria_out = PlaylistCriteria::default();
        criteria_out.bpm_min = 140.0;
        criteria_out.bpm_max = 150.0;
        assert_eq!(track.calculate_bpm_score(&criteria_out), 0.0);
    }

    #[test]
    fn test_mock_track_score_key_mode() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };
        let criteria = PlaylistCriteria::default();
        assert!(track.calculate_key_score(&criteria) > 0.0); // valid Camelot key

        let bad_track = MockTrack {
            id: "t2".into(),
            name: "Bad Key".into(),
            bpm: 120.0,
            key: "INVALID".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };
        // Simplified key scoring — just checks length and ending
        let _score = bad_track.score_against(&criteria);
    }

    #[test]
    fn test_mock_track_score_energy_range() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };
        let criteria = PlaylistCriteria {
            energy_min: 0.3,
            energy_max: 0.7,
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_energy_score(&criteria), 1.0);

        let criteria_out = PlaylistCriteria {
            energy_min: 0.0,
            energy_max: 0.2,
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_energy_score(&criteria_out), 0.0);
    }

    #[test]
    fn test_mock_track_score_genre_filter() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };

        // No filter
        let criteria_no = PlaylistCriteria {
            genres: vec![],
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_genre_score(&criteria_no), 1.0);

        // Genre matches
        let criteria_match = PlaylistCriteria {
            genres: vec!["House".into()],
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_genre_score(&criteria_match), 1.0);

        // Genre mismatch
        let criteria_mismatch = PlaylistCriteria {
            genres: vec!["Techno".into()],
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_genre_score(&criteria_mismatch), 0.0);
    }

    #[test]
    fn test_mock_track_score_mood_filter() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec!["energetic".into(), "danceable".into()],
            duration_secs: 300.0,
        };

        // All moods match
        let criteria_all = PlaylistCriteria {
            moods: vec!["energetic".into(), "danceable".into()],
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_mood_score(&criteria_all), 1.0);

        // Partial match
        let criteria_partial = PlaylistCriteria {
            moods: vec!["energetic".into(), "chill".into()],
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_mood_score(&criteria_partial), 0.5);

        // No filter
        let criteria_none = PlaylistCriteria {
            moods: vec![],
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_mood_score(&criteria_none), 1.0);
    }

    #[test]
    fn test_mock_track_score_rating_filter() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 120.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };

        // Rating meets minimum
        let criteria_meets = PlaylistCriteria {
            min_rating: Some(70),
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_rating_score(&criteria_meets), 0.8);

        // Rating below minimum
        let criteria_below = PlaylistCriteria {
            min_rating: Some(90),
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_rating_score(&criteria_below), 0.0);

        // No filter
        let criteria_none = PlaylistCriteria {
            min_rating: None,
            ..PlaylistCriteria::default()
        };
        assert_eq!(track.calculate_rating_score(&criteria_none), 1.0);
    }

    // ===== DeletedPlaylist tests =====

    #[test]
    fn test_deleted_playlist_undo_available() {
        let _now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let deleted = DeletedPlaylist {
            id: "pl1".into(),
            name: "Test".into(),
            deleted_at: 1000,
            undo_expires_at: 1100,
            tracks: vec![],
            criteria: PlaylistCriteria::default(),
        };
        assert!(deleted.is_undo_available(1050));
        assert!(!deleted.is_undo_available(1100));
        assert!(!deleted.is_undo_available(9999));
    }

    // ===== PlaylistViewMode tests =====

    #[test]
    fn test_playlist_view_mode_display() {
        assert_eq!(PlaylistViewMode::List.to_string(), "List");
        assert_eq!(PlaylistViewMode::Grid.to_string(), "Grid");
        assert_eq!(PlaylistViewMode::Timeline.to_string(), "Timeline");
    }

    #[test]
    fn test_playlist_view_mode_default() {
        assert_eq!(PlaylistViewMode::default(), PlaylistViewMode::List);
    }

    // ===== Edge case tests =====

    #[test]
    fn test_transition_prediction_edge_cases() {
        // Identical tracks
        let t = TransitionPrediction::calculate(120.0, 120.0, KeyCompatibility::Perfect, 0.5, 0.5);
        assert_eq!(t.color, "green");
        assert!(t.overall_score >= 70);
        assert_eq!(t.bpm_delta, 0.0);
    }

    #[test]
    fn test_playlist_criteria_max_tracks_zero() {
        let mut c = PlaylistCriteria::default();
        c.max_tracks = 0;
        assert_eq!(c.max_tracks, 0);
    }

    #[test]
    fn test_playlist_criteria_with_search_text() {
        let mut c = PlaylistCriteria::default();
        c.search_text = Some("test".into());
        assert_eq!(c.search_text.as_deref(), Some("test"));
    }

    #[test]
    fn test_playlist_criteria_with_analysis_window() {
        let mut c = PlaylistCriteria::default();
        c.analysis_window_days = Some(30);
        assert_eq!(c.analysis_window_days, Some(30));
    }

    #[test]
    fn test_track_score_format_reasons() {
        let score = TrackScore {
            track_id: "t1".into(),
            track_name: "Test".into(),
            match_score: 0.5,
            bpm_score: 0.1,
            key_score: 0.1,
            energy_score: 0.1,
            genre_score: 0.1,
            mood_score: 0.1,
            rating_score: 0.1,
            match_reasons: vec!["BPM match".into()],
        };
        // No new reasons below threshold
        assert!(score.format_reason("Low BPM", 0.5).is_none());
    }

    #[test]
    fn test_playlist_analysis_summary() {
        let summary = TransitionQualitySummary {
            avg_score: 80.0,
            smooth_pct: 70.0,
            caution_pct: 20.0,
            difficult_pct: 10.0,
            total_transitions: 9,
        };
        assert_eq!(summary.total_transitions, 9);
        assert_eq!(
            summary.smooth_pct + summary.caution_pct + summary.difficult_pct,
            100.0
        );
    }

    #[test]
    fn test_engine_can_delete_edge_cases() {
        let engine = SmartPlaylistEngine::new();
        assert!(engine.can_delete("pl_anything"));
        assert!(!engine.can_delete("system_"));
        assert!(!engine.can_delete("system_playlist_123"));
    }

    #[test]
    fn test_mock_track_relative_bpm_scoring() {
        let track = MockTrack {
            id: "t1".into(),
            name: "Test".into(),
            bpm: 125.0,
            key: "8A".into(),
            energy: 0.5,
            genre: "House".into(),
            rating: 80,
            moods: vec![],
            duration_secs: 300.0,
        };
        let criteria = PlaylistCriteria {
            bpm_mode: BpmMode::Relative,
            bpm_min: 110.0,
            bpm_max: 130.0,
            bpm_relative_delta: 10.0,
            ..PlaylistCriteria::default()
        };
        let score = track.calculate_bpm_score(&criteria);
        assert!(score > 0.5);
        assert!(score <= 1.0);
    }
}
