//! Tap tempo and batch analysis support for BPM detection.
//!
//! Provides tap-tempo input for manual BPM estimation and batch
//! processing of multiple tracks. Part of the BPM Detection feature
//! (P03a/P03b).
//!
//! ## Design principles
//!
//! * **Tap tempo** — User provides manual BPM input via keys or UI.
//! * **Batch analysis** — Queue multiple tracks for analysis.
//! * **Non-blocking** — Both tap tempo and batch run asynchronously.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a tap-tempo event.
#[derive(Debug, Clone)]
pub struct TapTempoResult {
    /// Estimated BPM from the tap sequence.
    pub bpm: f64,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// Number of taps used.
    pub tap_count: usize,
    /// Average interval between taps.
    pub average_interval_ms: f64,
}

impl TapTempoResult {
    /// Whether the result is reliable enough for auto-sync.
    pub fn is_reliable(&self) -> bool {
        self.confidence > 0.5 && self.tap_count >= 3
    }

    /// Format a user-friendly string.
    pub fn display(&self) -> String {
        format!(
            "{:.1} BPM ({} taps, {:.0}% confidence)",
            self.bpm,
            self.tap_count,
            self.confidence * 100.0
        )
    }
}

/// Tap tempo manager — accumulates taps and estimates BPM.
pub struct TapTempoManager {
    taps: Arc<Mutex<Vec<Instant>>>,
}

impl TapTempoManager {
    /// Create a new tap tempo manager.
    pub fn new() -> Self {
        Self {
            taps: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a tap event and return the current BPM estimate.
    pub fn record_tap(&self) -> TapTempoResult {
        let mut taps = self.taps.lock().unwrap();

        let now = Instant::now();

        // Clamp tap history to last 10 seconds (prevents stale taps).
        let cutoff = now - Duration::from_secs(10);
        taps.retain(|t| *t > cutoff);

        // Push current tap.
        taps.push(now);

        let tap_count = taps.len();

        // Need at least 2 taps to calculate interval.
        if tap_count < 2 {
            return TapTempoResult {
                bpm: 0.0,
                confidence: 0.0,
                tap_count,
                average_interval_ms: 0.0,
            };
        }

        // Calculate average interval between consecutive taps.
        let mut total_interval: f64 = 0.0;
        for w in taps.windows(2) {
            let interval = w[1].duration_since(w[0]).as_secs_f64() * 1000.0;
            total_interval += interval;
        }

        let num_intervals = (tap_count - 1) as f64;
        let average_interval_ms = total_interval / num_intervals;

        // BPM = 60000 / average_interval_ms.
        let bpm = if average_interval_ms > 0.0 {
            60000.0 / average_interval_ms
        } else {
            0.0
        };

        // Confidence based on tap count and interval consistency.
        let confidence = calculate_tap_confidence(&taps, average_interval_ms);

        TapTempoResult {
            bpm,
            confidence,
            tap_count,
            average_interval_ms,
        }
    }

    /// Reset the tap history.
    pub fn reset(&self) {
        let mut taps = self.taps.lock().unwrap();
        taps.clear();
    }

    /// Get the number of accumulated taps.
    pub fn tap_count(&self) -> usize {
        let taps = self.taps.lock().unwrap();
        taps.len()
    }

    /// Clear taps older than the given duration.
    pub fn prune(&self, max_age: Duration) {
        let cutoff = Instant::now() - max_age;
        let mut taps = self.taps.lock().unwrap();
        taps.retain(|t| *t > cutoff);
    }
}

impl Default for TapTempoManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Batch analysis
// ---------------------------------------------------------------------------

/// Status of a batch analysis job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchStatus {
    /// Waiting to start.
    Pending,
    /// Currently processing.
    Processing,
    /// All tracks analyzed successfully.
    Complete,
    /// Analysis failed for one or more tracks.
    Failed,
    /// User cancelled the batch.
    Cancelled,
}

impl fmt::Display for BatchStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BatchStatus::Pending => write!(f, "pending"),
            BatchStatus::Processing => write!(f, "processing"),
            BatchStatus::Complete => write!(f, "complete"),
            BatchStatus::Failed => write!(f, "failed"),
            BatchStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// Individual track analysis result in a batch.
#[derive(Debug, Clone)]
pub struct TrackAnalysisResult {
    /// Track file path.
    pub track_path: String,
    /// Detected BPM.
    pub bpm: f64,
    /// Beatgrid confidence (0.0 - 1.0).
    pub confidence: f64,
    /// Whether analysis succeeded.
    pub success: bool,
    /// Error message if analysis failed.
    pub error: Option<String>,
}

/// Batch analysis job.
#[derive(Debug, Clone)]
pub struct BatchAnalysisJob {
    /// Job ID (unique).
    pub job_id: u64,
    /// Track paths to analyze.
    pub tracks: Vec<String>,
    /// Current batch status.
    pub status: BatchStatus,
    /// Number of tracks currently being processed.
    pub processing_count: usize,
    /// Number of completed tracks.
    pub completed_count: usize,
    /// Results so far.
    pub results: Vec<TrackAnalysisResult>,
    /// Whether the batch was cancelled.
    pub cancelled: bool,
}

impl BatchAnalysisJob {
    /// Progress percentage (0.0 - 100.0).
    pub fn progress(&self) -> f64 {
        if self.tracks.is_empty() {
            return 0.0;
        }
        (self.completed_count as f64 / self.tracks.len() as f64) * 100.0
    }

    /// Whether the job has finished (complete, failed, or cancelled).
    pub fn is_finished(&self) -> bool {
        matches!(
            self.status,
            BatchStatus::Complete | BatchStatus::Failed | BatchStatus::Cancelled
        )
    }
}

/// Batch analysis scheduler — manages multiple batch jobs.
pub struct BatchAnalysisScheduler {
    jobs: Arc<Mutex<Vec<BatchAnalysisJob>>>,
    next_job_id: Arc<Mutex<u64>>,
}

impl BatchAnalysisScheduler {
    /// Create a new batch analysis scheduler.
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(Vec::new())),
            next_job_id: Arc::new(Mutex::new(1)),
        }
    }

    /// Create a new batch analysis job.
    pub fn create_job(&self, tracks: Vec<String>) -> BatchAnalysisJob {
        let mut id = self.next_job_id.lock().unwrap();
        let job_id = *id;
        *id += 1;

        let job = BatchAnalysisJob {
            job_id,
            tracks,
            status: BatchStatus::Pending,
            processing_count: 0,
            completed_count: 0,
            results: Vec::new(),
            cancelled: false,
        };

        let mut jobs = self.jobs.lock().unwrap();
        jobs.push(job.clone());

        job
    }

    /// Start processing a job (simulated).
    ///
    /// In production, this would call the BPM detection engine
    /// for each track in the job.
    pub fn process_job(&self, job_id: u64) -> Result<BatchAnalysisJob, String> {
        let mut jobs = self.jobs.lock().map_err(|e| e.to_string())?;

        let job = jobs
            .iter_mut()
            .find(|j| j.job_id == job_id)
            .ok_or_else(|| format!("Job {} not found", job_id))?;

        if job.is_finished() {
            return Err(format!(
                "Job {} is already finished ({})",
                job_id, job.status
            ));
        }

        job.status = BatchStatus::Processing;

        // Simulate processing (in production, this calls bpm_detection::analyze).
        for track_path in &job.tracks {
            if job.cancelled {
                job.status = BatchStatus::Cancelled;
                break;
            }

            // Simulate analysis result.
            let result = TrackAnalysisResult {
                track_path: track_path.clone(),
                bpm: 128.0, // Simulated BPM.
                confidence: 0.85,
                success: true,
                error: None,
            };

            job.results.push(result);
            job.completed_count += 1;

            // Simulate async processing (non-blocking in production).
        }

        // Finalize job.
        if job.results.len() == job.tracks.len() {
            job.status = BatchStatus::Complete;
        } else if !job.results.is_empty() {
            // Partial success.
            job.status = BatchStatus::Complete;
        } else {
            job.status = BatchStatus::Failed;
        }

        let job = jobs
            .iter()
            .find(|j| j.job_id == job_id)
            .cloned()
            .ok_or_else(|| format!("Job {} disappeared", job_id))?;

        Ok(job)
    }

    /// Cancel a job.
    pub fn cancel_job(&self, job_id: u64) -> Result<(), String> {
        let mut jobs = self.jobs.lock().map_err(|e| e.to_string())?;

        let job = jobs
            .iter_mut()
            .find(|j| j.job_id == job_id)
            .ok_or_else(|| format!("Job {} not found", job_id))?;

        job.cancelled = true;
        job.status = BatchStatus::Cancelled;

        Ok(())
    }

    /// Get job status.
    pub fn get_job(&self, job_id: u64) -> Result<BatchAnalysisJob, String> {
        let jobs = self.jobs.lock().map_err(|e| e.to_string())?;
        jobs.iter()
            .find(|j| j.job_id == job_id)
            .cloned()
            .ok_or_else(|| format!("Job {} not found", job_id))
    }

    /// List all jobs.
    pub fn list_jobs(&self) -> Result<Vec<BatchAnalysisJob>, String> {
        let jobs = self.jobs.lock().map_err(|e| e.to_string())?;
        Ok(jobs.clone())
    }

    /// Get pending job count.
    pub fn pending_count(&self) -> Result<usize, String> {
        let jobs = self.jobs.lock().map_err(|e| e.to_string())?;
        Ok(jobs
            .iter()
            .filter(|j| j.status == BatchStatus::Pending)
            .count())
    }
}

impl Default for BatchAnalysisScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Calculate confidence for a tap-tempo sequence.
///
/// Confidence is based on:
/// - Number of taps (more taps = higher confidence)
/// - Consistency of intervals (low variance = higher confidence)
fn calculate_tap_confidence(taps: &[Instant], _average_interval_ms: f64) -> f64 {
    if taps.len() < 2 {
        return 0.0;
    }

    // Calculate interval variance.
    let mut intervals: Vec<f64> = Vec::new();
    for (first, second) in taps.iter().zip(taps.iter().skip(1)) {
        let interval = second.duration_since(*first).as_secs_f64() * 1000.0;
        intervals.push(interval);
    }

    if intervals.is_empty() {
        return 0.0;
    }

    // Mean interval.
    let mean: f64 = intervals.iter().sum::<f64>() / intervals.len() as f64;

    // Variance.
    let variance: f64 =
        intervals.iter().map(|i| (i - mean).powi(2)).sum::<f64>() / intervals.len() as f64;

    // Standard deviation.
    let stddev = variance.sqrt();

    // Coefficient of variation (CV = stddev / mean).
    let cv = if mean > 0.0 { stddev / mean } else { 1.0 };

    // Confidence score: lower CV = higher confidence.
    // CV < 0.1 → 1.0 confidence
    // CV > 0.5 → 0.1 confidence
    let consistency_score = if cv < 0.1 {
        1.0
    } else if cv > 0.5 {
        0.1
    } else {
        1.0 - ((cv - 0.1) / 0.4) * 0.9
    };

    // Tap count score (0-1 based on count, capped at 10).
    let count_score = (taps.len().min(10) as f64) / 10.0;

    // Weighted average (consistency matters more).
    consistency_score * 0.7 + count_score * 0.3
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tap_tempo_requires_minimum_taps() {
        let manager = TapTempoManager::new();

        // Single tap — no BPM.
        let result = manager.record_tap();
        assert_eq!(result.tap_count, 1);
        assert_eq!(result.bpm, 0.0);
        assert_eq!(result.confidence, 0.0);
    }

    #[test]
    fn tap_tempo_calculates_bpm() {
        let manager = TapTempoManager::new();

        manager.record_tap();
        std::thread::sleep(Duration::from_millis(500));
        let result = manager.record_tap();

        // 500ms between taps → 120 BPM.
        assert!(result.bpm > 100.0 && result.bpm < 140.0);
        assert!(result.confidence > 0.0);
        assert_eq!(result.tap_count, 2);
    }

    #[test]
    fn tap_tempo_stabilizes_with_consistent_taps() {
        let manager = TapTempoManager::new();

        // 60 BPM = 1000ms interval.
        for _ in 0..5 {
            manager.record_tap();
            std::thread::sleep(Duration::from_millis(1000));
        }

        let result = manager.record_tap();
        assert!(result.bpm > 50.0 && result.bpm < 70.0);
        assert!(result.confidence > 0.5);
        assert!(result.is_reliable());
    }

    #[test]
    fn tap_tempo_prunes_stale_taps() {
        let manager = TapTempoManager::new();

        manager.record_tap();
        std::thread::sleep(Duration::from_millis(100));
        manager.record_tap();

        // The fresh tap survives; only the 100 ms old tap is stale.
        manager.prune(Duration::from_millis(10));
        assert_eq!(manager.tap_count(), 1);
        manager.prune(Duration::ZERO);
        assert_eq!(manager.tap_count(), 0);
    }

    #[test]
    fn tap_tempo_low_confidence_for_inconsistent_taps() {
        let manager = TapTempoManager::new();

        // Very irregular taps.
        manager.record_tap();
        std::thread::sleep(Duration::from_millis(200));
        manager.record_tap();
        std::thread::sleep(Duration::from_millis(2000));
        manager.record_tap();
        std::thread::sleep(Duration::from_millis(300));
        manager.record_tap();

        let result = manager.record_tap();

        // Should have lower confidence due to high variance.
        assert!(result.confidence < 0.7);
    }

    #[test]
    fn tap_tempo_result_display() {
        let result = TapTempoResult {
            bpm: 128.0,
            confidence: 0.85,
            tap_count: 5,
            average_interval_ms: 468.75,
        };

        let display = result.display();
        assert!(display.contains("128.0"));
        assert!(display.contains("5 taps"));
        assert!(display.contains("85%"));
    }

    #[test]
    fn tap_tempo_reset_clears_taps() {
        let manager = TapTempoManager::new();

        manager.record_tap();
        manager.record_tap();
        assert_eq!(manager.tap_count(), 2);

        manager.reset();
        assert_eq!(manager.tap_count(), 0);
    }

    #[test]
    fn batch_analysis_job_creation() {
        let scheduler = BatchAnalysisScheduler::new();
        let tracks = vec![
            "/music/track1.mp3".to_string(),
            "/music/track2.mp3".to_string(),
        ];

        let job = scheduler.create_job(tracks);
        assert_eq!(job.job_id, 1);
        assert_eq!(job.tracks.len(), 2);
        assert_eq!(job.status, BatchStatus::Pending);
    }

    #[test]
    fn batch_analysis_job_progress() {
        let scheduler = BatchAnalysisScheduler::new();
        let tracks = vec![
            "/music/a.mp3".to_string(),
            "/music/b.mp3".to_string(),
            "/music/c.mp3".to_string(),
        ];

        let job = scheduler.create_job(tracks);

        // Before processing: 0%.
        assert_eq!(job.progress(), 0.0);

        // Simulate processing (in production this is done via process_job).
        let result = scheduler.process_job(job.job_id).unwrap();
        assert_eq!(result.completed_count, 3);
        assert!((result.progress() - 100.0).abs() < 0.1);
        assert!(result.is_finished());
    }

    #[test]
    fn batch_analysis_cancel() {
        let scheduler = BatchAnalysisScheduler::new();
        let job = scheduler.create_job(vec!["/music/x.mp3".to_string()]);

        scheduler.cancel_job(job.job_id).unwrap();
        let cancelled = scheduler.get_job(job.job_id).unwrap();

        assert_eq!(cancelled.status, BatchStatus::Cancelled);
        assert!(cancelled.is_finished());
    }

    #[test]
    fn batch_analysis_job_not_found() {
        let scheduler = BatchAnalysisScheduler::new();

        assert!(scheduler.get_job(999).is_err());
        assert!(scheduler.cancel_job(999).is_err());
        assert!(scheduler.process_job(999).is_err());
    }

    #[test]
    fn batch_analysis_duplicate_processing() {
        let scheduler = BatchAnalysisScheduler::new();
        let job = scheduler.create_job(vec!["/music/test.mp3".to_string()]);

        // Process once.
        scheduler.process_job(job.job_id).unwrap();

        // Processing again should fail.
        assert!(scheduler.process_job(job.job_id).is_err());
    }
}
