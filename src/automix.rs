//! Automix — automatic beatmatching and crossfade between tracks.
//!
//! Analyzes tracks for energy/genre compatibility and generates
//! smooth crossfade transitions. Part of the Automix feature (P09a/P09b).
//!
//! ## Design principles
//!
//! * **Energy-aware** — Match tracks with similar energy levels.
//! * **Genre-aware** — Prefer matching genre/mood.
//! * **Smooth transitions** — No abrupt volume drops or clashes.
//! * **User override** — Manual adjustment always possible.

use std::fmt;
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Automix transition scoring result.
#[derive(Debug, Clone)]
pub struct TransitionScore {
    /// Overall compatibility score (0.0 - 1.0).
    pub score: f64,
    /// Energy compatibility score.
    pub energy_score: f64,
    /// Genre compatibility score.
    pub genre_score: f64,
    /// BPM compatibility score.
    pub bpm_score: f64,
    /// Key harmony compatibility score.
    pub key_score: f64,
}

impl TransitionScore {
    /// Whether this transition is suitable for automix.
    pub fn is_suitable(&self) -> bool {
        self.score >= 0.4
    }

    /// Whether this is a high-quality transition.
    pub fn is_excellent(&self) -> bool {
        self.score >= 0.8
    }

    /// Format for display.
    pub fn display(&self) -> String {
        format!(
            "Score: {:.0}% | Energy: {:.0}% | Genre: {:.0}% | BPM: {:.0}% | Key: {:.0}%",
            self.score * 100.0,
            self.energy_score * 100.0,
            self.genre_score * 100.0,
            self.bpm_score * 100.0,
            self.key_score * 100.0
        )
    }
}

/// Crossfade configuration.
#[derive(Debug, Clone)]
pub struct CrossfadeConfig {
    /// Crossfade duration in milliseconds.
    pub duration_ms: f64,
    /// Crossfade curve type.
    pub curve: CrossfadeCurve,
    /// Whether to match BPM during crossfade.
    pub bpm_match: bool,
    /// Whether to match key during crossfade.
    pub key_match: bool,
}

impl Default for CrossfadeConfig {
    fn default() -> Self {
        Self {
            duration_ms: 16000.0, // 16 seconds
            curve: CrossfadeCurve::Linear,
            bpm_match: true,
            key_match: true,
        }
    }
}

/// Crossfade curve shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossfadeCurve {
    Linear,
    EqualPower,
    Sine,
    Logarithmic,
}

impl fmt::Display for CrossfadeCurve {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CrossfadeCurve::Linear => write!(f, "linear"),
            CrossfadeCurve::EqualPower => write!(f, "equal-power"),
            CrossfadeCurve::Sine => write!(f, "sine"),
            CrossfadeCurve::Logarithmic => write!(f, "logarithmic"),
        }
    }
}

/// Automix mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomixMode {
    /// Automatically mix in the next track when current is nearly done.
    FullAuto,
    /// Suggest transitions but wait for confirmation.
    Suggest,
    /// Manual mixing only (automix disabled).
    Manual,
}

impl fmt::Display for AutomixMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutomixMode::FullAuto => write!(f, "full-auto"),
            AutomixMode::Suggest => write!(f, "suggest"),
            AutomixMode::Manual => write!(f, "manual"),
        }
    }
}

/// Automix recommendation.
#[derive(Debug, Clone)]
pub struct AutomixRecommendation {
    /// Track ID to mix into.
    pub next_track_id: u64,
    /// Track title.
    pub next_track_title: String,
    /// Transition score.
    pub score: f64,
    /// Recommended crossfade start position (in samples from current track end).
    pub crossfade_start_offset: i64,
    /// Recommended BPM adjustment for the next track.
    pub bpm_adjustment: f64,
}

impl AutomixRecommendation {
    /// Format recommendation for display.
    pub fn display(&self) -> String {
        format!(
            "Mix into: {} | Score: {:.0}% | BPM adj: {:.2}% | Crossfade in {}s",
            self.next_track_title,
            self.score * 100.0,
            self.bpm_adjustment * 100.0,
            self.crossfade_start_offset.abs() / 44100,
        )
    }
}

/// Automix configuration.
#[derive(Clone)]
pub struct AutomixConfig {
    /// Current automix mode.
    pub mode: AutomixMode,
    /// Crossfade settings.
    pub crossfade: CrossfadeConfig,
    /// Minimum score to auto-mix (0.0 - 1.0).
    pub min_score: f64,
    /// Maximum BPM difference for matching.
    pub max_bpm_diff: f64,
    /// Weight for energy matching (0.0 - 1.0).
    pub energy_weight: f64,
    /// Weight for genre matching (0.0 - 1.0).
    pub genre_weight: f64,
    /// Weight for BPM matching (0.0 - 1.0).
    pub bpm_weight: f64,
    /// Weight for key harmony (0.0 - 1.0).
    pub key_weight: f64,
}

impl Default for AutomixConfig {
    fn default() -> Self {
        Self {
            mode: AutomixMode::FullAuto,
            crossfade: CrossfadeConfig::default(),
            min_score: 0.5,
            max_bpm_diff: 8.0,
            energy_weight: 0.25,
            genre_weight: 0.25,
            bpm_weight: 0.25,
            key_weight: 0.25,
        }
    }
}

/// Crossfade state during an active mix.
pub struct CrossfadeState {
    /// Whether a crossfade is currently active.
    pub active: bool,
    /// Crossfade start position (sample index).
    pub start_position: u64,
    /// Crossfade duration (in samples).
    pub duration_samples: u64,
    /// Current crossfade position.
    pub current_position: u64,
    /// Source deck gain (decreasing).
    pub source_gain: f64,
    /// Destination deck gain (increasing).
    pub dest_gain: f64,
}

impl CrossfadeState {
    /// Get the current source deck gain for equal-power crossfade.
    pub fn equal_power_source_gain(&self) -> f64 {
        if self.duration_samples == 0 {
            return 0.0;
        }
        let progress = self.current_position as f64 / self.duration_samples as f64;
        (1.0 - progress).sqrt()
    }

    /// Get the current destination deck gain for equal-power crossfade.
    pub fn equal_power_dest_gain(&self) -> f64 {
        if self.duration_samples == 0 {
            return 0.0;
        }
        let progress = self.current_position as f64 / self.duration_samples as f64;
        progress.sqrt()
    }

    /// Whether the crossfade is complete.
    pub fn is_complete(&self) -> bool {
        self.current_position >= self.duration_samples
    }

    /// Advance the crossfade by the given number of samples.
    pub fn advance(&mut self, samples: u64) {
        self.current_position = self.current_position.saturating_add(samples);
        if self.current_position >= self.duration_samples {
            self.current_position = self.duration_samples;
        }
    }
}

impl Default for CrossfadeState {
    fn default() -> Self {
        Self {
            active: false,
            start_position: 0,
            duration_samples: 0,
            current_position: 0,
            source_gain: 1.0,
            dest_gain: 0.0,
        }
    }
}

/// Automix engine — manages transitions between tracks.
pub struct AutomixEngine {
    config: Arc<Mutex<AutomixConfig>>,
    crossfade_state: Arc<Mutex<CrossfadeState>>,
}

impl AutomixEngine {
    /// Create a new automix engine.
    pub fn new() -> Self {
        Self {
            config: Arc::new(Mutex::new(AutomixConfig::default())),
            crossfade_state: Arc::new(Mutex::new(CrossfadeState::default())),
        }
    }

    /// Calculate transition score between two tracks.
    ///
    /// ## Parameters
    /// - `track_a`: Current playing track info.
    /// - `track_b`: Candidate next track info.
    pub fn calculate_transition_score(
        &self,
        track_a: &TrackInfo,
        track_b: &TrackInfo,
    ) -> TransitionScore {
        let config = self.config.lock().unwrap();
        self.transition_score_with_config(track_a, track_b, &config)
    }

    fn transition_score_with_config(
        &self,
        track_a: &TrackInfo,
        track_b: &TrackInfo,
        config: &AutomixConfig,
    ) -> TransitionScore {
        let energy_score = self.score_energy(track_a, track_b);
        let genre_score = self.score_genre(track_a, track_b);
        let bpm_score = self.score_bpm(track_a, track_b, config);
        let key_score = self.score_key(track_a, track_b);

        let score = energy_score * config.energy_weight
            + genre_score * config.genre_weight
            + bpm_score * config.bpm_weight
            + key_score * config.key_weight;

        TransitionScore {
            score,
            energy_score,
            genre_score,
            bpm_score,
            key_score,
        }
    }

    /// Get an automix recommendation for the next track.
    ///
    /// Scans candidate tracks and returns the best match.
    pub fn get_recommendation(
        &self,
        current_track: &TrackInfo,
        candidates: &[TrackInfo],
    ) -> Option<AutomixRecommendation> {
        let config = self.config.lock().unwrap().clone();
        let mode = config.mode;

        // Manual mode returns no recommendations.
        if mode == AutomixMode::Manual {
            return None;
        }

        let mut best: Option<(TransitionScore, usize)> = None;

        for (i, candidate) in candidates.iter().enumerate() {
            let score = self.transition_score_with_config(current_track, candidate, &config);

            if !score.is_suitable() {
                continue;
            }

            match &best {
                Some((best_score, _)) if score.score <= best_score.score => continue,
                _ => {
                    best = Some((score, i));
                }
            }
        }

        let (score, idx) = best?;

        // Suggest mode: only recommend if score exceeds min_score.
        if mode == AutomixMode::Suggest && score.score < config.min_score {
            return None;
        }

        let candidate = &candidates[idx];

        // Calculate BPM adjustment.
        let bpm_adjustment = if config.crossfade.bpm_match {
            (current_track.bpm - candidate.bpm) / candidate.bpm
        } else {
            0.0
        };

        // Crossfade starts before current track ends.
        let crossfade_start_offset =
            -(config.crossfade.duration_ms / 1000.0 * current_track.sample_rate as f64) as i64;

        Some(AutomixRecommendation {
            next_track_id: candidate.id,
            next_track_title: candidate.title.clone(),
            score: score.score,
            crossfade_start_offset,
            bpm_adjustment,
        })
    }

    /// Start a crossfade between two tracks.
    pub fn start_crossfade(
        &self,
        source_position: u64,
        duration_samples: u64,
    ) -> Result<(), String> {
        let mut state = self.crossfade_state.lock().map_err(|e| e.to_string())?;

        if duration_samples == 0 {
            return Err("Crossfade duration must be > 0".to_string());
        }

        state.active = true;
        state.start_position = source_position;
        state.duration_samples = duration_samples;
        state.current_position = 0;

        Ok(())
    }

    /// Advance the crossfade by the given number of samples.
    pub fn advance_crossfade(&self, samples: u64) -> (f64, f64) {
        let mut state = self.crossfade_state.lock().unwrap();

        if !state.active {
            return (1.0, 0.0);
        }

        let config = self.config.lock().unwrap();

        match config.crossfade.curve {
            CrossfadeCurve::EqualPower => {
                state.advance(samples);
                (
                    state.equal_power_source_gain(),
                    state.equal_power_dest_gain(),
                )
            }
            CrossfadeCurve::Linear => {
                state.advance(samples);
                let progress = state.current_position as f64 / state.duration_samples as f64;
                (1.0 - progress, progress)
            }
            CrossfadeCurve::Sine => {
                state.advance(samples);
                let progress = state.current_position as f64 / state.duration_samples as f64;
                (
                    ((1.0 - progress) * (1.0 - progress).atan() * 2.0 / std::f64::consts::PI
                        + 0.5 * 2.0)
                        .clamp(0.0, 1.0),
                    (progress * progress.atan() * 2.0 / std::f64::consts::PI + 0.5 * 2.0)
                        .clamp(0.0, 1.0),
                )
            }
            CrossfadeCurve::Logarithmic => {
                state.advance(samples);
                let progress = state.current_position as f64 / state.duration_samples as f64;
                (1.0 - progress.ln_1p(), progress.ln_1p().ln().abs())
            }
        }
    }

    /// Check if a crossfade is active.
    pub fn is_crossfade_active(&self) -> bool {
        self.crossfade_state
            .lock()
            .map(|s| s.active)
            .unwrap_or(false)
    }

    /// Check if crossfade is complete.
    pub fn is_crossfade_complete(&self) -> bool {
        self.crossfade_state
            .lock()
            .map(|s| s.active && s.is_complete())
            .unwrap_or(false)
    }

    /// Stop an active crossfade.
    pub fn stop_crossfade(&self) -> Result<(), String> {
        let mut state = self.crossfade_state.lock().map_err(|e| e.to_string())?;
        state.active = false;
        state.current_position = 0;
        Ok(())
    }

    /// Update automix configuration.
    pub fn set_config(&self, config: AutomixConfig) -> Result<(), String> {
        if config.energy_weight + config.genre_weight + config.bpm_weight + config.key_weight > 1.01
        {
            return Err("Weight sum must be <= 1.0".to_string());
        }
        if config.min_score < 0.0 || config.min_score > 1.0 {
            return Err("min_score must be 0.0 - 1.0".to_string());
        }
        *self.config.lock().map_err(|e| e.to_string())? = config;
        Ok(())
    }

    /// Get current configuration.
    pub fn get_config(&self) -> Result<AutomixConfig, String> {
        self.config
            .lock()
            .map(|c| c.clone())
            .map_err(|e| e.to_string())
    }

    /// Score energy compatibility.
    fn score_energy(&self, a: &TrackInfo, b: &TrackInfo) -> f64 {
        let energy_diff = (a.energy.unwrap_or(0.5) - b.energy.unwrap_or(0.5)).abs();
        1.0 - energy_diff.clamp(0.0, 1.0)
    }

    /// Score genre compatibility.
    fn score_genre(&self, a: &TrackInfo, b: &TrackInfo) -> f64 {
        if a.genre == b.genre {
            return 1.0;
        }
        // Check genre compatibility map.
        if is_genre_compatible(&a.genre, &b.genre) {
            return 0.7;
        }
        0.3
    }

    /// Score BPM compatibility.
    fn score_bpm(&self, a: &TrackInfo, b: &TrackInfo, config: &AutomixConfig) -> f64 {
        let bpm_diff = (a.bpm - b.bpm).abs();
        if bpm_diff <= config.max_bpm_diff {
            1.0 - (bpm_diff / config.max_bpm_diff) * 0.3
        } else {
            0.3
        }
    }

    /// Score key harmony compatibility.
    fn score_key(&self, a: &TrackInfo, b: &TrackInfo) -> f64 {
        match (&a.key, &b.key) {
            (Some(key_a), Some(key_b)) if key_a == key_b => 1.0,
            (Some(key_a), Some(key_b)) if is_key_compatible(key_a, key_b) => 0.8,
            _ => 0.5,
        }
    }
}

impl Default for AutomixEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Track information for automix scoring.
#[derive(Debug, Clone)]
pub struct TrackInfo {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub bpm: f64,
    pub key: Option<String>,
    pub energy: Option<f64>,
    pub genre: String,
    pub duration_samples: u64,
    pub sample_rate: u32,
}

/// Check if two genres are compatible for mixing.
fn is_genre_compatible(genre_a: &str, genre_b: &str) -> bool {
    // Define genre compatibility groups.
    let groups: &[&[&str]] = &[
        &["house", "tech-house", "deep-house", "progressive-house"],
        &["techno", "minimal-techno", "melodic-techno"],
        &["trance", "progressive-trance", "vocal-trance"],
        &["dubstep", "brostep", "riddim"],
        &["drum-and-bass", "jungle", "liquid-dnb"],
        &["hip-hop", "rap", "trap"],
        &["pop", "dance-pop", "electropop"],
    ];

    let genre_a_lower = genre_a.to_lowercase();
    let genre_b_lower = genre_b.to_lowercase();

    for group in groups {
        let a_in_group = group.iter().any(|g| g.to_lowercase() == genre_a_lower);
        let b_in_group = group.iter().any(|g| g.to_lowercase() == genre_b_lower);
        if a_in_group && b_in_group {
            return true;
        }
    }

    // Cross-genre compatibility (simplified).
    let compatible_pairs = [
        ("house", "techno"),
        ("techno", "minimal-techno"),
        ("hip-hop", "trap"),
        ("dubstep", "riddim"),
        ("drum-and-bass", "jungle"),
    ];

    compatible_pairs.iter().any(|(a, b)| {
        (a == &genre_a_lower && b == &genre_b_lower) || (b == &genre_a_lower && a == &genre_b_lower)
    })
}

/// Check if two keys are harmonically compatible.
fn is_key_compatible(key_a: &str, key_b: &str) -> bool {
    // Simple Camelot wheel compatibility.
    // Neighboring keys on the wheel are compatible.
    // Same key is handled by the caller.

    let parse_cam = |key: &str| -> Option<(u8, char)> {
        let (number, side) = if let Some(number) = key.strip_suffix('A') {
            (number, 'A')
        } else {
            let number = key.strip_suffix('B')?;
            (number, 'B')
        };
        let number = number.parse::<u8>().ok()?;
        (1..=12).contains(&number).then_some((number, side))
    };

    let (num_a, letter_a) = match parse_cam(key_a) {
        Some(v) => v,
        None => return false,
    };
    let (num_b, letter_b) = match parse_cam(key_b) {
        Some(v) => v,
        None => return false,
    };

    // Same letter, adjacent numbers.
    if letter_a == letter_b {
        let difference = num_a.abs_diff(num_b);
        return matches!(difference, 0 | 1 | 11);
    }

    // Different letter but compatible (A/B pairs on Camelot wheel).
    // A and B at same number are relative major/minor.
    num_a == num_b
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_track(
        id: u64,
        title: &str,
        bpm: f64,
        key: &str,
        energy: f64,
        genre: &str,
    ) -> TrackInfo {
        TrackInfo {
            id,
            title: title.to_string(),
            artist: "Test Artist".to_string(),
            bpm,
            key: Some(key.to_string()),
            energy: Some(energy),
            genre: genre.to_string(),
            duration_samples: 441000,
            sample_rate: 44100,
        }
    }

    #[test]
    fn transition_score_identical_tracks() {
        let engine = AutomixEngine::new();
        let track_a = test_track(1, "Track A", 128.0, "8A", 0.7, "house");
        let track_b = test_track(2, "Track B", 128.0, "8A", 0.7, "house");

        let score = engine.calculate_transition_score(&track_a, &track_b);
        assert!(score.score > 0.9);
        assert!(score.is_excellent());
    }

    #[test]
    fn transition_score_different_genres() {
        let engine = AutomixEngine::new();
        let track_a = test_track(1, "Track A", 128.0, "8A", 0.7, "house");
        let track_b = test_track(2, "Track B", 170.0, "5B", 0.9, "drum-and-bass");

        let score = engine.calculate_transition_score(&track_a, &track_b);
        assert!(score.score < 0.6);
    }

    #[test]
    fn transition_score_key_match() {
        let engine = AutomixEngine::new();
        let track_a = test_track(1, "A", 128.0, "8A", 0.5, "house");
        let track_b = test_track(2, "B", 129.0, "8A", 0.5, "techno");

        let score = engine.calculate_transition_score(&track_a, &track_b);
        assert!(score.key_score >= 0.8);
    }

    #[test]
    fn transition_score_compatible_keys() {
        let engine = AutomixEngine::new();
        let track_a = test_track(1, "A", 128.0, "8A", 0.5, "house");
        let track_b = test_track(2, "B", 128.0, "9A", 0.5, "house");

        let score = engine.calculate_transition_score(&track_a, &track_b);
        assert!(score.key_score >= 0.8);
    }

    #[test]
    fn transition_score_bpm_diff() {
        let engine = AutomixEngine::new();
        let config = AutomixConfig {
            max_bpm_diff: 4.0,
            ..Default::default()
        };
        engine.set_config(config).unwrap();

        let track_a = test_track(1, "A", 128.0, "8A", 0.5, "house");
        let track_b = test_track(2, "B", 132.0, "8A", 0.5, "house"); // 4 BPM diff.

        let score = engine.calculate_transition_score(&track_a, &track_b);
        assert!((score.bpm_score - 0.7).abs() < 1e-12);
    }

    #[test]
    fn crossfade_equal_power() {
        let engine = AutomixEngine::new();
        let mut config = AutomixConfig::default();
        config.crossfade.curve = CrossfadeCurve::EqualPower;
        engine.set_config(config).unwrap();
        engine
            .start_crossfade(0, 44100) // 1 second at 44.1kHz.
            .unwrap();

        // Advance halfway.
        let (src_gain, dest_gain) = engine.advance_crossfade(22050);
        assert!((src_gain - 0.707).abs() < 0.01); // sqrt(0.5)
        assert!((dest_gain - 0.707).abs() < 0.01);

        // Advance to end.
        let (src_gain, dest_gain) = engine.advance_crossfade(22050);
        assert!(src_gain < 0.1);
        assert!(dest_gain > 0.9);
        assert!(engine.is_crossfade_complete());
    }

    #[test]
    fn crossfade_linear() {
        let config = AutomixConfig {
            crossfade: CrossfadeConfig {
                curve: CrossfadeCurve::Linear,
                ..Default::default()
            },
            ..Default::default()
        };
        let engine = AutomixEngine::new();
        engine.set_config(config).unwrap();

        engine.start_crossfade(0, 1000).unwrap();

        let (src_gain, dest_gain) = engine.advance_crossfade(500);
        assert!((src_gain - 0.5).abs() < 0.1);
        assert!((dest_gain - 0.5).abs() < 0.1);
    }

    #[test]
    fn crossfade_complete() {
        let engine = AutomixEngine::new();
        engine.start_crossfade(0, 100).unwrap();

        assert!(engine.is_crossfade_active());
        assert!(!engine.is_crossfade_complete());

        engine.advance_crossfade(100);
        assert!(engine.is_crossfade_complete());

        engine.stop_crossfade().unwrap();
        assert!(!engine.is_crossfade_active());
    }

    #[test]
    fn automix_recommendation() {
        let engine = AutomixEngine::new();

        let current = test_track(1, "Current", 128.0, "8A", 0.6, "house");
        let candidates = vec![
            test_track(2, "Good Match", 128.0, "8A", 0.6, "house"),
            test_track(3, "Bad Match", 170.0, "5B", 0.9, "drum-and-bass"),
            test_track(4, "Okay", 130.0, "9A", 0.7, "techno"),
        ];

        let rec = engine.get_recommendation(&current, &candidates).unwrap();
        assert_eq!(rec.next_track_id, 2); // Good match should win.
        assert!(rec.score > 0.7);
    }

    #[test]
    fn automix_no_recommendation_manual() {
        let engine = AutomixEngine::new();
        engine
            .set_config(AutomixConfig {
                mode: AutomixMode::Manual,
                ..Default::default()
            })
            .unwrap();

        let current = test_track(1, "Current", 128.0, "8A", 0.6, "house");
        let candidates = vec![test_track(2, "Track", 128.0, "8A", 0.6, "house")];

        assert!(engine.get_recommendation(&current, &candidates).is_none());
    }

    #[test]
    fn automix_config_validation() {
        let engine = AutomixEngine::new();

        // Valid config.
        assert!(engine.set_config(AutomixConfig::default()).is_ok());

        // Invalid weight sum.
        let bad_config = AutomixConfig {
            energy_weight: 0.5,
            genre_weight: 0.5,
            bpm_weight: 0.3,
            key_weight: 0.0,
            ..Default::default()
        };
        assert!(engine.set_config(bad_config).is_err());

        // Invalid min_score.
        let bad_config = AutomixConfig {
            min_score: 1.5,
            ..Default::default()
        };
        assert!(engine.set_config(bad_config).is_err());
    }

    #[test]
    fn genre_compatibility() {
        assert!(is_genre_compatible("house", "house"));
        assert!(is_genre_compatible("house", "tech-house"));
        assert!(is_genre_compatible("techno", "minimal-techno"));
        assert!(is_genre_compatible("hip-hop", "trap"));
        assert!(!is_genre_compatible("house", "drum-and-bass"));
    }

    #[test]
    fn key_compatibility() {
        assert!(is_key_compatible("8A", "8A"));
        assert!(is_key_compatible("8A", "9A"));
        assert!(is_key_compatible("8A", "7A"));
        assert!(is_key_compatible("8A", "8B")); // Relative major/minor
        assert!(!is_key_compatible("8A", "3C")); // Invalid
        assert!(!is_key_compatible("8A", "10B")); // Non-adjacent
    }

    #[test]
    fn crossfade_zero_duration_rejected() {
        let engine = AutomixEngine::new();
        assert!(engine.start_crossfade(0, 0).is_err());
    }

    #[test]
    fn crossfade_active_initially() {
        let engine = AutomixEngine::new();
        assert!(!engine.is_crossfade_active());

        engine.start_crossfade(0, 1000).unwrap();
        assert!(engine.is_crossfade_active());

        engine.stop_crossfade().unwrap();
        assert!(!engine.is_crossfade_active());
    }

    #[test]
    fn automix_recommendation_score_threshold() {
        let engine = AutomixEngine::new();
        engine
            .set_config(AutomixConfig {
                mode: AutomixMode::Suggest,
                min_score: 0.9,
                ..Default::default()
            })
            .unwrap();

        let current = test_track(1, "Current", 128.0, "8A", 0.6, "house");
        let candidates = vec![test_track(2, "Okay", 130.0, "9A", 0.8, "techno")];

        // Score might not reach 0.9 threshold.
        let rec = engine.get_recommendation(&current, &candidates);
        // Could be Some or None depending on scores.
        if let Some(r) = rec {
            assert!(r.score >= 0.9);
        }
    }
}
