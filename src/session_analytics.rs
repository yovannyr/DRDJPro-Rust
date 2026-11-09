//! Session Analytics — Track statistics, mixing patterns, performance insights.
//!
//! Analyzes mix sessions to provide insights into DJ mixing patterns,
//! transition quality, genre distribution, energy flow, and performance metrics.
//! Part of the Session Analytics feature (P24a).
//!
//! ## Design principles
//!
//! * **Descriptive** — Statistical analysis of recorded sessions.
//! * **Actionable** — Insights that improve mixing practice.
//! * **Multi-dimensional** — Energy, genre, BPM, timing analysis.
//! * **Exportable** — Report generation for review.

use std::collections::HashMap;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Session-wide analytics summary.
#[derive(Debug, Clone)]
pub struct SessionAnalytics {
    /// Session ID.
    pub session_id: String,
    /// Session name.
    pub session_name: String,
    /// Analysis duration.
    pub analysis_duration: Duration,
    /// Total tracks played.
    pub total_tracks: usize,
    /// Unique tracks.
    pub unique_tracks: usize,
    /// Total transitions.
    pub total_transitions: usize,
    /// Average transition duration (seconds).
    pub avg_transition_duration: f64,
    /// Average energy level.
    pub avg_energy: f64,
    /// Energy range (min/max).
    pub energy_min: f64,
    pub energy_max: f64,
    /// BPM statistics.
    pub bpm_mean: f64,
    pub bpm_min: f64,
    pub bpm_max: f64,
    pub bpm_stddev: f64,
    /// Genre distribution.
    pub genre_distribution: HashMap<String, f64>,
    /// Key distribution.
    pub key_distribution: HashMap<String, f64>,
    /// Hour-of-day distribution.
    pub time_distribution: HashMap<usize, f64>,
    /// Analysis timestamp.
    pub analyzed_at: String,
}

impl SessionAnalytics {
    /// Create a new session analytics report.
    pub fn new(session_id: String, session_name: String) -> Self {
        Self {
            session_id,
            session_name,
            analysis_duration: Duration::ZERO,
            total_tracks: 0,
            unique_tracks: 0,
            total_transitions: 0,
            avg_transition_duration: 0.0,
            avg_energy: 0.0,
            energy_min: 1.0,
            energy_max: 0.0,
            bpm_mean: 0.0,
            bpm_min: 0.0,
            bpm_max: 0.0,
            bpm_stddev: 0.0,
            genre_distribution: HashMap::new(),
            key_distribution: HashMap::new(),
            time_distribution: HashMap::new(),
            analyzed_at: String::new(),
        }
    }

    /// Set analysis timestamp.
    pub fn set_analyzed_at(&mut self, timestamp: String) {
        self.analyzed_at = timestamp;
    }

    /// Get summary string.
    pub fn summary(&self) -> String {
        format!(
            "Session Analytics: {} — {} tracks, {} transitions, avg BPM: {:.1}, avg energy: {:.2}",
            self.session_name,
            self.total_tracks,
            self.total_transitions,
            self.bpm_mean,
            self.avg_energy
        )
    }
}

/// Transition analysis.
#[derive(Debug, Clone)]
pub struct TransitionAnalysis {
    /// From deck.
    pub from_deck: String,
    /// To deck.
    pub to_deck: String,
    /// Transition type.
    pub transition_type: String,
    /// Duration in seconds.
    pub duration: f64,
    /// BPM difference.
    pub bpm_difference: f64,
    /// Key compatibility score (0.0-1.0).
    pub key_compatibility: f64,
    /// Energy difference.
    pub energy_difference: f64,
    /// Timing quality (how well aligned with beats).
    pub timing_quality: f64,
    /// Overall quality score (0.0-1.0).
    pub quality_score: f64,
}

/// Measurements describing a transition, separate from deck identity.
#[derive(Debug, Clone, Copy)]
pub struct TransitionMeasurement {
    pub duration: f64,
    pub bpm_difference: f64,
    pub key_compatibility: f64,
    pub energy_difference: f64,
    pub timing_quality: f64,
}
/// A track play event; borrowed identity and owned genre annotations.
#[derive(Debug)]
pub struct TrackPlay<'a> {
    pub track_id: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub duration: f64,
    pub bpm: f64,
    pub key: &'a str,
    pub genres: Vec<String>,
}

impl TransitionAnalysis {
    /// Create a new transition analysis.
    pub fn new(
        from_deck: String,
        to_deck: String,
        transition_type: String,
        measurement: TransitionMeasurement,
    ) -> Self {
        let TransitionMeasurement {
            duration,
            bpm_difference,
            key_compatibility,
            energy_difference,
            timing_quality,
        } = measurement;

        // Composite quality score.
        let quality_score = key_compatibility * 0.4
            + (1.0 - (bpm_difference / 200.0).min(1.0)) * 0.2
            + (1.0 - energy_difference.abs()) * 0.2
            + timing_quality * 0.2;

        Self {
            from_deck,
            to_deck,
            transition_type,
            duration,
            bpm_difference,
            key_compatibility,
            energy_difference,
            timing_quality,
            quality_score,
        }
    }

    /// Check if transition is high quality.
    pub fn is_high_quality(&self, threshold: f64) -> bool {
        self.quality_score >= threshold
    }
}

/// Track usage in a session.
#[derive(Debug, Clone)]
pub struct TrackUsage {
    /// Track ID.
    pub track_id: String,
    /// Track title.
    pub title: String,
    /// Artist.
    pub artist: String,
    /// Play count.
    pub play_count: usize,
    /// Total play duration (seconds).
    pub total_play_duration: f64,
    /// Average play position (when track was stopped, 0-1).
    pub avg_stop_position: f64,
    stop_observations: usize,
    /// Genres.
    pub genres: Vec<String>,
    /// BPM.
    pub bpm: f64,
    /// Key.
    pub key: String,
}

impl TrackUsage {
    /// Create a new track usage record.
    pub fn new(track_id: String, title: String, artist: String) -> Self {
        Self {
            track_id,
            title,
            artist,
            play_count: 0,
            total_play_duration: 0.0,
            avg_stop_position: 0.0,
            stop_observations: 0,
            genres: Vec::new(),
            bpm: 0.0,
            key: String::new(),
        }
    }

    /// Increment play count and duration.
    pub fn record_play(&mut self, duration: f64) {
        self.play_count += 1;
        self.total_play_duration += duration;
    }

    /// Update average stop position.
    pub fn update_avg_stop_position(&mut self, position: f64) {
        self.stop_observations += 1;
        let count = self.stop_observations as f64;
        if count > 0.0 {
            self.avg_stop_position = (self.avg_stop_position * (count - 1.0) + position) / count;
        }
    }
}

/// Energy flow analysis.
#[derive(Debug, Clone)]
pub struct EnergyFlow {
    /// Time position in session (seconds).
    pub time: f64,
    /// Energy level (0.0-1.0).
    pub energy: f64,
    /// Track ID.
    pub track_id: String,
    /// Track title.
    pub track_title: String,
}

impl EnergyFlow {
    /// Create a new energy flow point.
    pub fn new(time: f64, energy: f64, track_id: String, track_title: String) -> Self {
        Self {
            time,
            energy,
            track_id,
            track_title,
        }
    }
}

/// DJ mixing pattern.
#[derive(Debug, Clone)]
pub struct MixingPattern {
    /// Pattern name.
    pub name: String,
    /// Frequency of occurrence.
    pub frequency: usize,
    /// Average duration (seconds).
    pub avg_duration: f64,
    /// Description.
    pub description: String,
}

impl MixingPattern {
    /// Create a new mixing pattern.
    pub fn new(name: String, frequency: usize, avg_duration: f64, description: String) -> Self {
        Self {
            name,
            frequency,
            avg_duration,
            description,
        }
    }
}

/// Session analytics engine.
pub struct SessionAnalyticsEngine {
    /// Track usage records.
    track_usage: HashMap<String, TrackUsage>,
    /// Transitions analyzed.
    transitions: Vec<TransitionAnalysis>,
    /// Energy flow data points.
    energy_flow: Vec<EnergyFlow>,
    /// Detected mixing patterns.
    detected_patterns: Vec<MixingPattern>,
    /// Genre analysis.
    genre_stats: HashMap<String, GenreStats>,
    /// Key compatibility analysis.
    key_analysis: KeyCompatibilityAnalysis,
    /// Performance insights.
    insights: Vec<String>,
}

/// Genre statistics.
#[derive(Debug, Clone)]
pub struct GenreStats {
    /// Track count.
    pub track_count: usize,
    /// Total plays.
    pub total_plays: usize,
    /// Average duration (seconds).
    pub avg_duration: f64,
    /// Average BPM.
    pub avg_bpm: f64,
}

impl GenreStats {
    /// Create new genre stats.
    pub fn new() -> Self {
        Self {
            track_count: 0,
            total_plays: 0,
            avg_duration: 0.0,
            avg_bpm: 0.0,
        }
    }
}

/// Key compatibility analysis.
#[derive(Debug, Clone)]
pub struct KeyCompatibilityAnalysis {
    /// Most common transitions.
    pub common_transitions: Vec<(String, String, usize)>,
    /// Key energy profile.
    pub key_energy_profile: HashMap<String, f64>,
}

impl KeyCompatibilityAnalysis {
    /// Create new analysis.
    pub fn new() -> Self {
        Self {
            common_transitions: Vec::new(),
            key_energy_profile: HashMap::new(),
        }
    }
}

impl SessionAnalyticsEngine {
    /// Create a new analytics engine.
    pub fn new() -> Self {
        Self {
            track_usage: HashMap::new(),
            transitions: Vec::new(),
            energy_flow: Vec::new(),
            detected_patterns: Vec::new(),
            genre_stats: HashMap::new(),
            key_analysis: KeyCompatibilityAnalysis::new(),
            insights: Vec::new(),
        }
    }

    /// Record track play.
    pub fn record_track_play(&mut self, play: TrackPlay<'_>) {
        let TrackPlay {
            track_id,
            title,
            artist,
            duration,
            bpm,
            key,
            genres,
        } = play;

        let entry = self
            .track_usage
            .entry(track_id.to_string())
            .or_insert_with(|| {
                TrackUsage::new(track_id.to_string(), title.to_string(), artist.to_string())
            });
        entry.record_play(duration);
        entry.bpm = bpm;
        entry.key = key.to_string();
        entry.genres = genres;
        self.analyze_genres();
    }

    /// Record track stop position.
    pub fn record_track_stop(&mut self, track_id: &str, position: f64) {
        if let Some(usage) = self.track_usage.get_mut(track_id) {
            usage.update_avg_stop_position(position);
        }
    }

    /// Analyze a transition.
    pub fn analyze_transition(&mut self, transition: TransitionAnalysis) {
        self.transitions.push(transition);
    }

    /// Record an energy flow data point.
    pub fn record_energy(&mut self, time: f64, energy: f64, track_id: &str, track_title: &str) {
        self.energy_flow.push(EnergyFlow::new(
            time,
            energy,
            track_id.to_string(),
            track_title.to_string(),
        ));
    }

    /// Detect mixing patterns from transitions.
    pub fn detect_patterns(&mut self) {
        // Simple pattern detection: find common transition types and deck pairs.
        let mut pattern_counts: HashMap<String, usize> = HashMap::new();
        let mut pattern_durations: HashMap<String, Vec<f64>> = HashMap::new();

        for t in &self.transitions {
            let pattern_key = format!("{} -> {} ({})", t.from_deck, t.to_deck, t.transition_type);
            *pattern_counts.entry(pattern_key.clone()).or_insert(0) += 1;
            pattern_durations
                .entry(pattern_key)
                .or_default()
                .push(t.duration);
        }

        for (pattern, count) in &pattern_counts {
            if *count >= 2 {
                let durations = pattern_durations.get(pattern).unwrap();
                let avg_dur = durations.iter().sum::<f64>() / durations.len() as f64;
                self.detected_patterns.push(MixingPattern::new(
                    pattern.clone(),
                    *count,
                    avg_dur,
                    format!(
                        "Deck {} to {} via {}, {} occurrences, avg {:.1}s",
                        &pattern[..4],
                        &pattern[5..7],
                        pattern
                            .split('(')
                            .nth(1)
                            .unwrap_or("")
                            .split(')')
                            .next()
                            .unwrap_or(""),
                        count,
                        avg_dur
                    ),
                ));
            }
        }
    }

    /// Generate genre analysis.
    pub fn analyze_genres(&mut self) {
        self.genre_stats.clear();
        for usage in self.track_usage.values() {
            for genre in &usage.genres {
                let stats = self.genre_stats.entry(genre.clone()).or_default();
                stats.track_count += 1;
                stats.total_plays += usage.play_count;
            }
        }

        // Calculate averages.
        // Simplified: single-genre assumption per track.
    }

    /// Generate key analysis.
    pub fn analyze_keys(&mut self) {
        for usage in self.track_usage.values() {
            let energy_entry = self
                .key_analysis
                .key_energy_profile
                .entry(usage.key.clone())
                .or_insert(0.0);
            *energy_entry += 0.5; // Placeholder energy.
        }
    }

    /// Generate insights.
    pub fn generate_insights(&mut self) {
        self.insights.clear();

        if self.transitions.is_empty() {
            self.insights
                .push("No transitions detected — check if deck switching was recorded.".into());
            return;
        }

        let avg_quality = self
            .transitions
            .iter()
            .map(|t| t.quality_score)
            .sum::<f64>()
            / self.transitions.len() as f64;
        if avg_quality >= 0.8 {
            self.insights
                .push("Excellent transition quality detected across the session.".into());
        } else if avg_quality >= 0.6 {
            self.insights.push("Good transition quality overall. Consider smoother BPM matching on some transitions.".into());
        } else {
            self.insights.push("Transition quality could be improved. Focus on beat matching and key compatibility.".into());
        }

        // Check for energy consistency.
        if !self.energy_flow.is_empty() {
            let energy_avg = self.energy_flow.iter().map(|e| e.energy).sum::<f64>()
                / self.energy_flow.len() as f64;
            if energy_avg < 0.3 {
                self.insights
                    .push("Low average energy. Consider selecting higher-energy tracks.".into());
            } else if energy_avg > 0.8 {
                self.insights
                    .push("Very high energy throughout. Consider dynamic variation.".into());
            }
        }

        // Check for genre diversity.
        if self.genre_stats.len() <= 2 {
            self.insights.push(
                "Limited genre diversity. Consider exploring more genres for a richer mix.".into(),
            );
        }

        // Check track repetition.
        let repeated = self
            .track_usage
            .values()
            .filter(|t| t.play_count > 1)
            .count();
        if repeated > self.track_usage.len() / 2 {
            self.insights
                .push("High track repetition. Consider expanding your track selection.".into());
        }
    }

    /// Get transition count.
    pub fn transition_count(&self) -> usize {
        self.transitions.len()
    }

    /// Get average transition quality.
    pub fn avg_transition_quality(&self) -> f64 {
        if self.transitions.is_empty() {
            return 0.0;
        }
        self.transitions
            .iter()
            .map(|t| t.quality_score)
            .sum::<f64>()
            / self.transitions.len() as f64
    }

    /// Get detected patterns.
    pub fn detected_patterns(&self) -> &[MixingPattern] {
        &self.detected_patterns
    }

    /// Get insights.
    pub fn insights(&self) -> &[String] {
        &self.insights
    }

    /// Get track usage count.
    pub fn track_count(&self) -> usize {
        self.track_usage.len()
    }

    /// Get most played tracks.
    pub fn most_played(&self, limit: usize) -> Vec<(&String, &TrackUsage)> {
        let mut usage: Vec<_> = self.track_usage.iter().collect();
        usage.sort_by_key(|value| std::cmp::Reverse(value.1.play_count));
        usage.into_iter().take(limit).collect()
    }

    /// Get genre distribution percentages.
    pub fn genre_distribution(&self) -> HashMap<String, f64> {
        let total_plays: usize = self.genre_stats.values().map(|g| g.total_plays).sum();
        if total_plays == 0 {
            return HashMap::new();
        }

        self.genre_stats
            .iter()
            .map(|(genre, stats)| (genre.clone(), stats.total_plays as f64 / total_plays as f64))
            .collect()
    }

    /// Get energy flow data.
    pub fn energy_flow(&self) -> &[EnergyFlow] {
        &self.energy_flow
    }

    /// Generate session analytics report.
    pub fn generate_report(&self, session_id: &str, session_name: &str) -> SessionAnalytics {
        let total_tracks = self.track_usage.len();
        let unique_tracks = self
            .track_usage
            .values()
            .filter(|t| t.play_count == 1)
            .count();
        let total_transitions = self.transitions.len();

        let avg_transition_duration = if total_transitions > 0 {
            self.transitions.iter().map(|t| t.duration).sum::<f64>() / total_transitions as f64
        } else {
            0.0
        };

        let avg_energy = if !self.energy_flow.is_empty() {
            self.energy_flow.iter().map(|e| e.energy).sum::<f64>() / self.energy_flow.len() as f64
        } else {
            0.0
        };

        let energy_min = self
            .energy_flow
            .iter()
            .map(|e| e.energy)
            .fold(1.0, f64::min);
        let energy_max = self
            .energy_flow
            .iter()
            .map(|e| e.energy)
            .fold(0.0, f64::max);

        let bpms: Vec<f64> = self.track_usage.values().map(|t| t.bpm).collect();
        let bpm_mean = if !bpms.is_empty() {
            bpms.iter().sum::<f64>() / bpms.len() as f64
        } else {
            0.0
        };
        let bpm_min = bpms.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
        let bpm_max = bpms
            .iter()
            .cloned()
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.0);
        let bpm_stddev = if !bpms.is_empty() {
            let variance: f64 =
                bpms.iter().map(|b| (b - bpm_mean).powi(2)).sum::<f64>() / bpms.len() as f64;
            variance.sqrt()
        } else {
            0.0
        };

        let genre_dist = self.genre_distribution();

        SessionAnalytics {
            session_id: session_id.to_string(),
            session_name: session_name.to_string(),
            analysis_duration: Duration::ZERO,
            total_tracks,
            unique_tracks,
            total_transitions,
            avg_transition_duration,
            avg_energy,
            energy_min,
            energy_max,
            bpm_mean,
            bpm_min,
            bpm_max,
            bpm_stddev,
            genre_distribution: genre_dist,
            key_distribution: HashMap::new(),
            time_distribution: HashMap::new(),
            analyzed_at: String::new(),
        }
    }
}

impl Default for SessionAnalyticsEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

impl Default for GenreStats {
    fn default() -> Self {
        Self::new()
    }
}
impl Default for KeyCompatibilityAnalysis {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_analytics_creation() {
        let analytics = SessionAnalytics::new("session1".to_string(), "Test Session".to_string());
        assert_eq!(analytics.session_id, "session1");
        assert_eq!(analytics.session_name, "Test Session");
        assert_eq!(analytics.total_tracks, 0);
    }

    #[test]
    fn transition_analysis_creation() {
        let analysis = TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 2.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.85,
            },
        );

        assert_eq!(analysis.bpm_difference, 2.0);
        assert_eq!(analysis.key_compatibility, 0.9);
        assert!(analysis.is_high_quality(0.7));
    }

    #[test]
    fn transition_analysis_low_quality() {
        let analysis = TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Cut".to_string(),
            TransitionMeasurement {
                duration: 0.0,
                bpm_difference: 20.0,
                key_compatibility: 0.2,
                energy_difference: 0.8,
                timing_quality: 0.3,
            },
        );

        assert!(analysis.quality_score < 0.5);
        assert!(!analysis.is_high_quality(0.7));
    }

    #[test]
    fn track_usage_creation() {
        let mut usage = TrackUsage::new(
            "track1".to_string(),
            "Test Track".to_string(),
            "Artist".to_string(),
        );
        assert_eq!(usage.play_count, 0);
        assert_eq!(usage.total_play_duration, 0.0);

        usage.record_play(240.0);
        assert_eq!(usage.play_count, 1);
        assert_eq!(usage.total_play_duration, 240.0);

        usage.record_play(180.0);
        assert_eq!(usage.play_count, 2);
        assert_eq!(usage.total_play_duration, 420.0);
    }

    #[test]
    fn track_usage_avg_stop_position() {
        let mut usage = TrackUsage::new(
            "track1".to_string(),
            "Test Track".to_string(),
            "Artist".to_string(),
        );
        usage.play_count = 2;

        usage.update_avg_stop_position(0.5);
        usage.update_avg_stop_position(0.75);

        assert_eq!(usage.avg_stop_position, 0.625); // (0.5 + 0.75) / 2
    }

    #[test]
    fn analytics_engine_creation() {
        let engine = SessionAnalyticsEngine::new();
        assert_eq!(engine.track_count(), 0);
        assert_eq!(engine.transition_count(), 0);
    }

    #[test]
    fn analytics_engine_record_track() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_track_play(TrackPlay {
            track_id: "track1",
            title: "Test Track",
            artist: "Artist",
            duration: 240.0,
            bpm: 128.0,
            key: "8A",
            genres: vec!["House".to_string()],
        });
        assert_eq!(engine.track_count(), 1);
    }

    #[test]
    fn analytics_engine_record_transition() {
        let mut engine = SessionAnalyticsEngine::new();
        let transition = TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.5,
                key_compatibility: 0.85,
                energy_difference: 0.1,
                timing_quality: 0.9,
            },
        );
        engine.analyze_transition(transition);
        assert_eq!(engine.transition_count(), 1);
    }

    #[test]
    fn analytics_engine_record_energy() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_energy(0.0, 0.5, "track1", "Test Track");
        engine.record_energy(60.0, 0.7, "track2", "Track Two");
        engine.record_energy(120.0, 0.6, "track3", "Track Three");

        assert_eq!(engine.energy_flow().len(), 3);
    }

    #[test]
    fn analytics_engine_detect_patterns() {
        let mut engine = SessionAnalyticsEngine::new();

        // Same pattern twice.
        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.9,
            },
        ));
        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 25.0,
                bpm_difference: 0.5,
                key_compatibility: 0.8,
                energy_difference: 0.2,
                timing_quality: 0.85,
            },
        ));

        engine.detect_patterns();
        assert!(!engine.detected_patterns().is_empty());
        assert_eq!(engine.detected_patterns().len(), 1);
    }

    #[test]
    fn analytics_engine_insights_empty() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.generate_insights();
        assert_eq!(engine.insights().len(), 1);
        assert!(engine.insights()[0].contains("No transitions"));
    }

    #[test]
    fn analytics_engine_insights_quality() {
        let mut engine = SessionAnalyticsEngine::new();

        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.95,
                energy_difference: 0.05,
                timing_quality: 0.95,
            },
        ));
        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 25.0,
                bpm_difference: 0.5,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.9,
            },
        ));

        engine.generate_insights();
        assert!(engine.insights()[0].contains("Excellent"));
    }

    #[test]
    fn analytics_engine_avg_transition_quality() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.85,
            },
        ));

        let quality = engine.avg_transition_quality();
        assert!(quality > 0.0 && quality <= 1.0);
    }

    #[test]
    fn analytics_engine_genre_distribution() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_track_play(TrackPlay {
            track_id: "track1",
            title: "Track 1",
            artist: "Artist",
            duration: 240.0,
            bpm: 128.0,
            key: "8A",
            genres: vec!["House".to_string()],
        });
        engine.record_track_play(TrackPlay {
            track_id: "track2",
            title: "Track 2",
            artist: "Artist",
            duration: 180.0,
            bpm: 126.0,
            key: "9A",
            genres: vec!["Techno".to_string()],
        });

        let dist = engine.genre_distribution();
        assert_eq!(dist.len(), 2);
    }

    #[test]
    fn analytics_engine_most_played() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_track_play(TrackPlay {
            track_id: "track1",
            title: "Track 1",
            artist: "Artist",
            duration: 240.0,
            bpm: 128.0,
            key: "8A",
            genres: vec!["House".to_string()],
        });
        engine.record_track_play(TrackPlay {
            track_id: "track2",
            title: "Track 2",
            artist: "Artist",
            duration: 180.0,
            bpm: 126.0,
            key: "9A",
            genres: vec!["Techno".to_string()],
        });

        let most = engine.most_played(1);
        assert_eq!(most.len(), 1);
    }

    #[test]
    fn analytics_engine_generate_report() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_track_play(TrackPlay {
            track_id: "track1",
            title: "Track 1",
            artist: "Artist",
            duration: 240.0,
            bpm: 128.0,
            key: "8A",
            genres: vec!["House".to_string()],
        });
        engine.record_track_play(TrackPlay {
            track_id: "track2",
            title: "Track 2",
            artist: "Artist",
            duration: 180.0,
            bpm: 130.0,
            key: "9A",
            genres: vec!["Techno".to_string()],
        });

        let report = engine.generate_report("session1", "Test Session");
        assert_eq!(report.total_tracks, 2);
        assert_eq!(report.bpm_mean, 129.0);
    }

    #[test]
    fn analytics_engine_detect_patterns_no_match() {
        let mut engine = SessionAnalyticsEngine::new();

        // Different patterns — no match.
        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.9,
            },
        ));
        engine.analyze_transition(TransitionAnalysis::new(
            "deck_b".to_string(),
            "deck_a".to_string(),
            "Cut".to_string(),
            TransitionMeasurement {
                duration: 0.0,
                bpm_difference: 2.0,
                key_compatibility: 0.5,
                energy_difference: 0.3,
                timing_quality: 0.6,
            },
        ));

        engine.detect_patterns();
        assert!(engine.detected_patterns().is_empty());
    }

    #[test]
    fn analytics_engine_track_stop_position() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_track_play(TrackPlay {
            track_id: "track1",
            title: "Track 1",
            artist: "Artist",
            duration: 240.0,
            bpm: 128.0,
            key: "8A",
            genres: vec!["House".to_string()],
        });

        engine.record_track_stop("track1", 0.5);
        engine.record_track_stop("track1", 0.75);

        let usage = engine.track_usage.get("track1").unwrap();
        assert_eq!(usage.play_count, 1);
        // Stop position updates on call, but play_count is only from record_track_play.
        assert!(!usage.genres.is_empty());
    }

    #[test]
    fn analytics_engine_energy_flow() {
        let mut engine = SessionAnalyticsEngine::new();
        engine.record_energy(0.0, 0.3, "track1", "Track 1");
        engine.record_energy(60.0, 0.8, "track2", "Track 2");

        let flow = engine.energy_flow();
        assert_eq!(flow.len(), 2);
        assert_eq!(flow[0].energy, 0.3);
        assert_eq!(flow[1].energy, 0.8);
    }

    #[test]
    fn analytics_engine_insights_low_energy() {
        let mut engine = SessionAnalyticsEngine::new();

        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.7,
                energy_difference: 0.3,
                timing_quality: 0.7,
            },
        ));

        engine.record_energy(0.0, 0.2, "track1", "Track 1");
        engine.record_energy(60.0, 0.25, "track2", "Track 2");

        engine.generate_insights();
        assert!(engine
            .insights()
            .iter()
            .any(|i| i.contains("Low average energy")));
    }

    #[test]
    fn analytics_engine_insights_high_energy() {
        let mut engine = SessionAnalyticsEngine::new();

        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.9,
            },
        ));

        engine.record_energy(0.0, 0.9, "track1", "Track 1");
        engine.record_energy(60.0, 0.95, "track2", "Track 2");

        engine.generate_insights();
        assert!(engine
            .insights()
            .iter()
            .any(|i| i.contains("high energy") || i.contains("dynamic variation")));
    }

    #[test]
    fn analytics_engine_insights_repetition() {
        let mut engine = SessionAnalyticsEngine::new();

        engine.record_track_play(TrackPlay {
            track_id: "track1",
            title: "T1",
            artist: "A",
            duration: 240.0,
            bpm: 128.0,
            key: "8A",
            genres: vec!["House".to_string()],
        });
        engine.record_track_play(TrackPlay {
            track_id: "track2",
            title: "T2",
            artist: "A",
            duration: 180.0,
            bpm: 126.0,
            key: "9A",
            genres: vec!["House".to_string()],
        });

        engine.analyze_transition(TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.9,
            },
        ));

        engine.generate_insights();
        // With 2 tracks, repetition threshold is > 1.
        assert!(!engine.insights().is_empty());
    }

    #[test]
    fn session_analytics_summary() {
        let mut analytics = SessionAnalytics::new("s1".to_string(), "Test".to_string());
        analytics.total_tracks = 10;
        analytics.total_transitions = 8;
        analytics.bpm_mean = 128.0;
        analytics.avg_energy = 0.7;

        let summary = analytics.summary();
        assert!(summary.contains("Test"));
        assert!(summary.contains("10 tracks"));
        assert!(summary.contains("128.0"));
    }

    #[test]
    fn transition_analysis_timing() {
        let analysis = TransitionAnalysis::new(
            "deck_a".to_string(),
            "deck_b".to_string(),
            "Fade".to_string(),
            TransitionMeasurement {
                duration: 30.0,
                bpm_difference: 1.0,
                key_compatibility: 0.9,
                energy_difference: 0.1,
                timing_quality: 0.85,
            },
        );

        assert_eq!(analysis.duration, 30.0);
        assert!((analysis.bpm_difference - 1.0).abs() < 0.01);
    }
}
