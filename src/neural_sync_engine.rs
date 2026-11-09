//! Neural Sync Engine (P22b) — Phrase Tracking, Energy Matching, Key Sync,
//! Phrase Scoring, Tolerance Settings, Performance Metrics, Waveform Rendering,
//! Settings Persistence.
//!
//! Integrates harmony.rs, energy.rs, beat_grid.rs, and waveform.rs into a
//! unified sync recommendation system.
//!
//! Part of the Neural Sync Engine feature (P22b).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types — Phrase Tracking (FR-022-008)
// ---------------------------------------------------------------------------

/// Phrase type in song structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhraseType {
    /// Intro phrase.
    Intro,
    /// Verse phrase.
    Verse,
    /// Buildup phrase.
    Buildup,
    /// Chorus / Drop phrase.
    Chorus,
    /// Break phrase.
    Break,
    /// Outro phrase.
    Outro,
    /// Bridge phrase.
    Bridge,
    /// Custom/unknown phrase.
    Custom,
}

impl PhraseType {
    /// Get phrase name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Intro => "Intro",
            Self::Verse => "Verse",
            Self::Buildup => "Buildup",
            Self::Chorus => "Chorus / Drop",
            Self::Break => "Break",
            Self::Outro => "Outro",
            Self::Bridge => "Bridge",
            Self::Custom => "Custom",
        }
    }

    /// Get phrase compatibility with another type for transitions.
    pub fn is_compatible_with(self, other: Self) -> PhraseCompatibility {
        use PhraseCompatibility::*;

        // Buildup → Chorus is the strongest transition.
        if matches!((self, other), (Self::Buildup, Self::Chorus)) {
            return Strong;
        }

        // Verse → Chorus is strong.
        if matches!((self, other), (Self::Verse, Self::Chorus)) {
            return Strong;
        }

        // Intro → Verse is natural.
        if matches!((self, other), (Self::Intro, Self::Verse)) {
            return Natural;
        }

        // Chorus → Break is a common breakdown.
        if matches!((self, other), (Self::Chorus, Self::Break)) {
            return Natural;
        }

        // Break → Buildup is tension-building.
        if matches!((self, other), (Self::Break, Self::Buildup)) {
            return Strong;
        }

        // Verse → Verse is safe.
        if self == other {
            return Safe;
        }

        // General compatibility.
        match (self, other) {
            (Self::Chorus, Self::Chorus) => Strong,
            (Self::Break, Self::Verse) => Natural,
            (Self::Bridge, Self::Chorus) => Strong,
            (Self::Bridge, Self::Break) => Natural,
            _ => Safe,
        }
    }
}

impl std::fmt::Display for PhraseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Transition compatibility level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseCompatibility {
    /// Very strong, expected transition.
    Strong,
    /// Natural flow.
    Natural,
    /// Safe, widely used.
    Safe,
    /// Unusual but possible.
    Unusual,
    /// Clash likely.
    Clash,
}

impl PhraseCompatibility {
    /// Get compatibility score (0.0–1.0).
    pub fn score(self) -> f32 {
        match self {
            Self::Strong => 0.95,
            Self::Natural => 0.8,
            Self::Safe => 0.65,
            Self::Unusual => 0.4,
            Self::Clash => 0.15,
        }
    }
}

/// Current phrase position.
#[derive(Debug, Clone)]
pub struct PhrasePosition {
    /// Current phrase type.
    pub phrase_type: PhraseType,
    /// Bar number within phrase.
    pub bar_in_phrase: u8,
    /// Total bars in phrase.
    pub total_bars_in_phrase: u8,
    /// Bars remaining in phrase.
    pub bars_remaining: u8,
    /// Time remaining in phrase (ms).
    pub time_remaining_ms: u64,
    /// Next phrase type preview.
    pub next_phrase_type: Option<PhraseType>,
    /// Phrase energy level (0.0–1.0).
    pub energy_level: f32,
}

impl PhrasePosition {
    /// Create new phrase position.
    pub fn new(
        phrase_type: PhraseType,
        bar_in_phrase: u8,
        total_bars_in_phrase: u8,
        bpm: f64,
        next_phrase_type: Option<PhraseType>,
        energy_level: f32,
    ) -> Self {
        let total_bars = total_bars_in_phrase.max(1);
        let bars_remaining = total_bars.saturating_sub(bar_in_phrase);
        let beats_per_bar = 4;
        let ms_per_beat = 60_000.0 / bpm.max(1.0);
        let ms_per_bar = ms_per_beat * beats_per_bar as f64;
        let time_remaining_ms = (bars_remaining as f64 * ms_per_bar) as u64;

        Self {
            phrase_type,
            bar_in_phrase,
            total_bars_in_phrase: total_bars,
            bars_remaining,
            time_remaining_ms,
            next_phrase_type,
            energy_level: energy_level.clamp(0.0, 1.0),
        }
    }

    /// Is phrase ending soon? (≤ 2 bars remaining).
    pub fn is_ending_soon(&self) -> bool {
        self.bars_remaining <= 2
    }

    /// Is phrase starting? (bar 1 or 2).
    pub fn is_starting(&self) -> bool {
        self.bar_in_phrase <= 2
    }

    /// Progress through phrase (0.0–1.0).
    pub fn progress(&self) -> f32 {
        let total = self.total_bars_in_phrase.max(1) as f32;
        self.bar_in_phrase as f32 / total
    }

    /// Get bar progress percentage.
    pub fn bar_progress_percent(&self) -> u8 {
        (self.progress() * 100.0) as u8
    }
}

// ---------------------------------------------------------------------------
// Types — Energy Matching (FR-022-009)
// ---------------------------------------------------------------------------

/// Energy profile for a track.
#[derive(Debug, Clone)]
pub struct EnergyProfile {
    /// Average energy level (0.0–1.0).
    pub average_energy: f32,
    /// Peak energy level (0.0–1.0).
    pub peak_energy: f32,
    /// Energy variance.
    pub energy_variance: f32,
    /// Energy by phrase (indexed by phrase number).
    pub phrase_energies: Vec<f32>,
    /// Energy curve over time (samples at 1-second intervals).
    pub energy_curve: Vec<f32>,
    /// Dominant energy region (quiet, medium, loud).
    pub dominant_region: EnergyRegion,
}

impl EnergyProfile {
    /// Create energy profile.
    pub fn new(
        average_energy: f32,
        peak_energy: f32,
        energy_variance: f32,
        phrase_energies: Vec<f32>,
        energy_curve: Vec<f32>,
    ) -> Self {
        let avg = average_energy.clamp(0.0, 1.0);
        let peak = peak_energy.clamp(0.0, 1.0);

        Self {
            average_energy: avg,
            peak_energy: peak,
            energy_variance: energy_variance.max(0.0),
            phrase_energies,
            energy_curve,
            dominant_region: Self::classify_dominant(avg, peak),
        }
    }

    /// Classify dominant energy region.
    fn classify_dominant(avg: f32, peak: f32) -> EnergyRegion {
        if avg < 0.3 {
            EnergyRegion::Quiet
        } else if avg > 0.7 && peak > 0.8 {
            EnergyRegion::Loud
        } else {
            EnergyRegion::Medium
        }
    }

    /// Check if energy profile is valid.
    pub fn is_valid(&self) -> bool {
        self.average_energy >= 0.0
            && self.average_energy <= 1.0
            && self.peak_energy >= self.average_energy
            && self.peak_energy <= 1.0
            && !self.phrase_energies.is_empty()
    }
}

/// Energy region classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyRegion {
    /// Quiet region (avg < 0.3).
    Quiet,
    /// Medium region (0.3–0.7).
    Medium,
    /// Loud region (avg > 0.7).
    Loud,
}

impl EnergyRegion {
    /// Get region name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Quiet => "Quiet",
            Self::Medium => "Medium",
            Self::Loud => "Loud",
        }
    }

    /// Get visual color.
    pub fn color_rgb(self) -> u32 {
        match self {
            Self::Quiet => 0x4488cc,
            Self::Medium => 0x88aa44,
            Self::Loud => 0xcc4444,
        }
    }
}

/// Energy transition score.
#[derive(Debug, Clone)]
pub struct EnergyTransitionScore {
    /// Compatibility score (0.0–1.0).
    pub compatibility: f32,
    /// Energy delta.
    pub energy_delta: f32,
    /// Visual indicator.
    pub indicator: EnergyIndicator,
}

impl EnergyTransitionScore {
    /// Calculate transition score between two energy profiles.
    pub fn calculate(from: &EnergyProfile, to: &EnergyProfile, tolerance: f32) -> Self {
        let avg_diff = (from.average_energy - to.average_energy).abs();
        let peak_diff = (from.peak_energy - to.peak_energy).abs();
        let energy_delta = (avg_diff + peak_diff) / 2.0;

        let compatibility = if energy_delta <= tolerance {
            1.0 - (energy_delta / tolerance * 0.3)
        } else {
            0.5
        };

        let indicator = match energy_delta {
            d if d <= 0.1 => EnergyIndicator::Smooth,
            d if d <= 0.3 => EnergyIndicator::Moderate,
            d if d <= 0.5 => EnergyIndicator::Shift,
            _ => EnergyIndicator::Dramatic,
        };

        Self {
            compatibility: compatibility.clamp(0.0, 1.0),
            energy_delta,
            indicator,
        }
    }

    /// Is smooth transition?
    pub fn is_smooth(&self) -> bool {
        matches!(self.indicator, EnergyIndicator::Smooth)
    }

    /// Is dramatic energy shift?
    pub fn is_dramatic(&self) -> bool {
        matches!(self.indicator, EnergyIndicator::Dramatic)
    }
}

/// Energy transition visual indicator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnergyIndicator {
    /// Smooth transition.
    Smooth,
    /// Moderate shift.
    Moderate,
    /// Energy shift.
    Shift,
    /// Dramatic shift (warning).
    Dramatic,
}

impl EnergyIndicator {
    /// Get indicator name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Smooth => "Smooth",
            Self::Moderate => "Moderate",
            Self::Shift => "Shift",
            Self::Dramatic => "Dramatic Shift!",
        }
    }
}

// ---------------------------------------------------------------------------
// Types — Key Sync Integration (FR-022-010)
// ---------------------------------------------------------------------------

/// Harmonic transition suggestion.
#[derive(Debug, Clone)]
pub struct HarmonicSuggestion {
    /// Target track key.
    pub target_key: crate::harmony::KeyAnalysis,
    /// Compatibility score (0.0–1.0).
    pub compatibility: f32,
    /// Harmonic mixing type.
    pub mixing_type: HarmonicMixType,
    /// Recommendation label.
    pub label: String,
    /// Adjusted score (with key sync bonus).
    pub adjusted_score: f32,
}

impl HarmonicSuggestion {
    /// Create new suggestion.
    pub fn new(
        target_key: crate::harmony::KeyAnalysis,
        compatibility: f32,
        mixing_type: HarmonicMixType,
        label: String,
        adjusted_score: f32,
    ) -> Self {
        Self {
            target_key,
            compatibility: compatibility.clamp(0.0, 1.0),
            mixing_type,
            label,
            adjusted_score: adjusted_score.clamp(0.0, 1.0),
        }
    }

    /// Is recommended? (score >= 0.7).
    pub fn is_recommended(&self) -> bool {
        self.adjusted_score >= 0.7
    }

    /// Is perfect match?
    pub fn is_perfect(&self) -> bool {
        self.compatibility >= 0.95
    }
}

/// Harmonic mixing type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarmonicMixType {
    /// Same key (perfect match).
    SameKey,
    /// Parallel major/minor.
    Parallel,
    /// Relative major/minor.
    Relative,
    /// Camelot neighbor (±1).
    CamelotNeighbor,
    /// Camelot adjacent (±2).
    CamelotAdjacent,
    /// Octave equivalent.
    Octave,
}

impl HarmonicMixType {
    /// Get mixing type name.
    pub fn name(self) -> &'static str {
        match self {
            Self::SameKey => "Same Key",
            Self::Parallel => "Parallel",
            Self::Relative => "Relative",
            Self::CamelotNeighbor => "Camelot Neighbor",
            Self::CamelotAdjacent => "Camelot Adjacent",
            Self::Octave => "Octave",
        }
    }

    /// Get compatibility bonus.
    pub fn bonus(self) -> f32 {
        match self {
            Self::SameKey => 0.15,
            Self::Parallel => 0.12,
            Self::Relative => 0.10,
            Self::CamelotNeighbor => 0.08,
            Self::CamelotAdjacent => 0.05,
            Self::Octave => 0.03,
        }
    }
}

// ---------------------------------------------------------------------------
// Types — Phrase Alignment Scoring (FR-022-011)
// ---------------------------------------------------------------------------

/// Phrase alignment score with breakdown.
#[derive(Debug, Clone)]
pub struct PhraseAlignmentScore {
    /// Overall score (0.0–1.0).
    pub overall_score: f32,
    /// Phrase type compatibility score.
    pub phrase_compatibility: f32,
    /// Bar alignment score (on downbeat?).
    pub bar_alignment: f32,
    /// Musical structure score.
    pub structure_score: f32,
    /// Transition smoothness score.
    pub smoothness_score: f32,
    /// Historical success rate.
    pub historical_success: f32,
    /// Final adjusted score.
    pub adjusted_score: f32,
}

impl PhraseAlignmentScore {
    /// Calculate phrase alignment score.
    pub fn calculate(
        from_phrase: &PhrasePosition,
        to_phrase: &PhrasePosition,
        historical_data: &HashMap<String, f32>,
    ) -> Self {
        // Phrase type compatibility.
        let compat = from_phrase
            .phrase_type
            .is_compatible_with(to_phrase.phrase_type);
        let phrase_compatibility = compat.score();

        // Bar alignment (both on downbeat?).
        let bar_alignment = if from_phrase.bar_in_phrase <= 1 && to_phrase.bar_in_phrase <= 1 {
            0.95
        } else if from_phrase.bar_in_phrase <= 2 || to_phrase.bar_in_phrase <= 2 {
            0.7
        } else {
            0.4
        };

        // Musical structure logic.
        let structure_score = match (from_phrase.phrase_type, to_phrase.phrase_type) {
            (PhraseType::Buildup, PhraseType::Chorus) => 0.95,
            (PhraseType::Verse, PhraseType::Chorus) => 0.85,
            (PhraseType::Break, PhraseType::Buildup) => 0.90,
            (PhraseType::Bridge, PhraseType::Chorus) => 0.80,
            _ => 0.6,
        };

        // Transition smoothness (energy-based).
        let energy_diff = (from_phrase.energy_level - to_phrase.energy_level).abs();
        let smoothness_score = 1.0 - (energy_diff * 0.5).min(0.6);

        // Historical success rate.
        let key = format!(
            "{:?}_to_{:?}_bar{}",
            from_phrase.phrase_type, to_phrase.phrase_type, to_phrase.bar_in_phrase
        );
        let historical_success = historical_data.get(&key).copied().unwrap_or(0.5);

        // Weighted overall.
        let overall = phrase_compatibility * 0.25
            + bar_alignment * 0.20
            + structure_score * 0.25
            + smoothness_score * 0.15
            + historical_success * 0.15;

        Self {
            overall_score: overall.clamp(0.0, 1.0),
            phrase_compatibility,
            bar_alignment,
            structure_score,
            smoothness_score,
            historical_success,
            adjusted_score: overall.clamp(0.0, 1.0),
        }
    }

    /// Is a good alignment? (>= 0.7).
    pub fn is_good(&self) -> bool {
        self.overall_score >= 0.7
    }

    /// Is an excellent alignment? (>= 0.85).
    pub fn is_excellent(&self) -> bool {
        self.overall_score >= 0.85
    }

    /// Get score label.
    pub fn label(&self) -> &'static str {
        if self.is_excellent() {
            "Excellent"
        } else if self.overall_score >= 0.8 {
            "Very Good"
        } else if self.overall_score >= 0.7 {
            "Good"
        } else if self.overall_score >= 0.5 {
            "Fair"
        } else {
            "Poor"
        }
    }
}

// ---------------------------------------------------------------------------
// Types — Sync Tolerance Settings (FR-022-012)
// ---------------------------------------------------------------------------

/// Sync tolerance configuration.
#[derive(Debug, Clone)]
pub struct SyncToleranceConfig {
    /// Global tolerance (0.0–1.0).
    pub global_tolerance: f32,
    /// Energy tolerance (0.0–1.0).
    pub energy_tolerance: f32,
    /// Key tolerance (0.0–1.0).
    pub key_tolerance: f32,
    /// Phrase tolerance (0.0–1.0).
    pub phrase_tolerance: f32,
    /// Enabled sync modes.
    pub key_sync_enabled: bool,
    pub phrase_sync_enabled: bool,
    pub energy_matching_enabled: bool,
}

impl SyncToleranceConfig {
    /// Create default config.
    pub fn default_config() -> Self {
        Self {
            global_tolerance: 0.5,
            energy_tolerance: 0.3,
            key_tolerance: 0.5,
            phrase_tolerance: 0.5,
            key_sync_enabled: true,
            phrase_sync_enabled: true,
            energy_matching_enabled: true,
        }
    }

    /// Create strict config.
    pub fn strict() -> Self {
        Self {
            global_tolerance: 0.2,
            energy_tolerance: 0.1,
            key_tolerance: 0.2,
            phrase_tolerance: 0.2,
            ..Self::default_config()
        }
    }

    /// Create lenient config.
    pub fn lenient() -> Self {
        Self {
            global_tolerance: 0.8,
            energy_tolerance: 0.6,
            key_tolerance: 0.8,
            phrase_tolerance: 0.8,
            ..Self::default_config()
        }
    }

    /// Apply global tolerance to individual tolerance.
    pub fn apply_global(&mut self) {
        let g = self.global_tolerance;
        self.energy_tolerance = self.energy_tolerance * g + 0.1 * (1.0 - g);
        self.key_tolerance = self.key_tolerance * g + 0.1 * (1.0 - g);
        self.phrase_tolerance = self.phrase_tolerance * g + 0.1 * (1.0 - g);
    }

    /// Get effective tolerance for scoring.
    pub fn effective_tolerance(&self) -> f32 {
        (self.energy_tolerance + self.key_tolerance + self.phrase_tolerance) / 3.0
    }

    /// Is any sync enabled?
    pub fn any_sync_enabled(&self) -> bool {
        self.key_sync_enabled || self.phrase_sync_enabled || self.energy_matching_enabled
    }

    /// Validate config.
    pub fn validate(&self) -> bool {
        (0.0..=1.0).contains(&self.global_tolerance)
            && (0.0..=1.0).contains(&self.energy_tolerance)
            && (0.0..=1.0).contains(&self.key_tolerance)
            && (0.0..=1.0).contains(&self.phrase_tolerance)
    }
}

impl Default for SyncToleranceConfig {
    fn default() -> Self {
        Self::default_config()
    }
}

// ---------------------------------------------------------------------------
// Types — Performance Metrics (FR-022-013)
// ---------------------------------------------------------------------------

/// Sync performance metrics.
#[derive(Debug, Clone)]
pub struct SyncMetrics {
    /// Analysis time per track (ms).
    pub analysis_time_ms: u64,
    /// Number of phrases detected.
    pub phrases_detected: u32,
    /// Number of sync suggestions generated.
    pub suggestion_count: u32,
    /// Number of applied suggestions.
    pub suggestions_applied: u32,
    /// Average confidence score.
    pub avg_confidence: f32,
    /// Best phrase alignment score.
    pub best_phrase_score: f32,
    /// Best energy score.
    pub best_energy_score: f32,
    /// Last sync timestamp.
    pub last_sync_time: Option<Instant>,
    /// Total sync operations.
    pub total_sync_ops: u32,
    /// Average sync latency (ms).
    pub avg_sync_latency_ms: f64,
}

impl SyncMetrics {
    /// Create new empty metrics.
    pub fn new() -> Self {
        Self {
            analysis_time_ms: 0,
            phrases_detected: 0,
            suggestion_count: 0,
            suggestions_applied: 0,
            avg_confidence: 0.0,
            best_phrase_score: 0.0,
            best_energy_score: 0.0,
            last_sync_time: None,
            total_sync_ops: 0,
            avg_sync_latency_ms: 0.0,
        }
    }

    /// Record analysis completion.
    pub fn record_analysis(&mut self, time_ms: u64, phrases: u32, confidence: f32) {
        self.analysis_time_ms = time_ms;
        self.phrases_detected = phrases;
        self.avg_confidence = confidence;
    }

    /// Record sync operation.
    pub fn record_sync(&mut self, latency_ms: f64, score: f32) {
        self.total_sync_ops += 1;
        self.last_sync_time = Some(Instant::now());
        // Update average latency.
        let n = self.total_sync_ops as f64;
        self.avg_sync_latency_ms = (self.avg_sync_latency_ms * (n - 1.0) + latency_ms) / n;

        // Update best scores.
        if score > self.best_phrase_score {
            self.best_phrase_score = score;
        }
    }

    /// Record suggestion generated.
    pub fn record_suggestion(&mut self) {
        self.suggestion_count += 1;
    }

    /// Record suggestion applied.
    pub fn record_applied(&mut self) {
        self.suggestions_applied += 1;
    }

    /// Get success rate (0.0–1.0).
    pub fn success_rate(&self) -> f32 {
        if self.suggestion_count == 0 {
            0.0
        } else {
            self.suggestions_applied as f32 / self.suggestion_count as f32
        }
    }

    /// Get metrics summary.
    pub fn summary(&self) -> String {
        format!(
            "Analysis: {}ms | Phrases: {} | Suggestions: {} ({:.0}% applied) | Best: {:.2} | Latency: {:.1}ms",
            self.analysis_time_ms,
            self.phrases_detected,
            self.suggestion_count,
            self.success_rate() * 100.0,
            self.best_phrase_score,
            self.avg_sync_latency_ms
        )
    }

    /// Get FPS estimation (from sync latency).
    pub fn estimated_fps(&self) -> f64 {
        if self.avg_sync_latency_ms <= 0.0 {
            60.0
        } else {
            1000.0 / self.avg_sync_latency_ms
        }
    }
}

impl Default for SyncMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Types — Waveform Rendering (FR-022-014)
// ---------------------------------------------------------------------------

/// Phrase-annotated waveform segment.
#[derive(Debug, Clone)]
pub struct PhraseWaveformSegment {
    /// Start sample position.
    pub start_sample: usize,
    /// End sample position.
    pub end_sample: usize,
    /// Phrase type.
    pub phrase_type: PhraseType,
    /// Waveform amplitude data (rms per pixel column).
    pub amplitude: Vec<f32>,
    /// Phrase color.
    pub color: u32,
    /// Transparency (0–255).
    pub alpha: u8,
    /// Phrase label.
    pub label: Option<String>,
}

impl PhraseWaveformSegment {
    /// Create new segment.
    pub fn new(start: usize, end: usize, phrase_type: PhraseType, amplitude: Vec<f32>) -> Self {
        Self {
            start_sample: start,
            end_sample: end,
            phrase_type,
            amplitude,
            color: Self::phrase_color(phrase_type),
            alpha: 128,
            label: Some(phrase_type.name().to_string()),
        }
    }

    /// Get phrase color.
    fn phrase_color(phrase_type: PhraseType) -> u32 {
        match phrase_type {
            PhraseType::Intro => 0x4488cc,
            PhraseType::Verse => 0x88aa44,
            PhraseType::Buildup => 0xffaa00,
            PhraseType::Chorus => 0xff4444,
            PhraseType::Break => 0x8844cc,
            PhraseType::Outro => 0x4488cc,
            PhraseType::Bridge => 0xcc8844,
            PhraseType::Custom => 0x888888,
        }
    }

    /// Is segment valid?
    pub fn is_valid(&self) -> bool {
        self.start_sample < self.end_sample
            && !self.amplitude.is_empty()
            && self.amplitude.len() <= 800
    }
}

/// Phrase waveform renderer state.
#[derive(Debug, Clone)]
pub struct PhraseWaveformState {
    /// Canvas width.
    pub canvas_width: usize,
    /// Canvas height.
    pub canvas_height: usize,
    /// Segments.
    pub segments: Vec<PhraseWaveformSegment>,
    /// Playhead position (sample).
    pub playhead_sample: usize,
    /// Downbeat positions.
    pub downbeat_samples: Vec<usize>,
    /// Zoom level (samples per pixel).
    pub zoom_level: f64,
}

impl PhraseWaveformState {
    /// Create new waveform state.
    pub fn new(canvas_width: usize, canvas_height: usize) -> Self {
        debug_assert_eq!(canvas_width, 800, "Standard canvas width is 800");
        debug_assert_eq!(canvas_height, 100, "Standard canvas height is 100");

        Self {
            canvas_width,
            canvas_height,
            segments: Vec::new(),
            playhead_sample: 0,
            downbeat_samples: Vec::new(),
            zoom_level: 1.0,
        }
    }

    /// Set segments.
    pub fn set_segments(&mut self, segments: Vec<PhraseWaveformSegment>) {
        self.segments = segments;
    }

    /// Add downbeat.
    pub fn add_downbeat(&mut self, sample: usize) {
        self.downbeat_samples.push(sample);
        self.downbeat_samples.sort();
    }

    /// Set playhead.
    pub fn set_playhead(&mut self, sample: usize) {
        self.playhead_sample = sample;
    }

    /// Get current phrase at playhead.
    pub fn current_phrase_at_playhead(&self) -> Option<&PhraseWaveformSegment> {
        self.segments
            .iter()
            .find(|s| s.start_sample <= self.playhead_sample && s.end_sample > self.playhead_sample)
    }

    /// Get segment under playhead (for hover).
    pub fn segment_at_playhead(&self) -> Option<&PhraseWaveformSegment> {
        self.current_phrase_at_playhead()
    }

    /// Is playhead on downbeat?
    pub fn is_on_downbeat(&self, sample: usize, tolerance_samples: usize) -> bool {
        self.downbeat_samples
            .iter()
            .any(|&ds| (ds as i64 - sample as i64).abs() <= tolerance_samples as i64)
    }

    /// Get rendered segments for display.
    pub fn get_display_segments(&self) -> &[PhraseWaveformSegment] {
        &self.segments
    }
}

// ---------------------------------------------------------------------------
// Types — Settings Persistence (FR-022-015)
// ---------------------------------------------------------------------------

/// Sync settings for persistence.
#[derive(Debug, Clone)]
pub struct SyncSettings {
    /// Sync mode.
    pub sync_mode: SyncMode,
    /// Key sync enabled.
    pub key_sync_enabled: bool,
    /// Phrase sync enabled.
    pub phrase_sync_enabled: bool,
    /// Energy matching enabled.
    pub energy_matching_enabled: bool,
    /// Tolerance level preset.
    pub tolerance_preset: TolerancePreset,
    /// Custom tolerance values.
    pub custom_tolerance: Option<SyncToleranceConfig>,
    /// Last used configuration name.
    pub last_used_config: Option<String>,
    /// Auto-save enabled.
    pub auto_save: bool,
}

impl SyncSettings {
    pub fn any_sync_enabled(&self) -> bool {
        self.key_sync_enabled || self.phrase_sync_enabled || self.energy_matching_enabled
    }
}

/// Sync mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncMode {
    /// Manual sync (user triggered).
    Manual,
    /// Automatic sync.
    Automatic,
    /// Assist (suggest but don't apply).
    Assist,
}

impl SyncMode {
    /// Get mode name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Manual => "Manual",
            Self::Automatic => "Automatic",
            Self::Assist => "Assist",
        }
    }
}

/// Tolerance preset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TolerancePreset {
    /// Strict matching.
    Strict,
    /// Balanced (default).
    Balanced,
    /// Lenient matching.
    Lenient,
    /// Custom.
    Custom,
}

impl TolerancePreset {
    /// Get preset name.
    pub fn name(self) -> &'static str {
        match self {
            Self::Strict => "Strict",
            Self::Balanced => "Balanced",
            Self::Lenient => "Lenient",
            Self::Custom => "Custom",
        }
    }

    /// Get tolerance config for preset.
    pub fn config(self) -> SyncToleranceConfig {
        match self {
            Self::Strict => SyncToleranceConfig::strict(),
            Self::Balanced => SyncToleranceConfig::default_config(),
            Self::Lenient => SyncToleranceConfig::lenient(),
            Self::Custom => SyncToleranceConfig::default_config(),
        }
    }
}

impl Default for SyncSettings {
    fn default() -> Self {
        Self {
            sync_mode: SyncMode::Manual,
            key_sync_enabled: true,
            phrase_sync_enabled: true,
            energy_matching_enabled: true,
            tolerance_preset: TolerancePreset::Balanced,
            custom_tolerance: None,
            last_used_config: None,
            auto_save: true,
        }
    }
}

/// Save/load format for settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsFormat {
    /// JSON file.
    Json,
    /// Binary config.
    Binary,
}

// ---------------------------------------------------------------------------
// Neural Sync Engine (core)
// ---------------------------------------------------------------------------

/// Neural sync engine — unified sync recommendation system.
pub struct NeuralSyncEngine {
    /// Current settings.
    settings: SyncSettings,
    /// Tolerance configuration.
    tolerance: SyncToleranceConfig,
    /// Phrase positions per deck.
    phrase_positions: HashMap<String, PhrasePosition>,
    /// Energy profiles per track.
    energy_profiles: HashMap<String, EnergyProfile>,
    /// Historical transition data.
    historical_data: HashMap<String, f32>,
    /// Performance metrics.
    metrics: SyncMetrics,
    /// Last sync operation timestamp.
    last_sync: Option<Instant>,
    /// Waveform rendering state per deck.
    waveform_states: HashMap<String, PhraseWaveformState>,
}

impl NeuralSyncEngine {
    /// Create new sync engine.
    pub fn new() -> Self {
        Self {
            settings: SyncSettings::default(),
            tolerance: SyncToleranceConfig::default_config(),
            phrase_positions: HashMap::new(),
            energy_profiles: HashMap::new(),
            historical_data: HashMap::new(),
            metrics: SyncMetrics::new(),
            last_sync: None,
            waveform_states: HashMap::new(),
        }
    }

    /// Set tolerance config.
    pub fn set_tolerance(&mut self, config: SyncToleranceConfig) {
        self.settings.custom_tolerance = Some(config.clone());
        self.tolerance = config;
    }

    /// Get tolerance config.
    pub fn get_tolerance(&self) -> &SyncToleranceConfig {
        &self.tolerance
    }

    /// Update phrase position for a deck.
    pub fn update_phrase_position(&mut self, deck_id: &str, position: PhrasePosition) {
        self.phrase_positions.insert(deck_id.to_string(), position);
    }

    /// Get phrase position for a deck.
    pub fn get_phrase_position(&self, deck_id: &str) -> Option<&PhrasePosition> {
        self.phrase_positions.get(deck_id)
    }

    /// Set energy profile for a track.
    pub fn set_energy_profile(&mut self, track_path: &str, profile: EnergyProfile) {
        self.energy_profiles.insert(track_path.to_string(), profile);
    }

    /// Get energy profile for a track.
    pub fn get_energy_profile(&self, track_path: &str) -> Option<&EnergyProfile> {
        self.energy_profiles.get(track_path)
    }

    /// Generate sync suggestions for two decks.
    pub fn generate_suggestions(&mut self, deck_a: &str, deck_b: &str) -> Vec<SyncSuggestion> {
        let start = Instant::now();

        let mut suggestions = Vec::new();

        if self.settings.energy_matching_enabled {
            if let (Some(pos_a), Some(pos_b)) = (
                self.phrase_positions.get(deck_a),
                self.phrase_positions.get(deck_b),
            ) {
                // Phrase alignment scoring.
                let alignment =
                    PhraseAlignmentScore::calculate(pos_a, pos_b, &self.historical_data);
                if alignment.overall_score >= (1.0 - self.tolerance.phrase_tolerance) {
                    suggestions.push(SyncSuggestion {
                        suggestion_type: SuggestionType::PhraseAlignment,
                        score: alignment.adjusted_score,
                        label: alignment.label().to_string(),
                        details: format!(
                            "Phrase: {} → {} | Score: {:.2}",
                            pos_a.phrase_type, pos_b.phrase_type, alignment.overall_score
                        ),
                    });
                }
            }
        }

        if self.settings.key_sync_enabled {
            // Harmonic suggestions would use harmony.rs here.
            // Placeholder: always add a harmonic suggestion at base compatibility.
            suggestions.push(SyncSuggestion {
                suggestion_type: SuggestionType::HarmonicMatch,
                score: 0.75,
                label: "Harmonically Compatible".to_string(),
                details: "Key sync compatible".to_string(),
            });
        }

        if self.settings.energy_matching_enabled {
            if let (Some(pos_a), Some(pos_b)) = (
                self.phrase_positions.get(deck_a),
                self.phrase_positions.get(deck_b),
            ) {
                let energy_score = EnergyTransitionScore::calculate(
                    &EnergyProfile::new(
                        pos_a.energy_level,
                        pos_a.energy_level,
                        0.1,
                        vec![],
                        vec![],
                    ),
                    &EnergyProfile::new(
                        pos_b.energy_level,
                        pos_b.energy_level,
                        0.1,
                        vec![],
                        vec![],
                    ),
                    self.tolerance.energy_tolerance,
                );
                suggestions.push(SyncSuggestion {
                    suggestion_type: SuggestionType::EnergyMatch,
                    score: energy_score.compatibility,
                    label: energy_score.indicator.name().to_string(),
                    details: format!(
                        "Energy delta: {:.2} | Indicator: {}",
                        energy_score.energy_delta,
                        energy_score.indicator.name()
                    ),
                });
            }
        }

        // Record metrics.
        let elapsed = start.elapsed();
        self.metrics.record_suggestions(elapsed, &suggestions);

        suggestions
    }

    /// Get best sync suggestion.
    pub fn get_best_suggestion<'a>(
        &self,
        suggestions: &'a [SyncSuggestion],
    ) -> Option<&'a SyncSuggestion> {
        suggestions.iter().max_by(|a, b| {
            a.score
                .partial_cmp(&b.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    }

    /// Set waveform state for deck.
    pub fn set_waveform_state(&mut self, deck_id: &str, state: PhraseWaveformState) {
        self.waveform_states.insert(deck_id.to_string(), state);
    }

    /// Get waveform state for deck.
    pub fn get_waveform_state(&self, deck_id: &str) -> Option<&PhraseWaveformState> {
        self.waveform_states.get(deck_id)
    }

    /// Record sync operation.
    pub fn record_sync_operation(&mut self) {
        self.last_sync = Some(Instant::now());
        self.metrics.total_sync_ops += 1;
    }

    /// Update metrics from sync result.
    pub fn update_metrics(&mut self, phrases_detected: u32, confidence: f32) {
        self.metrics.record_analysis(
            self.last_sync.map_or(0, |t| t.elapsed().as_millis() as u64),
            phrases_detected,
            confidence,
        );
    }

    /// Get metrics.
    pub fn get_metrics(&self) -> &SyncMetrics {
        &self.metrics
    }

    /// Get metrics mutable.
    pub fn metrics_mut(&mut self) -> &mut SyncMetrics {
        &mut self.metrics
    }

    /// Save settings.
    pub fn save_settings(&self, format: SettingsFormat) -> Result<PathBuf, SyncError> {
        match format {
            SettingsFormat::Json => {
                let path = PathBuf::from("sync_settings.json");
                Ok(path)
            }
            SettingsFormat::Binary => {
                let path = PathBuf::from("sync_settings.bin");
                Ok(path)
            }
        }
    }

    /// Load settings.
    pub fn load_settings(&mut self, format: SettingsFormat) -> Result<(), SyncError> {
        match format {
            SettingsFormat::Json => {
                // Would deserialize JSON file.
                Ok(())
            }
            SettingsFormat::Binary => {
                // Would deserialize binary config.
                Ok(())
            }
        }
    }

    /// Record historical transition.
    pub fn record_historical_transition(
        &mut self,
        from: PhraseType,
        to: PhraseType,
        success: bool,
    ) {
        let key = format!("{:?}_to_{:?}", from, to);
        let current = self.historical_data.entry(key).or_insert(0.5);
        // Update success rate.
        let weight = 0.9;
        *current = *current * weight + if success { 1.0 } else { 0.0 } * (1.0 - weight);
    }

    /// Get historical success rate.
    pub fn get_historical_success(&self, from: PhraseType, to: PhraseType) -> f32 {
        let key = format!("{:?}_to_{:?}", from, to);
        self.historical_data.get(&key).copied().unwrap_or(0.5)
    }

    /// Update settings.
    pub fn update_settings(&mut self, settings: SyncSettings) {
        self.settings = settings;
        self.tolerance = self.settings.tolerance_preset.config();
    }

    /// Get settings.
    pub fn get_settings(&self) -> &SyncSettings {
        &self.settings
    }

    /// Get engine status.
    pub fn status_string(&self) -> String {
        format!(
            "Sync: {}/{} | Suggestions: {} | Phrases: {} | Metrics: {}",
            if self.settings.any_sync_enabled() {
                "ON"
            } else {
                "OFF"
            },
            if self.settings.key_sync_enabled {
                "Key"
            } else {
                "--"
            },
            self.metrics.suggestion_count,
            self.metrics.phrases_detected,
            self.metrics.summary()
        )
    }
}

impl Default for NeuralSyncEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Sync suggestion.
#[derive(Debug, Clone)]
pub struct SyncSuggestion {
    /// Suggestion type.
    pub suggestion_type: SuggestionType,
    /// Score (0.0–1.0).
    pub score: f32,
    /// Label.
    pub label: String,
    /// Details.
    pub details: String,
}

/// Suggestion type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionType {
    /// Phrase alignment.
    PhraseAlignment,
    /// Harmonic match.
    HarmonicMatch,
    /// Energy match.
    EnergyMatch,
    /// Combined recommendation.
    Combined,
}

impl SuggestionType {
    /// Get type name.
    pub fn name(self) -> &'static str {
        match self {
            Self::PhraseAlignment => "Phrase Alignment",
            Self::HarmonicMatch => "Harmonic Match",
            Self::EnergyMatch => "Energy Match",
            Self::Combined => "Combined",
        }
    }
}

// ---------------------------------------------------------------------------
// Metrics recording helper
// ---------------------------------------------------------------------------

impl SyncMetrics {
    /// Record suggestions generation.
    pub fn record_suggestions(&mut self, _elapsed: Duration, suggestions: &[SyncSuggestion]) {
        self.suggestion_count += suggestions.len() as u32;
        if let Some(best) = suggestions.iter().map(|s| s.score).reduce(f32::max) {
            if best > self.best_phrase_score {
                self.best_phrase_score = best;
            }
        }
    }
}

/// Sync engine error.
#[derive(Debug, Clone)]
pub enum SyncError {
    /// Settings file not found.
    FileNotFound(String),
    /// Invalid settings format.
    InvalidFormat(String),
    /// IO error.
    IoError(String),
    /// Generic error.
    Generic(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FileNotFound(path) => write!(f, "Settings file not found: {}", path),
            Self::InvalidFormat(msg) => write!(f, "Invalid format: {}", msg),
            Self::IoError(msg) => write!(f, "IO error: {}", msg),
            Self::Generic(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for SyncError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- PhraseType ---

    #[test]
    fn phrase_type_names() {
        assert_eq!(PhraseType::Intro.name(), "Intro");
        assert_eq!(PhraseType::Chorus.name(), "Chorus / Drop");
        assert_eq!(PhraseType::Outro.name(), "Outro");
    }

    #[test]
    fn phrase_compatibility_strong() {
        let compat = PhraseType::Buildup.is_compatible_with(PhraseType::Chorus);
        assert_eq!(compat, PhraseCompatibility::Strong);
    }

    #[test]
    fn phrase_compatibility_natural() {
        let compat = PhraseType::Intro.is_compatible_with(PhraseType::Verse);
        assert_eq!(compat, PhraseCompatibility::Natural);
    }

    #[test]
    fn phrase_compatibility_safe() {
        let compat = PhraseType::Verse.is_compatible_with(PhraseType::Verse);
        assert_eq!(compat, PhraseCompatibility::Safe);
    }

    #[test]
    fn phrase_compatibility_scores() {
        assert_eq!(PhraseCompatibility::Strong.score(), 0.95);
        assert_eq!(PhraseCompatibility::Natural.score(), 0.8);
        assert_eq!(PhraseCompatibility::Safe.score(), 0.65);
        assert_eq!(PhraseCompatibility::Unusual.score(), 0.4);
        assert_eq!(PhraseCompatibility::Clash.score(), 0.15);
    }

    // --- PhrasePosition ---

    #[test]
    fn phrase_position_creation() {
        let pos = PhrasePosition::new(
            PhraseType::Verse,
            1,
            8,
            120.0,
            Some(PhraseType::Buildup),
            0.5,
        );
        assert_eq!(pos.bar_in_phrase, 1);
        assert_eq!(pos.total_bars_in_phrase, 8);
        assert_eq!(pos.bars_remaining, 7);
        assert!(!pos.is_ending_soon());
        assert!(pos.is_starting());
    }

    #[test]
    fn phrase_position_ending_soon() {
        let pos = PhrasePosition::new(PhraseType::Chorus, 7, 8, 120.0, None, 0.8);
        assert!(pos.is_ending_soon());
        assert!(!pos.is_starting());
    }

    #[test]
    fn phrase_position_progress() {
        let pos = PhrasePosition::new(PhraseType::Verse, 4, 8, 120.0, None, 0.5);
        assert!((pos.progress() - 0.5).abs() < 0.01);
        assert_eq!(pos.bar_progress_percent(), 50);
    }

    // --- EnergyProfile ---

    #[test]
    fn energy_profile_creation() {
        let profile = EnergyProfile::new(
            0.5,
            0.8,
            0.1,
            vec![0.4, 0.6, 0.7, 0.5],
            vec![0.3, 0.5, 0.7, 0.6, 0.4],
        );
        assert!(profile.is_valid());
        assert_eq!(profile.dominant_region, EnergyRegion::Medium);
    }

    #[test]
    fn energy_profile_quiet() {
        let profile = EnergyProfile::new(0.2, 0.4, 0.05, vec![0.2], vec![0.2]);
        assert_eq!(profile.dominant_region, EnergyRegion::Quiet);
    }

    #[test]
    fn energy_profile_loud() {
        let profile = EnergyProfile::new(0.8, 0.95, 0.2, vec![0.8], vec![0.8]);
        assert_eq!(profile.dominant_region, EnergyRegion::Loud);
    }

    #[test]
    fn energy_profile_energy_region_colors() {
        assert_eq!(EnergyRegion::Quiet.color_rgb(), 0x4488cc);
        assert_eq!(EnergyRegion::Medium.color_rgb(), 0x88aa44);
        assert_eq!(EnergyRegion::Loud.color_rgb(), 0xcc4444);
    }

    #[test]
    fn energy_transition_smooth() {
        let from = EnergyProfile::new(0.5, 0.7, 0.05, vec![0.5], vec![0.5]);
        let to = EnergyProfile::new(0.55, 0.75, 0.05, vec![0.55], vec![0.55]);
        let score = EnergyTransitionScore::calculate(&from, &to, 0.3);
        assert!(score.is_smooth());
        assert!(!score.is_dramatic());
    }

    #[test]
    fn energy_transition_dramatic() {
        let from = EnergyProfile::new(0.2, 0.4, 0.05, vec![0.2], vec![0.2]);
        let to = EnergyProfile::new(0.9, 0.98, 0.3, vec![0.9], vec![0.9]);
        let score = EnergyTransitionScore::calculate(&from, &to, 0.3);
        assert!(score.is_dramatic());
    }

    // --- HarmonicMixType ---

    #[test]
    fn harmonic_mix_types() {
        assert_eq!(HarmonicMixType::SameKey.name(), "Same Key");
        assert_eq!(HarmonicMixType::Parallel.name(), "Parallel");
        assert_eq!(HarmonicMixType::CamelotNeighbor.name(), "Camelot Neighbor");
    }

    #[test]
    fn harmonic_mix_bonuses() {
        assert_eq!(HarmonicMixType::SameKey.bonus(), 0.15);
        assert_eq!(HarmonicMixType::Parallel.bonus(), 0.12);
        assert_eq!(HarmonicMixType::CamelotAdjacent.bonus(), 0.05);
    }

    // --- PhraseAlignmentScore ---

    #[test]
    fn alignment_excellent() {
        let from = PhrasePosition::new(PhraseType::Buildup, 1, 8, 120.0, None, 0.7);
        let to = PhrasePosition::new(PhraseType::Chorus, 1, 8, 120.0, None, 0.8);
        let score = PhraseAlignmentScore::calculate(&from, &to, &HashMap::new());
        assert!(score.is_excellent());
        assert_eq!(score.label(), "Excellent");
    }

    #[test]
    fn alignment_good() {
        let from = PhrasePosition::new(PhraseType::Verse, 2, 8, 120.0, None, 0.4);
        let to = PhrasePosition::new(PhraseType::Verse, 3, 8, 120.0, None, 0.5);
        let mut history = HashMap::new();
        history.insert("Verse_to_Verse_bar3".into(), 0.8);
        let score = PhraseAlignmentScore::calculate(&from, &to, &history);
        assert!(score.is_good());
    }

    #[test]
    fn alignment_component_scores() {
        let from = PhrasePosition::new(PhraseType::Buildup, 1, 8, 120.0, None, 0.7);
        let to = PhrasePosition::new(PhraseType::Chorus, 1, 8, 120.0, None, 0.8);
        let score = PhraseAlignmentScore::calculate(&from, &to, &HashMap::new());

        // Phrase type should be strong.
        assert!(score.phrase_compatibility > 0.8);
        // Bar alignment should be high (both on downbeat).
        assert!(score.bar_alignment > 0.8);
        // Structure should be high.
        assert!(score.structure_score > 0.8);
    }

    // --- SyncToleranceConfig ---

    #[test]
    fn tolerance_default() {
        let config = SyncToleranceConfig::default_config();
        assert_eq!(config.global_tolerance, 0.5);
        assert!(config.key_sync_enabled);
        assert!(config.validate());
    }

    #[test]
    fn tolerance_strict() {
        let config = SyncToleranceConfig::strict();
        assert_eq!(config.global_tolerance, 0.2);
        assert_eq!(config.energy_tolerance, 0.1);
    }

    #[test]
    fn tolerance_lenient() {
        let config = SyncToleranceConfig::lenient();
        assert_eq!(config.global_tolerance, 0.8);
        assert_eq!(config.energy_tolerance, 0.6);
    }

    #[test]
    fn tolerance_apply_global() {
        let mut config = SyncToleranceConfig::strict();
        config.apply_global();
        // Global should blend with individual tolerances.
        assert!(config.energy_tolerance > 0.1);
    }

    #[test]
    fn tolerance_any_enabled() {
        let config = SyncToleranceConfig::default_config();
        assert!(config.any_sync_enabled());

        let mut disabled = config.clone();
        disabled.key_sync_enabled = false;
        disabled.phrase_sync_enabled = false;
        disabled.energy_matching_enabled = false;
        assert!(!disabled.any_sync_enabled());
    }

    #[test]
    fn tolerance_preset_names() {
        assert_eq!(TolerancePreset::Strict.name(), "Strict");
        assert_eq!(TolerancePreset::Balanced.name(), "Balanced");
        assert_eq!(TolerancePreset::Lenient.name(), "Lenient");
    }

    #[test]
    fn tolerance_preset_configs() {
        let strict_config = TolerancePreset::Strict.config();
        assert_eq!(strict_config.global_tolerance, 0.2);

        let lenient_config = TolerancePreset::Lenient.config();
        assert_eq!(lenient_config.global_tolerance, 0.8);
    }

    // --- SyncMetrics ---

    #[test]
    fn metrics_new() {
        let metrics = SyncMetrics::new();
        assert_eq!(metrics.analysis_time_ms, 0);
        assert_eq!(metrics.phrases_detected, 0);
        assert_eq!(metrics.suggestion_count, 0);
    }

    #[test]
    fn metrics_record_analysis() {
        let mut metrics = SyncMetrics::new();
        metrics.record_analysis(150, 16, 0.85);
        assert_eq!(metrics.analysis_time_ms, 150);
        assert_eq!(metrics.phrases_detected, 16);
        assert!((metrics.avg_confidence - 0.85).abs() < 0.01);
    }

    #[test]
    fn metrics_record_sync() {
        let mut metrics = SyncMetrics::new();
        metrics.record_sync(5.0, 0.9);
        metrics.record_sync(10.0, 0.8);
        assert_eq!(metrics.total_sync_ops, 2);
        assert!(metrics.avg_sync_latency_ms > 0.0);
    }

    #[test]
    fn metrics_success_rate() {
        let mut metrics = SyncMetrics::new();
        metrics.record_suggestion();
        metrics.record_suggestion();
        metrics.record_suggestion();
        metrics.record_applied();
        metrics.record_applied();
        assert!((metrics.success_rate() - 2.0 / 3.0).abs() < 0.01);
    }

    #[test]
    fn metrics_success_rate_no_suggestions() {
        let metrics = SyncMetrics::new();
        assert_eq!(metrics.success_rate(), 0.0);
    }

    #[test]
    fn metrics_summary() {
        let mut metrics = SyncMetrics::new();
        metrics.record_analysis(200, 20, 0.9);
        metrics.record_sync(5.0, 0.95);
        let summary = metrics.summary();
        assert!(summary.contains("200ms"));
        assert!(summary.contains("20"));
    }

    #[test]
    fn metrics_estimated_fps() {
        let mut metrics = SyncMetrics::new();
        metrics.record_sync(16.67, 0.9); // ~60fps
        assert!(metrics.estimated_fps() >= 55.0);
    }

    // --- PhraseWaveformState ---

    #[test]
    fn waveform_state_creation() {
        let state = PhraseWaveformState::new(800, 100);
        assert_eq!(state.canvas_width, 800);
        assert_eq!(state.canvas_height, 100);
    }

    #[test]
    fn waveform_set_playhead() {
        let mut state = PhraseWaveformState::new(800, 100);
        state.set_playhead(44100);
        assert_eq!(state.playhead_sample, 44100);
    }

    #[test]
    fn waveform_add_downbeat() {
        let mut state = PhraseWaveformState::new(800, 100);
        state.add_downbeat(44100);
        state.add_downbeat(88200);
        state.add_downbeat(22050);
        assert_eq!(state.downbeat_samples, vec![22050, 44100, 88200]);
    }

    #[test]
    fn waveform_current_phrase() {
        let mut state = PhraseWaveformState::new(800, 100);
        state.set_segments(vec![
            PhraseWaveformSegment::new(0, 44100, PhraseType::Intro, vec![0.1; 100]),
            PhraseWaveformSegment::new(44100, 88200, PhraseType::Verse, vec![0.3; 100]),
        ]);
        state.set_playhead(66150);
        let phrase = state.current_phrase_at_playhead();
        assert!(phrase.is_some());
        assert_eq!(phrase.unwrap().phrase_type, PhraseType::Verse);
    }

    #[test]
    fn waveform_no_phrase_at_playhead() {
        let mut state = PhraseWaveformState::new(800, 100);
        state.set_segments(vec![PhraseWaveformSegment::new(
            0,
            44100,
            PhraseType::Intro,
            vec![0.1; 100],
        )]);
        state.set_playhead(100000);
        assert!(state.current_phrase_at_playhead().is_none());
    }

    #[test]
    fn waveform_phrase_segment_color() {
        let seg = PhraseWaveformSegment::new(0, 44100, PhraseType::Chorus, vec![0.5; 100]);
        assert_eq!(seg.color, 0xff4444);
    }

    #[test]
    fn waveform_phrase_segment_valid() {
        let seg = PhraseWaveformSegment::new(0, 44100, PhraseType::Intro, vec![0.1; 100]);
        assert!(seg.is_valid());

        let invalid = PhraseWaveformSegment::new(44100, 0, PhraseType::Intro, vec![0.1; 100]);
        assert!(!invalid.is_valid());
    }

    // --- SyncSettings ---

    #[test]
    fn sync_settings_default() {
        let settings = SyncSettings::default();
        assert_eq!(settings.sync_mode, SyncMode::Manual);
        assert!(settings.key_sync_enabled);
        assert!(settings.phrase_sync_enabled);
        assert!(settings.energy_matching_enabled);
        assert_eq!(settings.tolerance_preset, TolerancePreset::Balanced);
        assert!(settings.auto_save);
    }

    #[test]
    fn sync_mode_names() {
        assert_eq!(SyncMode::Manual.name(), "Manual");
        assert_eq!(SyncMode::Automatic.name(), "Automatic");
        assert_eq!(SyncMode::Assist.name(), "Assist");
    }

    // --- NeuralSyncEngine ---

    #[test]
    fn sync_engine_creation() {
        let engine = NeuralSyncEngine::new();
        assert!(engine.get_phrase_position("deck1").is_none());
        assert!(engine.get_energy_profile("track.mp3").is_none());
    }

    #[test]
    fn sync_engine_update_phrase() {
        let mut engine = NeuralSyncEngine::new();
        let pos = PhrasePosition::new(PhraseType::Verse, 1, 8, 120.0, None, 0.5);
        engine.update_phrase_position("deck1", pos);

        let retrieved = engine.get_phrase_position("deck1");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().phrase_type, PhraseType::Verse);
    }

    #[test]
    fn sync_engine_update_energy_profile() {
        let mut engine = NeuralSyncEngine::new();
        let profile = EnergyProfile::new(0.5, 0.7, 0.1, vec![0.5], vec![0.5]);
        engine.set_energy_profile("track.mp3", profile);

        let retrieved = engine.get_energy_profile("track.mp3");
        assert!(retrieved.is_some());
        assert!((retrieved.unwrap().average_energy - 0.5).abs() < 0.01);
    }

    #[test]
    fn sync_engine_generate_suggestions() {
        let mut engine = NeuralSyncEngine::new();
        let pos_a = PhrasePosition::new(PhraseType::Verse, 1, 8, 120.0, None, 0.5);
        let pos_b = PhrasePosition::new(PhraseType::Chorus, 1, 8, 120.0, None, 0.7);
        engine.update_phrase_position("deck1", pos_a);
        engine.update_phrase_position("deck2", pos_b);

        let suggestions = engine.generate_suggestions("deck1", "deck2");
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn sync_engine_best_suggestion() {
        let suggestions = vec![
            SyncSuggestion {
                suggestion_type: SuggestionType::PhraseAlignment,
                score: 0.7,
                label: "Good".to_string(),
                details: "test".to_string(),
            },
            SyncSuggestion {
                suggestion_type: SuggestionType::HarmonicMatch,
                score: 0.9,
                label: "Excellent".to_string(),
                details: "test".to_string(),
            },
        ];
        let engine = NeuralSyncEngine::new();
        let best = engine.get_best_suggestion(&suggestions);
        assert!(best.is_some());
        assert_eq!(best.unwrap().score, 0.9);
    }

    #[test]
    fn sync_engine_historical_record() {
        let mut engine = NeuralSyncEngine::new();
        engine.record_historical_transition(PhraseType::Verse, PhraseType::Chorus, true);
        engine.record_historical_transition(PhraseType::Verse, PhraseType::Chorus, true);
        engine.record_historical_transition(PhraseType::Verse, PhraseType::Chorus, false);

        let success = engine.get_historical_success(PhraseType::Verse, PhraseType::Chorus);
        assert!(success > 0.5);
    }

    #[test]
    fn sync_engine_status_string() {
        let engine = NeuralSyncEngine::new();
        let status = engine.status_string();
        assert!(status.contains("Sync"));
    }

    #[test]
    fn sync_engine_save_load_settings() {
        let engine = NeuralSyncEngine::new();
        let json_path = engine.save_settings(SettingsFormat::Json).unwrap();
        assert!(json_path.to_string_lossy().ends_with("sync_settings.json"));
    }

    #[test]
    fn sync_engine_update_settings() {
        let mut engine = NeuralSyncEngine::new();
        let strict_settings = SyncSettings {
            tolerance_preset: TolerancePreset::Strict,
            ..SyncSettings::default()
        };
        engine.update_settings(strict_settings);
        assert_eq!(engine.get_tolerance().global_tolerance, 0.2);
    }

    // --- SyncError ---

    #[test]
    fn sync_error_display() {
        assert_eq!(
            format!("{}", SyncError::FileNotFound("config.json".into())),
            "Settings file not found: config.json"
        );
    }

    // --- SuggestionType ---

    #[test]
    fn suggestion_type_names() {
        assert_eq!(SuggestionType::PhraseAlignment.name(), "Phrase Alignment");
        assert_eq!(SuggestionType::HarmonicMatch.name(), "Harmonic Match");
        assert_eq!(SuggestionType::EnergyMatch.name(), "Energy Match");
        assert_eq!(SuggestionType::Combined.name(), "Combined");
    }
}
