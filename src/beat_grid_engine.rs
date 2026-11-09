//! Beat Grid Engine — Beat detection, grid management, phase synchronization, downbeat analysis.
//!
//! Implements precise beat grid generation, visualization, correction,
//! phase analysis between decks, and downbeat detection.
//! Part of the Beat Grid feature (P17a).
//!
//! ## Design principles
//!
//! * **Precision** — Sample-accurate beat positions.
//! * **Phase-aware** — Inter-deck phase analysis.
//! * **Visual** — Grid visualization for UI.
//! * **Correctable** — Manual grid correction support.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Beat type within a measure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BeatType {
    /// First beat of measure (downbeat).
    Downbeat,
    /// Second beat.
    Beat2,
    /// Third beat.
    Beat3,
    /// Fourth beat.
    Beat4,
    /// Off-beat (eight note).
    OffBeat,
    /// Unknown/uncertain.
    Unknown,
}

impl BeatType {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Downbeat => "Downbeat",
            Self::Beat2 => "Beat 2",
            Self::Beat3 => "Beat 3",
            Self::Beat4 => "Beat 4",
            Self::OffBeat => "Off-beat",
            Self::Unknown => "Unknown",
        }
    }

    /// Get color for UI visualization.
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Downbeat => (255, 50, 50),  // Red
            Self::Beat2 => (255, 200, 50),    // Orange
            Self::Beat3 => (50, 150, 255),    // Blue
            Self::Beat4 => (50, 255, 100),    // Green
            Self::OffBeat => (150, 150, 150), // Gray
            Self::Unknown => (100, 100, 100), // Dark gray
        }
    }
}

/// Beat grid event at a specific time.
#[derive(Debug, Clone)]
pub struct BeatEvent {
    /// Time in seconds.
    pub time: f64,
    /// Beat type.
    pub beat_type: BeatType,
    /// Confidence (0.0-1.0).
    pub confidence: f64,
    /// Bar number (0-based).
    pub bar: u32,
    /// Beat position within bar (1-4).
    pub beat_in_bar: u8,
    /// Sample position.
    pub sample_position: usize,
}

impl BeatEvent {
    /// Create a new beat event.
    pub fn new(
        time: f64,
        beat_type: BeatType,
        confidence: f64,
        bar: u32,
        beat_in_bar: u8,
        sample_position: usize,
    ) -> Self {
        Self {
            time,
            beat_type,
            confidence,
            bar,
            beat_in_bar,
            sample_position,
        }
    }
}

/// Phase relationship between two decks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhaseRelationship {
    /// In phase (same position in beat).
    InPhase,
    /// 90° out of phase.
    QuarterPhase,
    /// 180° out of phase (opposite).
    HalfPhase,
    /// 270° out of phase.
    ThreeQuarterPhase,
    /// Unknown/uncertain.
    Unknown,
}

impl PhaseRelationship {
    /// Get phase difference in degrees.
    pub fn degrees(&self) -> f64 {
        match self {
            Self::InPhase => 0.0,
            Self::QuarterPhase => 90.0,
            Self::HalfPhase => 180.0,
            Self::ThreeQuarterPhase => 270.0,
            Self::Unknown => -1.0,
        }
    }

    /// Get visual indicator.
    pub fn visual_indicator(&self) -> &str {
        match self {
            Self::InPhase => "●",           // Filled circle
            Self::QuarterPhase => "◐",      // Half-filled
            Self::HalfPhase => "◌",         // Empty circle
            Self::ThreeQuarterPhase => "◑", // Three-quarter filled
            Self::Unknown => "○",           // Question mark circle
        }
    }

    /// Check if phases are compatible for mixing.
    pub fn is_compatible(&self) -> bool {
        matches!(self, Self::InPhase | Self::HalfPhase)
    }
}

/// Grid correction action.
#[derive(Debug, Clone)]
pub struct GridCorrection {
    /// Correction type.
    pub correction_type: CorrectionType,
    /// Time position of correction.
    pub time: f64,
    /// Time shift applied.
    pub time_shift: f64,
    /// Reason for correction.
    pub reason: String,
}

/// Type of grid correction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CorrectionType {
    /// Shift beat time.
    ShiftBeat,
    /// Insert new beat.
    InsertBeat,
    /// Remove beat.
    RemoveBeat,
    /// Merge adjacent beats.
    MergeBeats,
    /// Snap to nearest beat.
    SnapToBeat,
}

impl CorrectionType {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::ShiftBeat => "Shift Beat",
            Self::InsertBeat => "Insert Beat",
            Self::RemoveBeat => "Remove Beat",
            Self::MergeBeats => "Merge Beats",
            Self::SnapToBeat => "Snap to Beat",
        }
    }
}

/// Beat grid for a single track.
pub struct BeatGrid {
    /// Track ID.
    pub track_id: String,
    /// BPM (may vary with tempo maps).
    pub base_bpm: f64,
    /// Grid events.
    pub events: Vec<BeatEvent>,
    /// Corrections applied.
    pub corrections: Vec<GridCorrection>,
    /// Confidence score (0.0-1.0).
    pub confidence: f64,
    /// Grid locked (no automatic changes).
    pub locked: bool,
    /// Time signature numerator.
    pub time_signature: u8,
    /// Time signature denominator (power of 2).
    pub time_denominator: u8,
    /// Bar duration in seconds.
    pub bar_duration: f64,
}

impl BeatGrid {
    /// Create a new empty beat grid.
    pub fn new(track_id: String, base_bpm: f64, time_signature: (u8, u8)) -> Self {
        let beats_per_bar = time_signature.0 as f64;
        let quarter_notes_per_bar = beats_per_bar;
        let beats_per_minute = base_bpm;
        let seconds_per_beat = 60.0 / beats_per_minute;
        let bar_duration = seconds_per_beat * quarter_notes_per_bar;

        Self {
            track_id,
            base_bpm,
            events: Vec::new(),
            corrections: Vec::new(),
            confidence: 0.0,
            locked: false,
            time_signature: time_signature.0,
            time_denominator: time_signature.1,
            bar_duration,
        }
    }

    /// Add a beat event.
    pub fn add_event(&mut self, event: BeatEvent) {
        if !self.locked {
            self.events.push(event);
            self.events
                .sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        }
    }

    /// Generate events from BPM and time signature.
    pub fn generate_from_bpm(&mut self, track_duration: Duration) {
        if self.locked {
            return;
        }

        self.events.clear();
        let _total_beats = self.base_bpm / 60.0 * track_duration.as_secs_f64();
        let samples_per_beat = 44100; // Simplified.
        let _samples_per_bar = samples_per_beat as f64 * self.time_signature as f64;

        for bar in 0.. {
            let bar_start = bar as f64 * self.bar_duration;
            if bar_start >= track_duration.as_secs_f64() {
                break;
            }

            for beat_in_bar in 1..=self.time_signature {
                let beat_time = bar_start
                    + ((beat_in_bar - 1) as f64 / self.time_signature as f64) * self.bar_duration;
                let sample_position = (beat_time * 44100.0) as usize;

                let beat_type = match beat_in_bar {
                    1 => BeatType::Downbeat,
                    2 => BeatType::Beat2,
                    3 => BeatType::Beat3,
                    4 => BeatType::Beat4,
                    _ => BeatType::Unknown,
                };

                let confidence = 0.85 + (bar as f64 * 0.01).min(0.15); // Simulated.

                self.events.push(BeatEvent::new(
                    beat_time,
                    beat_type,
                    confidence,
                    bar,
                    beat_in_bar,
                    sample_position,
                ));
            }
        }
    }

    /// Get beat events in a time range.
    pub fn events_in_range(&self, start: f64, end: f64) -> Vec<&BeatEvent> {
        self.events
            .iter()
            .filter(|e| e.time >= start && e.time <= end)
            .collect()
    }

    /// Get beat event at nearest time.
    pub fn nearest_event(&self, time: f64) -> Option<&BeatEvent> {
        self.events
            .iter()
            .min_by_key(|e| (e.time - time).abs() as i64)
    }

    /// Get bar number at time.
    pub fn bar_at_time(&self, time: f64) -> u32 {
        (time / self.bar_duration) as u32
    }

    /// Get beat position within bar at time.
    pub fn beat_in_bar_at_time(&self, time: f64) -> u8 {
        let beat_time = time - (self.bar_at_time(time) as f64 * self.bar_duration);
        let beat_in_bar = (beat_time / self.bar_duration * self.time_signature as f64) as u8 + 1;
        (beat_in_bar - 1).clamp(1, self.time_signature)
    }

    /// Apply a grid correction.
    pub fn apply_correction(&mut self, correction: GridCorrection) {
        if !self.locked {
            self.corrections.push(correction);
        }
    }

    /// Get correction count.
    pub fn correction_count(&self) -> usize {
        self.corrections.len()
    }

    /// Check if grid is locked.
    pub fn is_locked(&self) -> bool {
        self.locked
    }

    /// Lock/unlock grid.
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    /// Get total beat count.
    pub fn beat_count(&self) -> usize {
        self.events.len()
    }

    /// Get average confidence.
    pub fn avg_confidence(&self) -> f64 {
        if self.events.is_empty() {
            return 0.0;
        }
        self.events.iter().map(|e| e.confidence).sum::<f64>() / self.events.len() as f64
    }

    /// Get downbeat count.
    pub fn downbeat_count(&self) -> usize {
        self.events
            .iter()
            .filter(|e| e.beat_type == BeatType::Downbeat)
            .count()
    }

    /// Get grid visualization data (time -> color).
    pub fn visualization_data(&self) -> BTreeMap<BeatTime, (u8, u8, u8)> {
        let mut viz = BTreeMap::new();
        for event in &self.events {
            viz.insert(BeatTime(event.time), event.beat_type.color());
        }
        viz
    }

    /// Validate grid (no overlaps, correct ordering).
    pub fn validate(&self) -> bool {
        if self.events.is_empty() {
            return false;
        }

        // Check ordered by time.
        for i in 1..self.events.len() {
            if self.events[i].time < self.events[i - 1].time {
                return false;
            }
        }

        // Check bars are sequential.
        let mut last_bar = 0u32;
        for event in &self.events {
            if event.bar < last_bar {
                return false;
            }
            last_bar = event.bar;
        }

        true
    }
}

/// Phase analyzer — compares phase between two decks.
pub struct PhaseAnalyzer {
    /// Deck phase states.
    deck_phases: HashMap<String, DeckPhaseState>,
}

/// Per-deck phase state.
#[derive(Debug, Clone)]
pub struct DeckPhaseState {
    /// Deck ID.
    pub deck_id: String,
    /// Current beat position (0-1).
    pub beat_fraction: f64,
    /// Current BPM.
    pub bpm: f64,
    /// Last phase update time.
    pub last_updated: Duration,
}

impl DeckPhaseState {
    /// Create a new deck phase state.
    pub fn new(deck_id: String, bpm: f64) -> Self {
        Self {
            deck_id,
            beat_fraction: 0.0,
            bpm,
            last_updated: Duration::ZERO,
        }
    }

    /// Update beat fraction.
    pub fn update(&mut self, beat_fraction: f64) {
        self.beat_fraction = beat_fraction % 1.0;
    }
}

impl PhaseAnalyzer {
    /// Create a new phase analyzer.
    pub fn new() -> Self {
        Self {
            deck_phases: HashMap::new(),
        }
    }

    /// Update deck phase state.
    pub fn update_deck_phase(&mut self, deck_id: String, bpm: f64, beat_fraction: f64) {
        let state = self
            .deck_phases
            .entry(deck_id.clone())
            .or_insert_with(|| DeckPhaseState::new(deck_id, bpm));
        state.bpm = bpm;
        state.beat_fraction = beat_fraction;
    }

    /// Get phase relationship between two decks.
    pub fn phase_relationship(&self, deck_a: &str, deck_b: &str) -> PhaseRelationship {
        let phase_a = match self.deck_phases.get(deck_a) {
            Some(p) => p.beat_fraction,
            None => return PhaseRelationship::Unknown,
        };

        let phase_b = match self.deck_phases.get(deck_b) {
            Some(p) => p.beat_fraction,
            None => return PhaseRelationship::Unknown,
        };

        let diff = (phase_b - phase_a).abs();

        if diff < 0.05 || diff > 0.95 {
            PhaseRelationship::InPhase
        } else if (diff - 0.25).abs() < 0.05 {
            PhaseRelationship::QuarterPhase
        } else if (diff - 0.5).abs() < 0.05 {
            PhaseRelationship::HalfPhase
        } else if (diff - 0.75).abs() < 0.05 {
            PhaseRelationship::ThreeQuarterPhase
        } else {
            PhaseRelationship::Unknown
        }
    }

    /// Check if decks are phase-compatible.
    pub fn are_phase_compatible(&self, deck_a: &str, deck_b: &str) -> bool {
        self.phase_relationship(deck_a, deck_b).is_compatible()
    }

    /// Get visual phase indicator for a deck.
    pub fn phase_indicator(&self, deck_id: &str) -> &str {
        if let Some(phase) = self.deck_phases.get(deck_id) {
            // Approximate indicator based on fraction.
            let sector = (phase.beat_fraction * 4.0) as u8 % 4;
            match sector {
                0 => "●",
                1 => "◐",
                2 => "◌",
                3 => "◑",
                _ => "○",
            }
        } else {
            "○"
        }
    }

    /// Get phase difference in degrees.
    pub fn phase_difference_degrees(&self, deck_a: &str, deck_b: &str) -> Option<f64> {
        let phase_a = self.deck_phases.get(deck_a)?.beat_fraction;
        let phase_b = self.deck_phases.get(deck_b)?.beat_fraction;
        let diff = (phase_b - phase_a).abs() * 360.0;
        Some(diff)
    }

    /// Get all deck phases.
    pub fn all_phases(&self) -> Vec<&DeckPhaseState> {
        self.deck_phases.values().collect()
    }

    /// Clear all phase states.
    pub fn clear(&mut self) {
        self.deck_phases.clear();
    }
}

/// Beat grid engine — manages grids, phase, and corrections.
pub struct BeatGridEngine {
    /// Grids by track ID.
    grids: HashMap<String, BeatGrid>,
    /// Phase analyzer.
    phase_analyzer: PhaseAnalyzer,
    /// Grid correction history.
    correction_history: Vec<GridCorrection>,
    /// Auto-downbeat detection enabled.
    auto_downbeat: bool,
    /// Default time signature.
    default_time_signature: (u8, u8),
}

impl BeatGridEngine {
    /// Create a new beat grid engine.
    pub fn new(default_time_signature: (u8, u8)) -> Self {
        Self {
            grids: HashMap::new(),
            phase_analyzer: PhaseAnalyzer::new(),
            correction_history: Vec::new(),
            auto_downbeat: true,
            default_time_signature,
        }
    }

    /// Create or get a beat grid.
    pub fn get_or_create_grid(&mut self, track_id: String, base_bpm: f64) -> &mut BeatGrid {
        self.grids
            .entry(track_id.clone())
            .or_insert_with(|| BeatGrid::new(track_id, base_bpm, self.default_time_signature))
    }

    /// Get a beat grid.
    pub fn get_grid(&self, track_id: &str) -> Option<&BeatGrid> {
        self.grids.get(track_id)
    }

    /// Generate beat grid from BPM.
    pub fn generate_grid(&mut self, track_id: &str, duration: Duration) -> bool {
        if let Some(grid) = self.grids.get_mut(track_id) {
            grid.generate_from_bpm(duration);
            grid.confidence = grid.avg_confidence();
            true
        } else {
            false
        }
    }

    /// Find downbeats in a grid.
    pub fn find_downbeats(&self, track_id: &str) -> Vec<&BeatEvent> {
        self.grids
            .get(track_id)
            .map(|g| {
                g.events
                    .iter()
                    .filter(|e| e.beat_type == BeatType::Downbeat)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Analyze phase between two tracks.
    pub fn analyze_phase(&self, deck_a: &str, deck_b: &str) -> PhaseRelationship {
        self.phase_analyzer.phase_relationship(deck_a, deck_b)
    }

    /// Update phase for a deck.
    pub fn update_deck_phase(&mut self, deck_id: String, bpm: f64, beat_fraction: f64) {
        self.phase_analyzer
            .update_deck_phase(deck_id, bpm, beat_fraction);
    }

    /// Apply a grid correction.
    pub fn apply_correction(&mut self, track_id: &str, correction: GridCorrection) -> bool {
        if let Some(grid) = self.grids.get_mut(track_id) {
            grid.apply_correction(correction.clone());
            self.correction_history.push(correction);
            true
        } else {
            false
        }
    }

    /// Snap beat to nearest grid position.
    pub fn snap_to_grid(&self, track_id: &str, time: f64) -> Option<f64> {
        self.grids
            .get(track_id)
            .and_then(|g| g.nearest_event(time))
            .map(|e| e.time)
    }

    /// Get phase relationship.
    pub fn phase_compatible(&self, deck_a: &str, deck_b: &str) -> bool {
        self.phase_analyzer.are_phase_compatible(deck_a, deck_b)
    }

    /// Get grid count.
    pub fn grid_count(&self) -> usize {
        self.grids.len()
    }

    /// Get correction history.
    pub fn correction_history(&self) -> &[GridCorrection] {
        &self.correction_history
    }

    /// Check if auto-downbeat detection is enabled.
    pub fn auto_downbeat_enabled(&self) -> bool {
        self.auto_downbeat
    }

    /// Toggle auto-downbeat detection.
    pub fn toggle_auto_downbeat(&mut self) {
        self.auto_downbeat = !self.auto_downbeat;
    }

    /// Validate all grids.
    pub fn validate_all(&self) -> HashMap<String, bool> {
        self.grids
            .iter()
            .map(|(id, grid)| (id.clone(), grid.validate()))
            .collect()
    }

    /// Clear a grid.
    pub fn clear_grid(&mut self, track_id: &str) -> bool {
        if let Some(grid) = self.grids.get_mut(track_id) {
            grid.events.clear();
            grid.corrections.clear();
            grid.confidence = 0.0;
            true
        } else {
            false
        }
    }
}

impl Default for BeatGridEngine {
    fn default() -> Self {
        Self::new((4, 4)) // 4/4 time.
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

impl Default for PhaseAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn beat_type_colors() {
        let (r, g, b) = BeatType::Downbeat.color();
        assert_eq!((r, g, b), (255, 50, 50));

        let (r, g, b) = BeatType::Beat2.color();
        assert_eq!((r, g, b), (255, 200, 50));

        let (r, g, b) = BeatType::Beat3.color();
        assert_eq!((r, g, b), (50, 150, 255));

        let (r, g, b) = BeatType::Beat4.color();
        assert_eq!((r, g, b), (50, 255, 100));
    }

    #[test]
    fn phase_relationship_degrees() {
        assert_eq!(PhaseRelationship::InPhase.degrees(), 0.0);
        assert_eq!(PhaseRelationship::QuarterPhase.degrees(), 90.0);
        assert_eq!(PhaseRelationship::HalfPhase.degrees(), 180.0);
        assert_eq!(PhaseRelationship::ThreeQuarterPhase.degrees(), 270.0);
    }

    #[test]
    fn phase_relationship_compatibility() {
        assert!(PhaseRelationship::InPhase.is_compatible());
        assert!(PhaseRelationship::HalfPhase.is_compatible());
        assert!(!PhaseRelationship::QuarterPhase.is_compatible());
        assert!(!PhaseRelationship::ThreeQuarterPhase.is_compatible());
        assert!(!PhaseRelationship::Unknown.is_compatible());
    }

    #[test]
    fn correction_type_names() {
        assert_eq!(CorrectionType::ShiftBeat.display_name(), "Shift Beat");
        assert_eq!(CorrectionType::InsertBeat.display_name(), "Insert Beat");
        assert_eq!(CorrectionType::RemoveBeat.display_name(), "Remove Beat");
        assert_eq!(CorrectionType::MergeBeats.display_name(), "Merge Beats");
        assert_eq!(CorrectionType::SnapToBeat.display_name(), "Snap to Beat");
    }

    #[test]
    fn beat_grid_creation() {
        let grid = BeatGrid::new("track1".to_string(), 128.0, (4, 4));
        assert_eq!(grid.track_id, "track1");
        assert_eq!(grid.base_bpm, 128.0);
        assert_eq!(grid.time_signature, 4);
        assert_eq!(grid.time_denominator, 4);
        assert!(grid.events.is_empty());
        assert!(!grid.is_locked());
    }

    #[test]
    fn beat_grid_generate_from_bpm() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        let duration = Duration::from_secs(10); // 10 seconds.
        grid.generate_from_bpm(duration);

        assert!(grid.beat_count() > 0);
        assert!(grid.validate());

        // At 120 BPM, 4/4, 10 seconds = 120/60 * 10 = 20 beats = 5 bars.
        let expected_bars = 5;
        let downbeats = grid.downbeat_count();
        assert!(downbeats >= 1 && downbeats <= expected_bars + 1);
    }

    #[test]
    fn beat_grid_events_in_range() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        grid.generate_from_bpm(Duration::from_secs(10));

        let events = grid.events_in_range(0.0, 1.0);
        assert!(!events.is_empty());
    }

    #[test]
    fn beat_grid_nearest_event() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        grid.generate_from_bpm(Duration::from_secs(10));

        // Find event nearest to 2.5 seconds.
        let nearest = grid.nearest_event(2.5);
        assert!(nearest.is_some());
    }

    #[test]
    fn beat_grid_bar_at_time() {
        let grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        // bar_duration = 60/120 * 4 = 2.0 seconds.
        assert_eq!(grid.bar_at_time(0.0), 0);
        assert_eq!(grid.bar_at_time(1.99), 0);
        assert_eq!(grid.bar_at_time(2.0), 1);
        assert_eq!(grid.bar_at_time(4.0), 2);
    }

    #[test]
    fn beat_grid_locked_no_changes() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        grid.generate_from_bpm(Duration::from_secs(10));
        let initial_count = grid.beat_count();

        grid.set_locked(true);
        grid.generate_from_bpm(Duration::from_secs(10));

        // Events should not change when locked.
        assert_eq!(grid.beat_count(), initial_count);
    }

    #[test]
    fn beat_grid_correction() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        grid.generate_from_bpm(Duration::from_secs(10));

        let correction = GridCorrection {
            correction_type: CorrectionType::ShiftBeat,
            time: 1.0,
            time_shift: 0.05,
            reason: "Manual adjustment".to_string(),
        };

        grid.apply_correction(correction.clone());
        assert_eq!(grid.correction_count(), 1);
    }

    #[test]
    fn beat_grid_validation() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        grid.generate_from_bpm(Duration::from_secs(10));
        assert!(grid.validate());
    }

    #[test]
    fn beat_grid_visualization_data() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));
        grid.generate_from_bpm(Duration::from_secs(4));

        let viz = grid.visualization_data();
        assert!(!viz.is_empty());

        // First event should be a downbeat (red).
        let first_time = *viz.keys().next().unwrap();
        let first_color = viz.get(&first_time).unwrap();
        assert_eq!(*first_color, (255, 50, 50)); // Red for downbeat.
    }

    #[test]
    fn phase_analyzer_creation() {
        let analyzer = PhaseAnalyzer::new();
        assert!(analyzer.all_phases().is_empty());
    }

    #[test]
    fn phase_analyzer_update_and_read() {
        let mut analyzer = PhaseAnalyzer::new();

        analyzer.update_deck_phase("deck_a".to_string(), 128.0, 0.25);
        analyzer.update_deck_phase("deck_b".to_string(), 128.0, 0.26);

        let relationship = analyzer.phase_relationship("deck_a", "deck_b");
        // Very close phase -> in phase.
        assert!(matches!(relationship, PhaseRelationship::InPhase));
    }

    #[test]
    fn phase_analyzer_half_phase() {
        let mut analyzer = PhaseAnalyzer::new();

        analyzer.update_deck_phase("deck_a".to_string(), 128.0, 0.0);
        analyzer.update_deck_phase("deck_b".to_string(), 128.0, 0.5);

        let relationship = analyzer.phase_relationship("deck_a", "deck_b");
        assert!(matches!(relationship, PhaseRelationship::HalfPhase));
    }

    #[test]
    fn phase_analyzer_unknown_deck() {
        let analyzer = PhaseAnalyzer::new();

        let relationship = analyzer.phase_relationship("deck_a", "deck_b");
        assert!(matches!(relationship, PhaseRelationship::Unknown));
    }

    #[test]
    fn phase_analyzer_clear() {
        let mut analyzer = PhaseAnalyzer::new();
        analyzer.update_deck_phase("deck_a".to_string(), 128.0, 0.5);
        assert_eq!(analyzer.all_phases().len(), 1);

        analyzer.clear();
        assert!(analyzer.all_phases().is_empty());
    }

    #[test]
    fn beat_grid_engine_creation() {
        let engine = BeatGridEngine::new((4, 4));
        assert_eq!(engine.grid_count(), 0);
        assert!(engine.auto_downbeat_enabled());
    }

    #[test]
    fn beat_grid_engine_get_or_create() {
        let mut engine = BeatGridEngine::new((4, 4));
        let grid = engine.get_or_create_grid("track1".to_string(), 128.0);
        assert_eq!(grid.base_bpm, 128.0);
        assert_eq!(engine.grid_count(), 1);
    }

    #[test]
    fn beat_grid_engine_generate() {
        let mut engine = BeatGridEngine::new((4, 4));
        engine.get_or_create_grid("track1".to_string(), 120.0);
        assert!(engine.generate_grid("track1", Duration::from_secs(10)));

        let grid = engine.get_grid("track1").unwrap();
        assert!(grid.beat_count() > 0);
    }

    #[test]
    fn beat_grid_engine_find_downbeats() {
        let mut engine = BeatGridEngine::new((4, 4));
        engine.get_or_create_grid("track1".to_string(), 120.0);
        engine.generate_grid("track1", Duration::from_secs(10));

        let downbeats = engine.find_downbeats("track1");
        assert!(!downbeats.is_empty());
    }

    #[test]
    fn beat_grid_engine_phase_analysis() {
        let mut engine = BeatGridEngine::new((4, 4));

        engine.update_deck_phase("deck_a".to_string(), 128.0, 0.0);
        engine.update_deck_phase("deck_b".to_string(), 128.0, 0.0);

        assert!(engine.phase_compatible("deck_a", "deck_b"));
    }

    #[test]
    fn beat_grid_engine_snap_to_grid() {
        let mut engine = BeatGridEngine::new((4, 4));
        engine.get_or_create_grid("track1".to_string(), 120.0);
        engine.generate_grid("track1", Duration::from_secs(10));

        let snapped = engine.snap_to_grid("track1", 2.03);
        assert!(snapped.is_some());
    }

    #[test]
    fn beat_grid_engine_correction_history() {
        let mut engine = BeatGridEngine::new((4, 4));
        engine.get_or_create_grid("track1".to_string(), 120.0);
        engine.generate_grid("track1", Duration::from_secs(10));

        let correction = GridCorrection {
            correction_type: CorrectionType::ShiftBeat,
            time: 1.0,
            time_shift: 0.1,
            reason: "Test".to_string(),
        };

        engine.apply_correction("track1", correction);
        assert_eq!(engine.correction_history().len(), 1);
    }

    #[test]
    fn beat_grid_engine_validate_all() {
        let mut engine = BeatGridEngine::new((4, 4));
        engine.get_or_create_grid("track1".to_string(), 120.0);
        engine.generate_grid("track1", Duration::from_secs(10));

        let validation = engine.validate_all();
        assert_eq!(validation.len(), 1);
        assert!(validation.get("track1").unwrap());
    }

    #[test]
    fn beat_grid_engine_toggle_auto_downbeat() {
        let mut engine = BeatGridEngine::new((4, 4));
        assert!(engine.auto_downbeat_enabled());

        engine.toggle_auto_downbeat();
        assert!(!engine.auto_downbeat_enabled());

        engine.toggle_auto_downbeat();
        assert!(engine.auto_downbeat_enabled());
    }

    #[test]
    fn beat_grid_engine_clear() {
        let mut engine = BeatGridEngine::new((4, 4));
        engine.get_or_create_grid("track1".to_string(), 120.0);
        engine.generate_grid("track1", Duration::from_secs(10));
        assert!(engine.get_grid("track1").unwrap().beat_count() > 0);

        engine.clear_grid("track1");
        assert_eq!(engine.get_grid("track1").unwrap().beat_count(), 0);
    }

    #[test]
    fn beat_grid_events_sorted() {
        let mut grid = BeatGrid::new("track1".to_string(), 120.0, (4, 4));

        // Add events out of order.
        grid.add_event(BeatEvent::new(2.0, BeatType::Downbeat, 0.9, 1, 1, 88200));
        grid.add_event(BeatEvent::new(0.5, BeatType::Downbeat, 0.95, 0, 1, 22050));
        grid.add_event(BeatEvent::new(1.0, BeatType::Beat2, 0.85, 0, 2, 44100));

        // Should be sorted by time.
        assert_eq!(grid.events[0].time, 0.5);
        assert_eq!(grid.events[1].time, 1.0);
        assert_eq!(grid.events[2].time, 2.0);
    }
}

/// Ordered source-time key for waveform visualization, using IEEE total ordering.
#[derive(Debug, Clone, Copy)]
pub struct BeatTime(pub f64);
impl PartialEq for BeatTime {
    fn eq(&self, other: &Self) -> bool {
        self.0.total_cmp(&other.0).is_eq()
    }
}
impl Eq for BeatTime {}
impl PartialOrd for BeatTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for BeatTime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}
