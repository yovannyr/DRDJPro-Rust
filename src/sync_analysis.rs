//! Sync Analysis — Real-time sync quality monitoring, drift detection,
//! phase tracking, and sync event logging.
//!
//! Implements comprehensive analysis of sync quality between decks,
//! detects sync drift, tracks phase relationships, and logs sync events
//! for debugging and performance review.
//! Part of the Sync Analysis feature (P22a).
//!
//! ## Design principles
//!
//! * **Continuous** — Real-time phase tracking.
//! * **Accurate** — Sub-sample phase resolution.
//! * **Diagnostic** — Detailed sync event logging.
//! * **Actionable** — Clear drift warnings.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Sync state between two decks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncState {
    /// Actively synced.
    Synced,
    /// Sync lost.
    Lost,
    /// Sync stabilizing after change.
    Stabilizing,
    /// Manually adjusted.
    Manual,
}

impl SyncState {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Synced => "Synced",
            Self::Lost => "Lost",
            Self::Stabilizing => "Stabilizing",
            Self::Manual => "Manual",
        }
    }

    /// Get color for UI.
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Synced => (50, 255, 100),
            Self::Lost => (255, 50, 50),
            Self::Stabilizing => (255, 200, 50),
            Self::Manual => (100, 150, 255),
        }
    }
}

/// Sync event logged during analysis.
#[derive(Debug, Clone)]
pub struct SyncEvent {
    /// Event type.
    pub event_type: SyncEventType,
    /// Deck pair.
    pub deck_pair: (String, String),
    /// BPM at event time.
    pub bpm_a: f64,
    pub bpm_b: f64,
    /// Phase offset at event.
    pub phase_offset: f64,
    /// Event timestamp.
    pub timestamp: Instant,
    /// Additional details.
    pub details: String,
}

/// Type of sync event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncEventType {
    /// Sync engaged.
    SyncEngaged,
    /// Sync disengaged.
    SyncDisengaged,
    /// Sync lost.
    SyncLost,
    /// Sync regained.
    SyncRegained,
    /// Drift detected.
    DriftDetected,
    /// BPM change.
    BpmChanged,
    /// Phase reset.
    PhaseReset,
    /// Manual adjustment.
    ManualAdjustment,
}

impl SyncEventType {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::SyncEngaged => "Sync Engaged",
            Self::SyncDisengaged => "Sync Disengaged",
            Self::SyncLost => "Sync Lost",
            Self::SyncRegained => "Sync Regained",
            Self::DriftDetected => "Drift Detected",
            Self::BpmChanged => "BPM Changed",
            Self::PhaseReset => "Phase Reset",
            Self::ManualAdjustment => "Manual Adjustment",
        }
    }
}

/// Phase tracking data for a single deck.
#[derive(Debug, Clone)]
pub struct PhaseTracker {
    /// Deck ID.
    pub deck_id: String,
    /// Current phase (0-1, fraction of beat).
    pub phase: f64,
    /// Current BPM.
    pub bpm: f64,
    /// Cumulative phase (for drift detection).
    pub cumulative_phase: f64,
    /// Last phase update.
    pub last_updated: Instant,
    /// Phase history (for trend analysis).
    pub phase_history: VecDeque<(Instant, f64)>,
    /// Max history size.
    pub max_history: usize,
    /// Drift rate (phase change per second).
    pub drift_rate: f64,
    /// Samples of drift per second.
    pub drift_samples_per_sec: f64,
}

impl PhaseTracker {
    /// Create a new phase tracker.
    pub fn new(deck_id: String, bpm: f64) -> Self {
        Self {
            deck_id,
            phase: 0.0,
            bpm,
            cumulative_phase: 0.0,
            last_updated: Instant::now(),
            phase_history: VecDeque::new(),
            max_history: 1000,
            drift_rate: 0.0,
            drift_samples_per_sec: 0.0,
        }
    }

    /// Update phase.
    pub fn update(&mut self, phase: f64, bpm: f64, now: Instant) {
        self.bpm = bpm;
        self.phase = (phase % 1.0).max(0.0);
        self.cumulative_phase += phase;
        self.last_updated = now;

        // Add to history.
        self.phase_history.push_back((now, phase));
        while self.phase_history.len() > self.max_history {
            self.phase_history.pop_front();
        }

        // Calculate drift rate.
        if self.phase_history.len() >= 2 {
            let first = self.phase_history.front().unwrap();
            let elapsed = now.duration_since(first.0).as_secs_f64();
            if elapsed > 0.0 {
                let phase_diff = (self.phase - first.1).abs();
                self.drift_rate = phase_diff / elapsed;
                self.drift_samples_per_sec = self.drift_rate * 44100.0;
            }
        }
    }

    /// Check if tracker is stale (no update in > 1 second).
    pub fn is_stale(&self) -> bool {
        self.last_updated.elapsed() > Duration::from_secs(1)
    }

    /// Get phase trend (improving/stable/degrading).
    pub fn phase_trend(&self) -> PhaseTrend {
        if self.phase_history.len() < 10 {
            return PhaseTrend::InsufficientData;
        }

        let recent: Vec<f64> = self
            .phase_history
            .iter()
            .rev()
            .take(10)
            .map(|(_, p)| *p)
            .collect();
        let older: Vec<f64> = self
            .phase_history
            .iter()
            .rev()
            .skip(10)
            .take(10)
            .map(|(_, p)| *p)
            .collect();

        let recent_avg = recent.iter().sum::<f64>() / recent.len() as f64;
        let older_avg = older.iter().sum::<f64>() / older.len() as f64;

        let diff = (recent_avg - older_avg).abs();
        if diff > 0.05 {
            if recent_avg > older_avg {
                PhaseTrend::Increasing
            } else {
                PhaseTrend::Decreasing
            }
        } else {
            PhaseTrend::Stable
        }
    }
}

/// Phase trend direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhaseTrend {
    Increasing,
    Decreasing,
    Stable,
    InsufficientData,
}

impl PhaseTrend {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Increasing => "Increasing",
            Self::Decreasing => "Decreasing",
            Self::Stable => "Stable",
            Self::InsufficientData => "Insufficient Data",
        }
    }
}

/// Sync quality measurement for a deck pair.
#[derive(Debug, Clone)]
pub struct SyncQuality {
    /// Time of measurement.
    pub measured_at: Instant,
    /// Phase offset (0-1).
    pub phase_offset: f64,
    /// BPM difference.
    pub bpm_difference: f64,
    /// Quality score (0.0-1.0).
    pub quality: f64,
    /// Time since last sync adjustment.
    pub time_since_adjustment: Duration,
    /// Drift rate.
    pub drift_rate: f64,
}

impl SyncQuality {
    /// Create a new sync quality measurement.
    pub fn new(
        phase_offset: f64,
        bpm_difference: f64,
        drift_rate: f64,
        time_since_adjustment: Duration,
        measured_at: Instant,
    ) -> Self {
        // Quality: decreases with phase offset, BPM difference, and drift.
        let quality = (1.0
            - phase_offset * 0.5
            - (bpm_difference / 200.0).min(0.3)
            - (drift_rate * 10.0).min(0.2))
        .max(0.0);

        Self {
            measured_at,
            phase_offset,
            bpm_difference,
            quality,
            time_since_adjustment,
            drift_rate,
        }
    }

    /// Check if quality is acceptable.
    pub fn is_acceptable(&self, threshold: f64) -> bool {
        self.quality >= threshold
    }

    /// Check if sync is degraded.
    pub fn is_degraded(&self) -> bool {
        self.quality < 0.5
    }
}

/// Sync analysis configuration.
#[derive(Debug, Clone)]
pub struct SyncAnalysisConfig {
    /// Drift threshold (phase/sec) for warning.
    pub drift_warning_threshold: f64,
    /// Drift threshold for critical.
    pub drift_critical_threshold: f64,
    /// Quality threshold for acceptable sync.
    pub quality_threshold: f64,
    /// Max events to keep.
    pub max_events: usize,
    /// Sample interval (how often to measure).
    pub sample_interval: Duration,
}

impl Default for SyncAnalysisConfig {
    fn default() -> Self {
        Self {
            drift_warning_threshold: 0.01,
            drift_critical_threshold: 0.05,
            quality_threshold: 0.7,
            max_events: 5000,
            sample_interval: Duration::from_millis(50),
        }
    }
}

/// Sync analysis engine — monitors, analyzes, and logs sync quality.
pub struct SyncAnalysisEngine {
    /// Phase trackers per deck.
    phase_trackers: HashMap<String, PhaseTracker>,
    /// Sync quality history per deck pair.
    quality_history: HashMap<(String, String), VecDeque<SyncQuality>>,
    /// Sync events.
    events: VecDeque<SyncEvent>,
    /// Sync state per deck pair.
    sync_states: HashMap<(String, String), SyncState>,
    /// Configuration.
    config: SyncAnalysisConfig,
    /// Last sync adjustment timestamp.
    last_adjustment: HashMap<(String, String), Instant>,
    /// Total sync events logged.
    total_events_logged: AtomicU64,
    /// Total sync quality measurements.
    total_measurements: AtomicU64,
    /// Analysis started.
    started_at: Option<Instant>,
    /// Analysis running.
    running: bool,
}

impl SyncAnalysisEngine {
    /// Create a new sync analysis engine.
    pub fn new(config: SyncAnalysisConfig) -> Self {
        Self {
            phase_trackers: HashMap::new(),
            quality_history: HashMap::new(),
            events: VecDeque::new(),
            sync_states: HashMap::new(),
            config,
            last_adjustment: HashMap::new(),
            total_events_logged: AtomicU64::new(0),
            total_measurements: AtomicU64::new(0),
            started_at: None,
            running: false,
        }
    }

    /// Start sync analysis.
    pub fn start(&mut self) {
        self.running = true;
        self.started_at = Some(Instant::now());
    }

    /// Stop sync analysis.
    pub fn stop(&mut self) {
        self.running = false;
    }

    /// Check if analysis is running.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get session duration.
    pub fn session_duration(&self) -> Duration {
        self.started_at
            .map(|s| s.elapsed())
            .unwrap_or(Duration::ZERO)
    }

    /// Register a deck.
    pub fn register_deck(&mut self, deck_id: String, bpm: f64) {
        self.phase_trackers
            .entry(deck_id.clone())
            .or_insert_with(|| PhaseTracker::new(deck_id, bpm));
    }

    /// Update phase for a deck.
    pub fn update_phase(&mut self, deck_id: String, phase: f64, bpm: f64) {
        let now = Instant::now();
        if let Some(tracker) = self.phase_trackers.get_mut(&deck_id) {
            tracker.update(phase, bpm, now);
        }
    }

    /// Record a sync event.
    pub fn log_event(&mut self, event: SyncEvent) {
        self.events.push_back(event);
        while self.events.len() > self.config.max_events {
            self.events.pop_front();
        }
        self.total_events_logged.fetch_add(1, Ordering::Relaxed);
    }

    /// Measure sync quality between two decks.
    pub fn measure_sync_quality(&mut self, deck_a: &str, deck_b: &str) -> Option<SyncQuality> {
        let tracker_a = self.phase_trackers.get(deck_a)?;
        let tracker_b = self.phase_trackers.get(deck_b)?;

        let phase_offset = (tracker_b.phase - tracker_a.phase)
            .abs()
            .min(1.0 - (tracker_b.phase - tracker_a.phase).abs());
        let bpm_difference = (tracker_b.bpm - tracker_a.bpm).abs();
        let drift_rate = (tracker_b.drift_rate - tracker_a.drift_rate).abs();

        let adjustment_time = self
            .last_adjustment
            .get(&(deck_a.to_string(), deck_b.to_string()))
            .or_else(|| {
                self.last_adjustment
                    .get(&(deck_b.to_string(), deck_a.to_string()))
            })
            .copied()
            .unwrap_or(Instant::now());
        let time_since_adjustment = Instant::now().duration_since(adjustment_time);

        let quality = SyncQuality::new(
            phase_offset,
            bpm_difference,
            drift_rate,
            time_since_adjustment,
            Instant::now(),
        );

        // Store in history.
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };
        let history = self.quality_history.entry(key).or_default();
        history.push_back(quality.clone());
        while history.len() > 10000 {
            history.pop_front();
        }

        self.total_measurements.fetch_add(1, Ordering::Relaxed);

        Some(quality)
    }

    /// Get average sync quality between two decks.
    pub fn avg_sync_quality(&self, deck_a: &str, deck_b: &str) -> Option<f64> {
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };
        let history = self.quality_history.get(&key)?;
        if history.is_empty() {
            return None;
        }
        Some(history.iter().map(|q| q.quality).sum::<f64>() / history.len() as f64)
    }

    /// Get latest sync quality between two decks.
    pub fn latest_sync_quality(&self, deck_a: &str, deck_b: &str) -> Option<&SyncQuality> {
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };
        self.quality_history.get(&key)?.back()
    }

    /// Check if two decks are in sync.
    pub fn are_synced(&self, deck_a: &str, deck_b: &str) -> bool {
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };
        self.sync_states
            .get(&key)
            .map(|s| *s == SyncState::Synced)
            .unwrap_or(false)
    }

    /// Get sync state between two decks.
    pub fn sync_state(&self, deck_a: &str, deck_b: &str) -> Option<SyncState> {
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };
        self.sync_states.get(&key).copied()
    }

    /// Set sync state between two decks.
    pub fn set_sync_state(&mut self, deck_a: &str, deck_b: &str, state: SyncState) {
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };

        // Log event.
        let quality = self.measure_sync_quality(deck_a, deck_b);
        self.log_event(SyncEvent {
            event_type: match state {
                SyncState::Synced => SyncEventType::SyncEngaged,
                SyncState::Lost => SyncEventType::SyncLost,
                SyncState::Stabilizing => SyncEventType::SyncRegained,
                SyncState::Manual => SyncEventType::ManualAdjustment,
            },
            deck_pair: (deck_a.to_string(), deck_b.to_string()),
            bpm_a: self
                .phase_trackers
                .get(deck_a)
                .map(|t| t.bpm)
                .unwrap_or(0.0),
            bpm_b: self
                .phase_trackers
                .get(deck_b)
                .map(|t| t.bpm)
                .unwrap_or(0.0),
            phase_offset: quality.map(|q| q.phase_offset).unwrap_or(0.0),
            timestamp: Instant::now(),
            details: format!("State changed to {}", state.display_name()),
        });

        self.sync_states.insert(key.clone(), state);
        if state == SyncState::Synced {
            self.last_adjustment.insert(key, Instant::now());
        }
    }

    /// Check for drift in a deck.
    pub fn check_drift(&mut self, deck_a: &str, deck_b: &str) -> Option<DriftStatus> {
        let quality = self.measure_sync_quality(deck_a, deck_b)?;

        if quality.drift_rate >= self.config.drift_critical_threshold {
            Some(DriftStatus::Critical(quality.drift_rate))
        } else if quality.drift_rate >= self.config.drift_warning_threshold {
            Some(DriftStatus::Warning(quality.drift_rate))
        } else {
            Some(DriftStatus::Normal)
        }
    }

    /// Get recent sync events.
    pub fn recent_events(&self, limit: usize) -> Vec<&SyncEvent> {
        self.events.iter().rev().take(limit).collect()
    }

    /// Get sync events filtered by type.
    pub fn events_by_type(&self, event_type: SyncEventType, limit: usize) -> Vec<&SyncEvent> {
        self.events
            .iter()
            .filter(|e| e.event_type == event_type)
            .rev()
            .take(limit)
            .collect()
    }

    /// Get phase tracker for a deck.
    pub fn get_tracker(&self, deck_id: &str) -> Option<&PhaseTracker> {
        self.phase_trackers.get(deck_id)
    }

    /// Get all phase trackers.
    pub fn all_trackers(&self) -> Vec<&PhaseTracker> {
        self.phase_trackers.values().collect()
    }

    /// Get registered deck IDs.
    pub fn registered_decks(&self) -> Vec<&String> {
        self.phase_trackers.keys().collect()
    }

    /// Log a BPM change.
    pub fn log_bpm_change(&mut self, deck_a: &str, deck_b: &str, bpm_a: f64, bpm_b: f64) {
        self.log_event(SyncEvent {
            event_type: SyncEventType::BpmChanged,
            deck_pair: (deck_a.to_string(), deck_b.to_string()),
            bpm_a,
            bpm_b,
            phase_offset: 0.0,
            timestamp: Instant::now(),
            details: format!("BPM A: {:.2}, BPM B: {:.2}", bpm_a, bpm_b),
        });
    }

    /// Log a phase reset.
    pub fn log_phase_reset(&mut self, deck_a: &str, deck_b: &str) {
        self.log_event(SyncEvent {
            event_type: SyncEventType::PhaseReset,
            deck_pair: (deck_a.to_string(), deck_b.to_string()),
            bpm_a: self
                .phase_trackers
                .get(deck_a)
                .map(|t| t.bpm)
                .unwrap_or(0.0),
            bpm_b: self
                .phase_trackers
                .get(deck_b)
                .map(|t| t.bpm)
                .unwrap_or(0.0),
            phase_offset: 0.0,
            timestamp: Instant::now(),
            details: "Phase reset".to_string(),
        });
    }

    /// Get total events logged.
    pub fn total_events(&self) -> u64 {
        self.total_events_logged.load(Ordering::Relaxed)
    }

    /// Get total measurements.
    pub fn total_measurements_count(&self) -> u64 {
        self.total_measurements.load(Ordering::Relaxed)
    }

    /// Get sync quality for a deck pair.
    pub fn sync_quality_history(&self, deck_a: &str, deck_b: &str) -> Vec<&SyncQuality> {
        let key = if deck_a < deck_b {
            (deck_a.to_string(), deck_b.to_string())
        } else {
            (deck_b.to_string(), deck_a.to_string())
        };
        self.quality_history
            .get(&key)
            .map(|h| h.iter().collect())
            .unwrap_or_default()
    }

    /// Clear all data.
    pub fn clear(&mut self) {
        self.phase_trackers.clear();
        self.quality_history.clear();
        self.events.clear();
        self.sync_states.clear();
        self.last_adjustment.clear();
    }
}

/// Drift status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DriftStatus {
    /// Normal drift.
    Normal,
    /// Warning drift.
    Warning(f64),
    /// Critical drift.
    Critical(f64),
}

impl DriftStatus {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Normal => "Normal",
            Self::Warning(_) => "Warning",
            Self::Critical(_) => "Critical",
        }
    }

    /// Get color for UI.
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Normal => (50, 255, 100),
            Self::Warning(_) => (255, 200, 50),
            Self::Critical(_) => (255, 50, 50),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_state_names() {
        assert_eq!(SyncState::Synced.display_name(), "Synced");
        assert_eq!(SyncState::Lost.display_name(), "Lost");
        assert_eq!(SyncState::Stabilizing.display_name(), "Stabilizing");
        assert_eq!(SyncState::Manual.display_name(), "Manual");
    }

    #[test]
    fn sync_event_type_names() {
        assert_eq!(SyncEventType::SyncEngaged.display_name(), "Sync Engaged");
        assert_eq!(SyncEventType::SyncLost.display_name(), "Sync Lost");
        assert_eq!(
            SyncEventType::DriftDetected.display_name(),
            "Drift Detected"
        );
        assert_eq!(SyncEventType::BpmChanged.display_name(), "BPM Changed");
    }

    #[test]
    fn phase_tracker_creation() {
        let tracker = PhaseTracker::new("deck_a".to_string(), 128.0);
        assert_eq!(tracker.deck_id, "deck_a");
        assert_eq!(tracker.bpm, 128.0);
        assert_eq!(tracker.phase, 0.0);
        assert!(!tracker.is_stale());
    }

    #[test]
    fn phase_tracker_update() {
        let mut tracker = PhaseTracker::new("deck_a".to_string(), 128.0);
        let now = Instant::now();
        tracker.update(0.5, 130.0, now);

        assert_eq!(tracker.phase, 0.5);
        assert_eq!(tracker.bpm, 130.0);
        assert_eq!(tracker.cumulative_phase, 0.5);
    }

    #[test]
    fn phase_tracker_phase_wrapping() {
        let mut tracker = PhaseTracker::new("deck_a".to_string(), 128.0);
        tracker.update(1.5, 128.0, Instant::now());
        assert_eq!(tracker.phase, 0.5); // Wrapped to 0-1.
    }

    #[test]
    fn phase_tracker_drift() {
        let mut tracker = PhaseTracker::new("deck_a".to_string(), 128.0);
        let now = Instant::now();

        tracker.update(0.1, 128.0, now);
        tracker.update(0.2, 128.0, now + Duration::from_millis(100));

        assert!(tracker.drift_rate >= 0.0);
    }

    #[test]
    fn phase_trend_names() {
        assert_eq!(PhaseTrend::Increasing.display_name(), "Increasing");
        assert_eq!(PhaseTrend::Decreasing.display_name(), "Decreasing");
        assert_eq!(PhaseTrend::Stable.display_name(), "Stable");
        assert_eq!(
            PhaseTrend::InsufficientData.display_name(),
            "Insufficient Data"
        );
    }

    #[test]
    fn sync_quality_creation() {
        let quality = SyncQuality::new(0.0, 0.0, 0.0, Duration::ZERO, Instant::now());
        assert_eq!(quality.phase_offset, 0.0);
        assert_eq!(quality.bpm_difference, 0.0);
        assert!((quality.quality - 1.0).abs() < 0.01);
        assert!(quality.is_acceptable(0.7));
    }

    #[test]
    fn sync_quality_degraded() {
        let quality = SyncQuality::new(0.5, 10.0, 0.05, Duration::ZERO, Instant::now());
        assert!(quality.is_degraded());
    }

    #[test]
    fn sync_analysis_engine_creation() {
        let engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        assert!(!engine.is_running());
        assert_eq!(engine.session_duration().as_millis(), 0);
    }

    #[test]
    fn sync_analysis_engine_start_stop() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        assert!(!engine.is_running());

        engine.start();
        assert!(engine.is_running());

        engine.stop();
        assert!(!engine.is_running());
    }

    #[test]
    fn sync_analysis_engine_register_deck() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);

        let tracker = engine.get_tracker("deck_a");
        assert!(tracker.is_some());
        assert_eq!(tracker.unwrap().bpm, 128.0);

        let decks = engine.registered_decks();
        assert_eq!(decks.len(), 1);
    }

    #[test]
    fn sync_analysis_engine_update_phase() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.update_phase("deck_a".to_string(), 0.5, 128.0);

        let tracker = engine.get_tracker("deck_a").unwrap();
        assert_eq!(tracker.phase, 0.5);
    }

    #[test]
    fn sync_analysis_engine_sync_quality() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);

        engine.update_phase("deck_a".to_string(), 0.1, 128.0);
        engine.update_phase("deck_b".to_string(), 0.12, 128.0);

        let quality = engine.measure_sync_quality("deck_a", "deck_b");
        assert!(quality.is_some());
        assert!(quality.unwrap().quality > 0.5);
    }

    #[test]
    fn sync_analysis_engine_avg_quality() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);

        engine.update_phase("deck_a".to_string(), 0.1, 128.0);
        engine.update_phase("deck_b".to_string(), 0.12, 128.0);

        for _ in 0..5 {
            engine.measure_sync_quality("deck_a", "deck_b");
        }

        let avg = engine.avg_sync_quality("deck_a", "deck_b");
        assert!(avg.is_some());
        assert!(avg.unwrap() > 0.0 && avg.unwrap() <= 1.0);
    }

    #[test]
    fn sync_analysis_engine_sync_state() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());

        engine.set_sync_state("deck_a", "deck_b", SyncState::Synced);
        assert!(engine.are_synced("deck_a", "deck_b"));
        assert_eq!(
            engine.sync_state("deck_a", "deck_b"),
            Some(SyncState::Synced)
        );

        engine.set_sync_state("deck_a", "deck_b", SyncState::Lost);
        assert!(!engine.are_synced("deck_a", "deck_b"));
        assert_eq!(engine.sync_state("deck_a", "deck_b"), Some(SyncState::Lost));
    }

    #[test]
    fn sync_analysis_engine_drift() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);

        engine.update_phase("deck_a".to_string(), 0.1, 128.0);
        engine.update_phase("deck_b".to_string(), 0.1, 128.0);

        let drift = engine.check_drift("deck_a", "deck_b");
        assert!(drift.is_some());
        assert!(matches!(drift.unwrap(), DriftStatus::Normal));
    }

    #[test]
    fn sync_analysis_engine_log_event() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);

        engine.set_sync_state("deck_a", "deck_b", SyncState::Synced);
        assert!(engine.total_events() > 0);
    }

    #[test]
    fn sync_analysis_engine_recent_events() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);

        engine.set_sync_state("deck_a", "deck_b", SyncState::Synced);
        engine.set_sync_state("deck_a", "deck_b", SyncState::Lost);
        engine.set_sync_state("deck_a", "deck_b", SyncState::Synced);

        let events = engine.recent_events(5);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn sync_analysis_engine_log_bpm_change() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.log_bpm_change("deck_a", "deck_b", 128.0, 130.0);

        let events = engine.events_by_type(SyncEventType::BpmChanged, 10);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn sync_analysis_engine_log_phase_reset() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.log_phase_reset("deck_a", "deck_b");

        let events = engine.events_by_type(SyncEventType::PhaseReset, 10);
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn drift_status_names() {
        assert_eq!(DriftStatus::Normal.display_name(), "Normal");
        assert_eq!(DriftStatus::Warning(0.01).display_name(), "Warning");
        assert_eq!(DriftStatus::Critical(0.05).display_name(), "Critical");
    }

    #[test]
    fn drift_status_colors() {
        assert_eq!(DriftStatus::Normal.color(), (50, 255, 100));
        assert_eq!(DriftStatus::Warning(0.01).color(), (255, 200, 50));
        assert_eq!(DriftStatus::Critical(0.05).color(), (255, 50, 50));
    }

    #[test]
    fn sync_analysis_engine_total_counts() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        assert_eq!(engine.total_events(), 0);
        assert_eq!(engine.total_measurements_count(), 0);

        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);
        engine.update_phase("deck_a".to_string(), 0.1, 128.0);
        engine.update_phase("deck_b".to_string(), 0.12, 128.0);
        engine.measure_sync_quality("deck_a", "deck_b");

        assert!(engine.total_measurements_count() > 0);
    }

    #[test]
    fn sync_analysis_engine_clear() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);
        engine.update_phase("deck_a".to_string(), 0.1, 128.0);
        engine.set_sync_state("deck_a", "deck_b", SyncState::Synced);

        engine.clear();
        assert_eq!(engine.registered_decks().len(), 0);
        assert!(engine.recent_events(10).is_empty());
    }

    #[test]
    fn sync_quality_history() {
        let mut engine = SyncAnalysisEngine::new(SyncAnalysisConfig::default());
        engine.register_deck("deck_a".to_string(), 128.0);
        engine.register_deck("deck_b".to_string(), 128.0);
        engine.update_phase("deck_a".to_string(), 0.1, 128.0);
        engine.update_phase("deck_b".to_string(), 0.12, 128.0);

        for _ in 0..5 {
            engine.measure_sync_quality("deck_a", "deck_b");
        }

        let history = engine.sync_quality_history("deck_a", "deck_b");
        assert_eq!(history.len(), 5);
    }
}
