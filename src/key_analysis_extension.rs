//! Key Detection Extension (P06b) — Extended harmonic analysis, manual
//! key override persistence, key-change detection across edits, and
//! compatibility scoring with confidence intervals.
//!
//! Extends P06a (Camelot wheel, key compatibility) with FR-009 through
//! FR-017 of Spec 6.
//!
//! ## Design principles
//!
//! * **Confidence intervals** — Every key analysis carries a confidence
//!   score and a 95 % confidence interval over possible keys.
//! * **Manual overrides** — User edits are persisted with timestamps and
//!   can be reverted.
//! * **Change detection** — Detect when a key changes across re-analysis
//!   or import.
//! * **Thread-safe** — All mutations are Mutex-protected.

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Key confidence interval: the N keys most likely to be correct.
#[derive(Debug, Clone)]
pub struct KeyConfidenceInterval {
    /// Ranked candidate keys with likelihood scores.
    pub candidates: Vec<(CamelotKey, f64)>,
    /// Width of the 95 % interval (number of candidate keys).
    pub interval_width: usize,
}

impl KeyConfidenceInterval {
    /// Create a new interval with one candidate (100 % confidence).
    pub fn exact(key: CamelotKey) -> Self {
        Self {
            candidates: vec![(key, 1.0)],
            interval_width: 1,
        }
    }

    /// Get the top-N most likely keys.
    pub fn top_n(&self, n: usize) -> Vec<(CamelotKey, f64)> {
        let mut candidates = self.candidates.clone();
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        candidates.into_iter().take(n).collect()
    }

    /// Check if a key is within the interval (score > 0.01).
    pub fn contains(&self, key: CamelotKey) -> bool {
        self.candidates
            .iter()
            .any(|(k, score)| *k == key && *score > 0.01)
    }

    /// Get the best key (highest score).
    pub fn best_key(&self) -> Option<CamelotKey> {
        self.candidates
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(k, _)| *k)
    }
}

/// Key analysis with confidence data.
#[derive(Debug, Clone)]
pub struct KeyAnalysisWithConfidence {
    /// Track identifier.
    pub track_id: String,
    /// Detected key.
    pub key: CamelotKey,
    /// Confidence score (0.0 – 1.0).
    pub confidence: f64,
    /// Full confidence interval over candidate keys.
    pub confidence_interval: KeyConfidenceInterval,
    /// Whether this key was manually overridden.
    pub is_manual_override: bool,
    /// Timestamp of last override (if manual).
    pub override_timestamp: Option<std::time::Instant>,
    /// Number of re-analyses performed.
    pub reanalysis_count: u32,
}

impl fmt::Display for KeyAnalysisWithConfidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (confidence={:.1}%, manual={})",
            self.key.to_standard(),
            self.confidence * 100.0,
            self.is_manual_override
        )
    }
}

/// Key change event detected between two analyses.
#[derive(Debug, Clone)]
pub struct KeyChangeEvent {
    /// Track identifier.
    pub track_id: String,
    /// Previous key analysis.
    pub previous_key: CamelotKey,
    /// New key analysis.
    pub new_key: CamelotKey,
    /// Whether the change was detected automatically or manually.
    pub detected_by: KeyChangeSource,
    /// Timestamp of detection.
    pub detected_at: std::time::Instant,
}

impl fmt::Display for KeyChangeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Key change {} → {} (by {:?})",
            self.previous_key, self.new_key, self.detected_by
        )
    }
}

/// Source of a key change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyChangeSource {
    /// Automatic re-analysis detected a different key.
    ReAnalysis,
    /// User manually edited the key.
    ManualEdit,
    /// Import from a different source changed the key.
    Import,
}

impl fmt::Display for KeyChangeSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyChangeSource::ReAnalysis => write!(f, "re-analysis"),
            KeyChangeSource::ManualEdit => write!(f, "manual edit"),
            KeyChangeSource::Import => write!(f, "import"),
        }
    }
}

/// Key analysis override stored for persistence.
#[derive(Debug, Clone)]
pub struct KeyOverride {
    /// Track identifier.
    pub track_id: String,
    /// Manually set key.
    pub overridden_key: CamelotKey,
    /// Timestamp of override.
    pub timestamp: std::time::Instant,
    /// User who made the override (for collaboration).
    pub user: Option<String>,
    /// Whether this override should survive re-analysis.
    pub lock: bool,
}

impl KeyOverride {
    /// Create a new key override.
    pub fn new(track_id: String, key: CamelotKey, user: Option<String>) -> Self {
        Self {
            track_id,
            overridden_key: key,
            timestamp: std::time::Instant::now(),
            user,
            lock: false,
        }
    }
}

/// Key change history for a single track.
#[derive(Debug, Clone)]
pub struct KeyHistoryEntry {
    /// Track identifier.
    pub track_id: String,
    /// Key at this point in time.
    pub key: CamelotKey,
    /// Confidence at this point.
    pub confidence: f64,
    /// Whether this was auto-detected or manual.
    pub source: KeyChangeSource,
    /// Timestamp.
    pub timestamp: std::time::Instant,
}

/// Key analysis engine with extended features.
pub struct KeyAnalysisEngine {
    /// Current analyses keyed by track_id.
    analyses: std::sync::Mutex<HashMap<String, KeyAnalysisWithConfidence>>,
    /// Manual overrides keyed by track_id.
    overrides: std::sync::Mutex<HashMap<String, KeyOverride>>,
    /// Change events log.
    change_log: std::sync::Mutex<Vec<KeyChangeEvent>>,
    /// History per track.
    history: std::sync::Mutex<HashMap<String, Vec<KeyHistoryEntry>>>,
    /// Total analyses performed.
    analysis_count: AtomicU64,
    /// Total overrides applied.
    override_count: AtomicU64,
}

impl KeyAnalysisEngine {
    /// Create a new key analysis engine.
    pub fn new() -> Self {
        Self {
            analyses: std::sync::Mutex::new(HashMap::new()),
            overrides: std::sync::Mutex::new(HashMap::new()),
            change_log: std::sync::Mutex::new(Vec::new()),
            history: std::sync::Mutex::new(HashMap::new()),
            analysis_count: AtomicU64::new(0),
            override_count: AtomicU64::new(0),
        }
    }

    /// Analyze a track's key and store the result with confidence.
    pub fn analyze_key(&self, track_id: &str, key: CamelotKey, confidence: f64) {
        self.analysis_count.fetch_add(1, Ordering::Relaxed);

        // Check if a manual override is locked.
        let override_applied = {
            let overrides = self.overrides.lock().unwrap();
            overrides
                .get(track_id)
                .filter(|o| o.lock)
                .map(|o| o.overridden_key)
        };

        let effective_key = override_applied.unwrap_or(key);
        let is_manual = override_applied.is_some();

        let confidence_interval = if is_manual || confidence > 0.9 {
            KeyConfidenceInterval::exact(effective_key)
        } else {
            // Generate candidate keys based on confidence.
            self.generate_candidates(key, confidence)
        };

        let entry = KeyAnalysisWithConfidence {
            track_id: track_id.to_string(),
            key: effective_key,
            confidence,
            confidence_interval,
            is_manual_override: is_manual,
            override_timestamp: is_manual.then(std::time::Instant::now),
            reanalysis_count: 0,
        };

        // Store in history.
        {
            let mut history = self.history.lock().unwrap();
            let track_history = history.entry(track_id.to_string()).or_default();
            track_history.push(KeyHistoryEntry {
                track_id: track_id.to_string(),
                key,
                confidence,
                source: KeyChangeSource::ReAnalysis,
                timestamp: std::time::Instant::now(),
            });
            // Keep last 100 history entries per track.
            if track_history.len() > 100 {
                track_history.drain(..track_history.len() - 100);
            }
        }

        // Detect key change.
        {
            let analyses = self.analyses.lock().unwrap();
            if let Some(existing) = analyses.get(track_id) {
                if existing.key != effective_key {
                    let mut change_log = self.change_log.lock().unwrap();
                    change_log.push(KeyChangeEvent {
                        track_id: track_id.to_string(),
                        previous_key: existing.key,
                        new_key: effective_key,
                        detected_by: KeyChangeSource::ReAnalysis,
                        detected_at: std::time::Instant::now(),
                    });
                    // Keep last 1000 change events.
                    if change_log.len() > 1000 {
                        let excess = change_log.len() - 1000;
                        change_log.drain(..excess);
                    }
                }
            }
        }

        // Store the analysis.
        {
            let mut analyses = self.analyses.lock().unwrap();
            analyses.insert(track_id.to_string(), entry);
        }
    }

    /// Apply a manual key override.
    pub fn apply_override(
        &self,
        track_id: &str,
        key: CamelotKey,
        user: Option<String>,
        lock: bool,
    ) {
        let mut override_entry = KeyOverride::new(track_id.to_string(), key, user);
        override_entry.lock = lock;
        self.override_count.fetch_add(1, Ordering::Relaxed);

        // Store override.
        {
            let mut overrides = self.overrides.lock().unwrap();
            overrides.insert(track_id.to_string(), override_entry);
        }

        // Update the analysis.
        let mut analyses = self.analyses.lock().unwrap();
        if let Some(analysis) = analyses.get_mut(track_id) {
            analysis.key = key;
            analysis.is_manual_override = true;
            analysis.override_timestamp = Some(std::time::Instant::now());
        }

        // Log history.
        {
            let mut history = self.history.lock().unwrap();
            let track_history = history.entry(track_id.to_string()).or_default();
            track_history.push(KeyHistoryEntry {
                track_id: track_id.to_string(),
                key,
                confidence: 1.0,
                source: KeyChangeSource::ManualEdit,
                timestamp: std::time::Instant::now(),
            });
        }
    }

    /// Remove a manual override for a track.
    pub fn remove_override(&self, track_id: &str) {
        let mut overrides = self.overrides.lock().unwrap();
        overrides.remove(track_id);

        // Restore the most recent detected value, retaining manual entries in history.
        let detected = self
            .history
            .lock()
            .unwrap()
            .get(track_id)
            .and_then(|entries| {
                entries
                    .iter()
                    .rev()
                    .find(|entry| entry.source != KeyChangeSource::ManualEdit)
                    .cloned()
            });
        if let Some(detected) = detected {
            if let Some(analysis) = self.analyses.lock().unwrap().get_mut(track_id) {
                analysis.key = detected.key;
                analysis.confidence = detected.confidence;
                analysis.is_manual_override = false;
                analysis.override_timestamp = None;
                analysis.confidence_interval =
                    self.generate_candidates(detected.key, detected.confidence);
            }
        }
    }

    /// Get the current key analysis for a track.
    pub fn get_analysis(&self, track_id: &str) -> Option<KeyAnalysisWithConfidence> {
        self.analyses.lock().unwrap().get(track_id).cloned()
    }

    /// Get all key change events.
    pub fn get_change_log(&self) -> Vec<KeyChangeEvent> {
        self.change_log.lock().unwrap().clone()
    }

    /// Get the key history for a track.
    pub fn get_history(&self, track_id: &str) -> Vec<KeyHistoryEntry> {
        self.history
            .lock()
            .unwrap()
            .get(track_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all analyses for batch display.
    pub fn get_all_analyses(&self) -> Vec<KeyAnalysisWithConfidence> {
        self.analyses.lock().unwrap().values().cloned().collect()
    }

    /// Get pending overrides (unlocked manual overrides).
    pub fn pending_overrides(&self) -> Vec<KeyOverride> {
        self.overrides
            .lock()
            .unwrap()
            .values()
            .filter(|o| !o.lock)
            .cloned()
            .collect()
    }

    /// Clear all overrides and history.
    pub fn clear_all(&self) {
        self.analyses.lock().unwrap().clear();
        self.overrides.lock().unwrap().clear();
        self.history.lock().unwrap().clear();
        self.change_log.lock().unwrap().clear();
    }

    /// Get statistics.
    pub fn stats(&self) -> (u64, u64, usize) {
        let analyses = self.analyses.lock().unwrap().len();
        (
            self.analysis_count.load(Ordering::Relaxed),
            self.override_count.load(Ordering::Relaxed),
            analyses,
        )
    }

    /// Generate candidate keys for a low-confidence analysis.
    fn generate_candidates(&self, best_key: CamelotKey, confidence: f64) -> KeyConfidenceInterval {
        let mut candidates = Vec::new();
        candidates.push((best_key, confidence));

        // Add adjacent keys proportional to lack of confidence.
        let uncertainty = 1.0 - confidence;
        if uncertainty > 0.1 {
            // Adjacent keys.
            for offset in [-1i32, 1] {
                let num = ((best_key.number as i32 + offset - 1).rem_euclid(12) + 1) as u8;
                let adj = CamelotKey {
                    number: num,
                    mode: best_key.mode,
                };
                candidates.push((adj, uncertainty * 0.3));
            }

            // Relative key (major/minor).
            let relative_mode = match best_key.mode {
                crate::harmonie_wheel::KeyMode::Major => crate::harmonie_wheel::KeyMode::Minor,
                crate::harmonie_wheel::KeyMode::Minor => crate::harmonie_wheel::KeyMode::Major,
            };
            let relative = CamelotKey {
                number: best_key.number,
                mode: relative_mode,
            };
            candidates.push((relative, uncertainty * 0.2));
        }

        // Normalize scores to sum to ~1.0.
        let total: f64 = candidates.iter().map(|(_, s)| s).sum();
        if total > 0.0 {
            for (_, s) in &mut candidates {
                *s /= total;
            }
        }

        let interval_width = 3.min(candidates.len());
        KeyConfidenceInterval {
            candidates,
            interval_width,
        }
    }
}

impl Default for KeyAnalysisEngine {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export CamelotKey and KeyMode from harmonie_wheel.
use crate::harmonie_wheel::CamelotKey;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_confidence_interval_exact() {
        let interval = KeyConfidenceInterval::exact(CamelotKey::parse("8A").unwrap());
        assert_eq!(interval.interval_width, 1);
        assert_eq!(interval.candidates.len(), 1);
        assert_eq!(interval.candidates[0].1, 1.0);
    }

    #[test]
    fn test_key_confidence_interval_contains() {
        let interval = KeyConfidenceInterval::exact(CamelotKey::parse("8A").unwrap());
        assert!(interval.contains(CamelotKey::parse("8A").unwrap()));
        assert!(!interval.contains(CamelotKey::parse("9A").unwrap()));
    }

    #[test]
    fn test_key_confidence_interval_top_n() {
        let mut interval = KeyConfidenceInterval::exact(CamelotKey::parse("8A").unwrap());
        // Manually add candidates for testing.
        interval
            .candidates
            .push((CamelotKey::parse("9A").unwrap(), 0.3));
        interval
            .candidates
            .push((CamelotKey::parse("8B").unwrap(), 0.2));

        let top = interval.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].1, 1.0); // Best first.
    }

    #[test]
    fn test_key_analysis_engine_analyze() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);

        let analysis = engine.get_analysis("track-1").unwrap();
        assert_eq!(analysis.key, CamelotKey::parse("8A").unwrap());
        assert_eq!(analysis.confidence, 0.95);
        assert!(!analysis.is_manual_override);
    }

    #[test]
    fn test_key_analysis_engine_override() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);

        // Apply override.
        engine.apply_override(
            "track-1",
            CamelotKey::parse("9A").unwrap(),
            Some("user1".to_string()),
            false,
        );

        let analysis = engine.get_analysis("track-1").unwrap();
        assert_eq!(analysis.key, CamelotKey::parse("9A").unwrap());
        assert!(analysis.is_manual_override);

        // Remove override.
        engine.remove_override("track-1");
        let analysis = engine.get_analysis("track-1").unwrap();
        assert_eq!(analysis.key, CamelotKey::parse("8A").unwrap());
        assert!(!analysis.is_manual_override);
    }

    #[test]
    fn test_key_change_detection() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);
        engine.analyze_key("track-1", CamelotKey::parse("9A").unwrap(), 0.9);

        let changes = engine.get_change_log();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].previous_key, CamelotKey::parse("8A").unwrap());
        assert_eq!(changes[0].new_key, CamelotKey::parse("9A").unwrap());
    }

    #[test]
    fn test_key_history() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);
        engine.analyze_key("track-1", CamelotKey::parse("9A").unwrap(), 0.9);

        let history = engine.get_history("track-1");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].key, CamelotKey::parse("8A").unwrap());
        assert_eq!(history[1].key, CamelotKey::parse("9A").unwrap());
    }

    #[test]
    fn test_locked_override_survives_reanalysis() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);

        // Apply locked override.
        engine.apply_override("track-1", CamelotKey::parse("9A").unwrap(), None, true);

        // Re-analyze with different key.
        engine.analyze_key("track-1", CamelotKey::parse("7B").unwrap(), 0.95);

        let analysis = engine.get_analysis("track-1").unwrap();
        assert_eq!(analysis.key, CamelotKey::parse("9A").unwrap());
    }

    #[test]
    fn removing_locked_override_restores_latest_detection() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track", CamelotKey::parse("8A").unwrap(), 0.95);
        engine.apply_override("track", CamelotKey::parse("9A").unwrap(), None, true);
        engine.analyze_key("track", CamelotKey::parse("10A").unwrap(), 0.8);
        assert_eq!(engine.get_analysis("track").unwrap().key.to_string(), "9A");
        engine.remove_override("track");
        let analysis = engine.get_analysis("track").unwrap();
        assert_eq!(analysis.key.to_string(), "10A");
        assert!(!analysis.is_manual_override);
        assert_eq!(analysis.confidence, 0.8);
    }

    #[test]
    fn test_engine_stats() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);
        engine.analyze_key("track-2", CamelotKey::parse("9A").unwrap(), 0.85);
        engine.apply_override("track-1", CamelotKey::parse("9A").unwrap(), None, false);

        let (analyzes, overrides, stored) = engine.stats();
        assert_eq!(analyzes, 2);
        assert_eq!(overrides, 1);
        assert_eq!(stored, 2);
    }

    #[test]
    fn test_clear_all() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);
        engine.apply_override("track-1", CamelotKey::parse("9A").unwrap(), None, false);

        engine.clear_all();

        let (analyzes, overrides, stored) = engine.stats();
        assert_eq!(analyzes, 1); // One analysis was performed; the override has its own counter.
        assert_eq!(overrides, 1);
        assert_eq!(stored, 0); // Analyses are cleared.
    }

    #[test]
    fn test_key_override_display() {
        let override_entry = KeyOverride::new(
            "track-1".to_string(),
            CamelotKey::parse("8A").unwrap(),
            Some("user1".to_string()),
        );
        assert_eq!(override_entry.track_id, "track-1");
        assert_eq!(override_entry.user, Some("user1".to_string()));
        assert!(!override_entry.lock);
    }

    #[test]
    fn test_key_change_source_display() {
        assert_eq!(format!("{}", KeyChangeSource::ReAnalysis), "re-analysis");
        assert_eq!(format!("{}", KeyChangeSource::ManualEdit), "manual edit");
        assert_eq!(format!("{}", KeyChangeSource::Import), "import");
    }

    #[test]
    fn test_key_analysis_display() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("track-1", CamelotKey::parse("8A").unwrap(), 0.95);

        let analysis = engine.get_analysis("track-1").unwrap();
        let display = format!("{}", analysis);
        assert!(display.contains("A Minor"));
        assert!(display.contains("95.0"));
    }

    #[test]
    fn test_candidate_generation() {
        let engine = KeyAnalysisEngine::new();
        // High confidence → exact interval.
        let high_conf = engine.generate_candidates(CamelotKey::parse("8A").unwrap(), 0.95);
        assert_eq!(high_conf.interval_width, 1);

        // Low confidence → multiple candidates.
        let low_conf = engine.generate_candidates(CamelotKey::parse("8A").unwrap(), 0.4);
        assert!(low_conf.interval_width >= 2);
    }

    #[test]
    fn test_analyze_empty_track_id() {
        let engine = KeyAnalysisEngine::new();
        engine.analyze_key("", CamelotKey::parse("8A").unwrap(), 0.9);
        let analysis = engine.get_analysis("");
        assert!(analysis.is_some());
    }
}
