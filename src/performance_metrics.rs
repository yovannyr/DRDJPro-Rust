//! Performance Metrics — Real-time monitoring, sync analysis, statistics.
//!
//! Implements comprehensive performance monitoring, sync quality analysis,
//! throughput tracking, and statistical reporting for the DJ application.
//! Part of the Performance Metrics feature (P21a).
//!
//! ## Design principles
//!
//! * **Real-time** — Low-overhead monitoring.
//! * **Statistical** — Mean, median, percentiles, stddev.
//! * **Sync-aware** — Sync quality and drift analysis.
//! * **Visual** — Data suitable for UI charts/graphs.

use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Performance metric type.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetricType {
    /// CPU usage percentage.
    CpuUsage,
    /// Memory usage in MB.
    MemoryUsage,
    /// Audio callback duration in microseconds.
    CallbackDuration,
    /// I/O latency in microseconds.
    IoLatency,
    /// Network latency in milliseconds.
    NetworkLatency,
    /// Sample rate drift in ppm.
    SampleRateDrift,
    /// Buffer level (0-100).
    BufferLevel,
    /// Sync quality (0.0-1.0).
    SyncQuality,
    /// Frames per second (render).
    FramesPerSecond,
    /// UI response time in milliseconds.
    UiResponseTime,
    /// Custom metric.
    Custom(String),
}

/// Metric sample with timestamp.
#[derive(Debug, Clone)]
pub struct MetricSample {
    /// Metric type.
    pub metric_type: MetricType,
    /// Value.
    pub value: f64,
    /// Timestamp.
    pub timestamp: Instant,
}

impl MetricSample {
    /// Create a new metric sample.
    pub fn new(metric_type: MetricType, value: f64) -> Self {
        Self {
            metric_type,
            value,
            timestamp: Instant::now(),
        }
    }
}

/// Statistical summary of metric samples.
#[derive(Debug, Clone)]
pub struct MetricStatistics {
    /// Count of samples.
    pub count: usize,
    /// Mean value.
    pub mean: f64,
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// Median value.
    pub median: f64,
    /// Standard deviation.
    pub stddev: f64,
    /// 95th percentile.
    pub percentile_95: f64,
    /// 99th percentile.
    pub percentile_99: f64,
}

impl MetricStatistics {
    /// Calculate statistics from samples.
    pub fn calculate(samples: &[f64]) -> Self {
        if samples.is_empty() {
            return Self::empty();
        }

        let count = samples.len();
        let sum: f64 = samples.iter().sum();
        let mean = sum / count as f64;
        let min = samples.iter().cloned().fold(f64::INFINITY, f64::min);
        let max = samples.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        // Sort for median and percentiles.
        let mut sorted = samples.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());

        let median = if count.is_multiple_of(2) {
            (sorted[count / 2 - 1] + sorted[count / 2]) / 2.0
        } else {
            sorted[count / 2]
        };

        // Standard deviation.
        let variance: f64 = samples.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / count as f64;
        let stddev = variance.sqrt();

        // Percentiles.
        let p95_idx = (count as f64 * 0.95) as usize;
        let p99_idx = (count as f64 * 0.99) as usize;
        let p95 = sorted.get(p95_idx.min(count - 1)).copied().unwrap_or(max);
        let p99 = sorted.get(p99_idx.min(count - 1)).copied().unwrap_or(max);

        Self {
            count,
            mean,
            min,
            max,
            median,
            stddev,
            percentile_95: p95,
            percentile_99: p99,
        }
    }

    /// Return empty statistics.
    fn empty() -> Self {
        Self {
            count: 0,
            mean: 0.0,
            min: 0.0,
            max: 0.0,
            median: 0.0,
            stddev: 0.0,
            percentile_95: 0.0,
            percentile_99: 0.0,
        }
    }
}

/// Sync quality measurement.
#[derive(Debug, Clone)]
pub struct SyncQualityMeasurement {
    /// Deck A BPM.
    pub deck_a_bpm: f64,
    /// Deck B BPM.
    pub deck_b_bpm: f64,
    /// BPM difference.
    pub bpm_difference: f64,
    /// Phase offset in samples.
    pub phase_offset_samples: i64,
    /// Sync quality score (0.0-1.0, 1.0 = perfect).
    pub quality_score: f64,
    /// Timestamp.
    pub measured_at: Instant,
}

impl SyncQualityMeasurement {
    /// Create a new sync quality measurement.
    pub fn new(
        deck_a_bpm: f64,
        deck_b_bpm: f64,
        phase_offset_samples: i64,
        measured_at: Instant,
    ) -> Self {
        let bpm_diff = (deck_a_bpm - deck_b_bpm).abs();
        let bpm_difference = bpm_diff;
        // Quality score: decreases with BPM difference and phase offset.
        let quality_score =
            (1.0 - bpm_diff / 200.0 - (phase_offset_samples.abs() as f64 / 44100.0).min(0.5))
                .max(0.0);

        Self {
            deck_a_bpm,
            deck_b_bpm,
            bpm_difference,
            phase_offset_samples,
            quality_score,
            measured_at,
        }
    }

    /// Check if sync quality is acceptable.
    pub fn is_acceptable(&self, threshold: f64) -> bool {
        self.quality_score >= threshold
    }
}

/// Performance threshold alert.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertLevel {
    /// Informational.
    Info,
    /// Warning (approaching limits).
    Warning,
    /// Error (limit exceeded).
    Error,
    /// Critical (severe degradation).
    Critical,
}

impl AlertLevel {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Info => "Info",
            Self::Warning => "Warning",
            Self::Error => "Error",
            Self::Critical => "Critical",
        }
    }

    /// Get color for UI.
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Info => (100, 150, 200),   // Blue
            Self::Warning => (255, 200, 50), // Orange
            Self::Error => (255, 100, 50),   // Red-orange
            Self::Critical => (255, 50, 50), // Red
        }
    }
}

/// Performance alert.
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    /// Alert level.
    pub level: AlertLevel,
    /// Metric that triggered the alert.
    pub metric_type: MetricType,
    /// Current value.
    pub current_value: f64,
    /// Threshold that was exceeded.
    pub threshold: f64,
    /// Alert message.
    pub message: String,
    /// Alert time.
    pub alerted_at: Instant,
}

impl PerformanceAlert {
    /// Create a new performance alert.
    pub fn new(
        level: AlertLevel,
        metric_type: MetricType,
        current_value: f64,
        threshold: f64,
    ) -> Self {
        let message = match level {
            AlertLevel::Info => format!(
                "{} at {:.2} (info)",
                metric_type.display_name(),
                current_value
            ),
            AlertLevel::Warning => format!(
                "{} at {:.2} approaching threshold {:.2}",
                metric_type.display_name(),
                current_value,
                threshold
            ),
            AlertLevel::Error => format!(
                "{} at {:.2} exceeded threshold {:.2}",
                metric_type.display_name(),
                current_value,
                threshold
            ),
            AlertLevel::Critical => format!(
                "{} CRITICAL at {:.2} (threshold {:.2})",
                metric_type.display_name(),
                current_value,
                threshold
            ),
        };

        Self {
            level,
            metric_type,
            current_value,
            threshold,
            message,
            alerted_at: Instant::now(),
        }
    }
}

impl MetricType {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::CpuUsage => "CPU Usage",
            Self::MemoryUsage => "Memory Usage",
            Self::CallbackDuration => "Callback Duration",
            Self::IoLatency => "I/O Latency",
            Self::NetworkLatency => "Network Latency",
            Self::SampleRateDrift => "Sample Rate Drift",
            Self::BufferLevel => "Buffer Level",
            Self::SyncQuality => "Sync Quality",
            Self::FramesPerSecond => "FPS",
            Self::UiResponseTime => "UI Response Time",
            Self::Custom(name) => name,
        }
    }

    /// Get threshold for warning level.
    pub fn warning_threshold(&self) -> f64 {
        match self {
            Self::CpuUsage => 70.0,
            Self::MemoryUsage => 1024.0,     // MB
            Self::CallbackDuration => 500.0, // microseconds
            Self::IoLatency => 1000.0,       // microseconds
            Self::NetworkLatency => 100.0,   // ms
            Self::SampleRateDrift => 50.0,   // ppm
            Self::BufferLevel => 80.0,       // percent
            Self::SyncQuality => 0.5,
            Self::FramesPerSecond => 25.0,
            Self::UiResponseTime => 50.0, // ms
            Self::Custom(_) => 0.0,
        }
    }

    /// Get threshold for critical level.
    pub fn critical_threshold(&self) -> f64 {
        match self {
            Self::CpuUsage => 90.0,
            Self::MemoryUsage => 2048.0,      // MB
            Self::CallbackDuration => 1000.0, // microseconds
            Self::IoLatency => 5000.0,        // microseconds
            Self::NetworkLatency => 500.0,    // ms
            Self::SampleRateDrift => 200.0,   // ppm
            Self::BufferLevel => 95.0,        // percent
            Self::SyncQuality => 0.2,
            Self::FramesPerSecond => 15.0,
            Self::UiResponseTime => 200.0, // ms
            Self::Custom(_) => 0.0,
        }
    }
}

/// Performance metrics collector — collects and analyzes metric samples.
pub struct PerformanceMetrics {
    /// Metric history per type.
    metric_history: HashMap<MetricType, VecDeque<MetricSample>>,
    /// Recent alerts.
    alerts: VecDeque<PerformanceAlert>,
    /// Sync quality history.
    sync_measurements: VecDeque<SyncQualityMeasurement>,
    /// Session start time.
    session_start: Instant,
    /// Max samples per metric type.
    max_samples_per_metric: usize,
    /// Max alerts to keep.
    max_alerts: usize,
    /// Total metrics recorded.
    total_metrics_recorded: AtomicU64,
    /// Total alerts fired.
    total_alerts_fired: AtomicU32,
}

impl PerformanceMetrics {
    /// Create a new performance metrics collector.
    pub fn new(max_samples_per_metric: usize, max_alerts: usize) -> Self {
        Self {
            metric_history: HashMap::new(),
            alerts: VecDeque::new(),
            sync_measurements: VecDeque::new(),
            session_start: Instant::now(),
            max_samples_per_metric,
            max_alerts,
            total_metrics_recorded: AtomicU64::new(0),
            total_alerts_fired: AtomicU32::new(0),
        }
    }

    /// Record a metric sample.
    pub fn record(&mut self, sample: MetricSample) {
        let history = self
            .metric_history
            .entry(sample.metric_type.clone())
            .or_default();
        history.push_back(sample);

        // Trim to max size.
        while history.len() > self.max_samples_per_metric {
            history.pop_front();
        }

        self.total_metrics_recorded.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a metric value.
    pub fn record_value(&mut self, metric_type: MetricType, value: f64) {
        self.record(MetricSample::new(metric_type, value));
    }

    /// Get metric samples for a type.
    pub fn get_samples(&self, metric_type: MetricType) -> Vec<MetricSample> {
        self.metric_history
            .get(&metric_type)
            .map(|h| h.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get statistics for a metric type.
    pub fn get_statistics(&self, metric_type: MetricType) -> MetricStatistics {
        let samples = self.get_samples(metric_type);
        let values: Vec<f64> = samples.iter().map(|s| s.value).collect();
        MetricStatistics::calculate(&values)
    }

    /// Record a sync quality measurement.
    pub fn record_sync_quality(&mut self, measurement: SyncQualityMeasurement) {
        self.sync_measurements.push_back(measurement);

        // Trim.
        while self.sync_measurements.len() > 1000 {
            self.sync_measurements.pop_front();
        }
    }

    /// Get average sync quality.
    pub fn avg_sync_quality(&self) -> f64 {
        if self.sync_measurements.is_empty() {
            return 0.0;
        }
        self.sync_measurements
            .iter()
            .map(|m| m.quality_score)
            .sum::<f64>()
            / self.sync_measurements.len() as f64
    }

    /// Get sync quality trend (improving/stable/degrading).
    pub fn sync_quality_trend(&self) -> SyncTrend {
        if self.sync_measurements.len() < 20 {
            return SyncTrend::InsufficientData;
        }

        let recent: Vec<f64> = self
            .sync_measurements
            .iter()
            .rev()
            .take(10)
            .map(|m| m.quality_score)
            .collect();
        let older: Vec<f64> = self
            .sync_measurements
            .iter()
            .rev()
            .skip(10)
            .take(10)
            .map(|m| m.quality_score)
            .collect();

        let recent_avg = recent.iter().sum::<f64>() / recent.len() as f64;
        let older_avg = older.iter().sum::<f64>() / older.len() as f64;

        let diff = recent_avg - older_avg;
        if diff > 0.05 {
            SyncTrend::Improving
        } else if diff < -0.05 {
            SyncTrend::Degrading
        } else {
            SyncTrend::Stable
        }
    }

    /// Check for threshold violations and generate alerts.
    pub fn check_thresholds(
        &mut self,
        metric_type: MetricType,
        value: f64,
    ) -> Option<PerformanceAlert> {
        let critical = metric_type.critical_threshold();
        let warning = metric_type.warning_threshold();

        let alert = if value >= critical {
            Some(PerformanceAlert::new(
                AlertLevel::Critical,
                metric_type,
                value,
                critical,
            ))
        } else if value >= warning {
            Some(PerformanceAlert::new(
                AlertLevel::Warning,
                metric_type,
                value,
                warning,
            ))
        } else {
            None
        };

        if let Some(ref alert) = alert {
            self.alerts.push_back(alert.clone());
            while self.alerts.len() > self.max_alerts {
                self.alerts.pop_front();
            }
            self.total_alerts_fired.fetch_add(1, Ordering::Relaxed);
        }

        alert
    }

    /// Get recent alerts.
    pub fn recent_alerts(&self) -> Vec<&PerformanceAlert> {
        let count = self.alerts.len().min(50);
        self.alerts.iter().rev().take(count).collect()
    }

    /// Get alerts by level.
    pub fn alerts_by_level(&self, level: AlertLevel) -> Vec<&PerformanceAlert> {
        self.alerts.iter().filter(|a| a.level == level).collect()
    }

    /// Get session duration.
    pub fn session_duration(&self) -> Duration {
        self.session_start.elapsed()
    }

    /// Check if metrics are available for a type.
    pub fn has_metrics(&self, metric_type: MetricType) -> bool {
        self.metric_history.contains_key(&metric_type)
    }

    /// Get all available metric types.
    pub fn available_types(&self) -> Vec<MetricType> {
        self.metric_history.keys().cloned().collect()
    }

    /// Get total metrics recorded.
    pub fn total_metrics_recorded(&self) -> u64 {
        self.total_metrics_recorded.load(Ordering::Relaxed)
    }

    /// Get total alerts fired.
    pub fn total_alerts_fired(&self) -> u32 {
        self.total_alerts_fired.load(Ordering::Relaxed)
    }

    /// Clear all metrics.
    pub fn clear(&mut self) {
        self.metric_history.clear();
        self.sync_measurements.clear();
        self.alerts.clear();
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new(10000, 100)
    }
}

/// Sync trend direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncTrend {
    /// Improving.
    Improving,
    /// Stable.
    Stable,
    /// Degrading.
    Degrading,
    /// Insufficient data.
    InsufficientData,
}

impl SyncTrend {
    /// Get display name.
    pub fn display_name(&self) -> &str {
        match self {
            Self::Improving => "Improving",
            Self::Stable => "Stable",
            Self::Degrading => "Degrading",
            Self::InsufficientData => "Insufficient Data",
        }
    }

    /// Get indicator character.
    pub fn indicator(&self) -> &str {
        match self {
            Self::Improving => "↑",
            Self::Stable => "→",
            Self::Degrading => "↓",
            Self::InsufficientData => "?",
        }
    }
}

/// Performance report — summarizes all metrics.
#[derive(Debug, Clone)]
pub struct PerformanceReport {
    /// Session duration.
    pub session_duration: Duration,
    /// Total metrics recorded.
    pub total_metrics: u64,
    /// Total alerts fired.
    pub total_alerts: u32,
    /// Metric statistics by type.
    pub statistics: HashMap<MetricType, MetricStatistics>,
    /// Average sync quality.
    pub avg_sync_quality: f64,
    /// Sync quality trend.
    pub sync_trend: SyncTrend,
    /// Recent alerts.
    pub recent_alerts: Vec<String>,
}

impl PerformanceReport {
    /// Generate a performance report.
    pub fn generate(metrics: &PerformanceMetrics) -> Self {
        let available_types = metrics.available_types();
        let statistics: HashMap<MetricType, MetricStatistics> = available_types
            .iter()
            .map(|t| (t.clone(), metrics.get_statistics(t.clone())))
            .collect();

        let recent_alerts: Vec<String> = metrics
            .recent_alerts()
            .iter()
            .map(|a| a.message.clone())
            .collect();

        Self {
            session_duration: metrics.session_duration(),
            total_metrics: metrics.total_metrics_recorded(),
            total_alerts: metrics.total_alerts_fired(),
            statistics,
            avg_sync_quality: metrics.avg_sync_quality(),
            sync_trend: metrics.sync_quality_trend(),
            recent_alerts,
        }
    }

    /// Get summary string.
    pub fn summary(&self) -> String {
        format!(
            "Performance Report — Duration: {:.1}s | Metrics: {} | Alerts: {} | Avg Sync: {:.2} | Trend: {}",
            self.session_duration.as_secs_f64(),
            self.total_metrics,
            self.total_alerts,
            self.avg_sync_quality,
            self.sync_trend.display_name()
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_statistics_empty() {
        let stats = MetricStatistics::calculate(&[]);
        assert_eq!(stats.count, 0);
        assert_eq!(stats.mean, 0.0);
    }

    #[test]
    fn metric_statistics_basic() {
        let samples = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let stats = MetricStatistics::calculate(&samples);

        assert_eq!(stats.count, 5);
        assert_eq!(stats.mean, 30.0);
        assert_eq!(stats.min, 10.0);
        assert_eq!(stats.max, 50.0);
    }

    #[test]
    fn metric_statistics_median_even() {
        let samples = vec![10.0, 20.0, 30.0, 40.0];
        let stats = MetricStatistics::calculate(&samples);
        assert_eq!(stats.median, 25.0); // (20 + 30) / 2
    }

    #[test]
    fn metric_statistics_median_odd() {
        let samples = vec![10.0, 20.0, 30.0];
        let stats = MetricStatistics::calculate(&samples);
        assert_eq!(stats.median, 20.0);
    }

    #[test]
    fn metric_statistics_stddev() {
        let samples = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let stats = MetricStatistics::calculate(&samples);
        let expected_mean = 5.0;
        assert!((stats.mean - expected_mean).abs() < 0.01);
        assert!(stats.stddev > 0.0);
    }

    #[test]
    fn metric_type_names() {
        assert_eq!(MetricType::CpuUsage.display_name(), "CPU Usage");
        assert_eq!(MetricType::MemoryUsage.display_name(), "Memory Usage");
        assert_eq!(
            MetricType::CallbackDuration.display_name(),
            "Callback Duration"
        );
        assert_eq!(MetricType::SyncQuality.display_name(), "Sync Quality");
        assert_eq!(
            MetricType::Custom("Test".to_string()).display_name(),
            "Test"
        );
    }

    #[test]
    fn metric_type_thresholds() {
        assert_eq!(MetricType::CpuUsage.warning_threshold(), 70.0);
        assert_eq!(MetricType::CpuUsage.critical_threshold(), 90.0);
        assert_eq!(MetricType::CallbackDuration.warning_threshold(), 500.0);
        assert_eq!(MetricType::SyncQuality.warning_threshold(), 0.5);
    }

    #[test]
    fn alert_level_names() {
        assert_eq!(AlertLevel::Info.display_name(), "Info");
        assert_eq!(AlertLevel::Warning.display_name(), "Warning");
        assert_eq!(AlertLevel::Error.display_name(), "Error");
        assert_eq!(AlertLevel::Critical.display_name(), "Critical");
    }

    #[test]
    fn alert_level_colors() {
        assert_eq!(AlertLevel::Info.color(), (100, 150, 200));
        assert_eq!(AlertLevel::Warning.color(), (255, 200, 50));
        assert_eq!(AlertLevel::Error.color(), (255, 100, 50));
        assert_eq!(AlertLevel::Critical.color(), (255, 50, 50));
    }

    #[test]
    fn performance_metrics_record_and_read() {
        let mut metrics = PerformanceMetrics::default();

        metrics.record_value(MetricType::CpuUsage, 45.0);
        metrics.record_value(MetricType::CpuUsage, 55.0);
        metrics.record_value(MetricType::CpuUsage, 65.0);

        let samples = metrics.get_samples(MetricType::CpuUsage);
        assert_eq!(samples.len(), 3);
    }

    #[test]
    fn performance_metrics_statistics() {
        let mut metrics = PerformanceMetrics::default();
        for i in 0..10 {
            metrics.record_value(MetricType::MemoryUsage, i as f64 * 10.0);
        }

        let stats = metrics.get_statistics(MetricType::MemoryUsage);
        assert_eq!(stats.count, 10);
        assert_eq!(stats.mean, 45.0);
        assert_eq!(stats.min, 0.0);
        assert_eq!(stats.max, 90.0);
    }

    #[test]
    fn performance_metrics_threshold_alert() {
        let mut metrics = PerformanceMetrics::default();

        // Warning level.
        let alert = metrics.check_thresholds(MetricType::CpuUsage, 75.0);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().level, AlertLevel::Warning);

        // Critical level.
        let alert = metrics.check_thresholds(MetricType::CpuUsage, 95.0);
        assert!(alert.is_some());
        assert_eq!(alert.unwrap().level, AlertLevel::Critical);

        // Below warning.
        let alert = metrics.check_thresholds(MetricType::CpuUsage, 30.0);
        assert!(alert.is_none());
    }

    #[test]
    fn performance_metrics_recent_alerts() {
        let mut metrics = PerformanceMetrics::default();

        metrics.check_thresholds(MetricType::CpuUsage, 75.0);
        metrics.check_thresholds(MetricType::MemoryUsage, 1500.0);

        let alerts = metrics.recent_alerts();
        assert_eq!(alerts.len(), 2);
    }

    #[test]
    fn performance_metrics_sync_quality() {
        let mut metrics = PerformanceMetrics::default();

        metrics.record_sync_quality(SyncQualityMeasurement::new(128.0, 128.0, 0, Instant::now()));
        metrics.record_sync_quality(SyncQualityMeasurement::new(
            128.0,
            128.5,
            100,
            Instant::now(),
        ));

        let avg = metrics.avg_sync_quality();
        assert!(avg > 0.0 && avg <= 1.0);
    }

    #[test]
    fn performance_metrics_sync_trend() {
        let mut metrics = PerformanceMetrics::default();

        // Start with poor quality.
        for _ in 0..10 {
            metrics.record_sync_quality(SyncQualityMeasurement::new(
                128.0,
                130.0,
                4410, // 100 ms offset: a measurable quality decline.
                Instant::now(),
            ));
        }
        // Then improve.
        for _ in 0..10 {
            metrics.record_sync_quality(SyncQualityMeasurement::new(
                128.0,
                128.0,
                0,
                Instant::now(),
            ));
        }

        let trend = metrics.sync_quality_trend();
        assert!(matches!(trend, SyncTrend::Improving));
    }

    #[test]
    fn performance_metrics_session_duration() {
        let mut metrics = PerformanceMetrics::default();
        metrics.session_start = Instant::now() - Duration::from_secs(1);
        assert!(metrics.session_duration() >= Duration::from_secs(1));
    }

    #[test]
    fn performance_metrics_total_counts() {
        let mut metrics = PerformanceMetrics::default();
        assert_eq!(metrics.total_metrics_recorded(), 0);
        assert_eq!(metrics.total_alerts_fired(), 0);

        metrics.record_value(MetricType::CpuUsage, 50.0);
        assert_eq!(metrics.total_metrics_recorded(), 1);

        metrics.check_thresholds(MetricType::CpuUsage, 95.0);
        assert_eq!(metrics.total_alerts_fired(), 1);
    }

    #[test]
    fn performance_metrics_has_metrics() {
        let mut metrics = PerformanceMetrics::default();
        assert!(!metrics.has_metrics(MetricType::CpuUsage));

        metrics.record_value(MetricType::CpuUsage, 50.0);
        assert!(metrics.has_metrics(MetricType::CpuUsage));
    }

    #[test]
    fn performance_metrics_available_types() {
        let mut metrics = PerformanceMetrics::default();
        assert!(metrics.available_types().is_empty());

        metrics.record_value(MetricType::CpuUsage, 50.0);
        metrics.record_value(MetricType::MemoryUsage, 512.0);

        let types = metrics.available_types();
        assert_eq!(types.len(), 2);
        assert!(types.contains(&MetricType::CpuUsage));
        assert!(types.contains(&MetricType::MemoryUsage));
    }

    #[test]
    fn performance_metrics_clear() {
        let mut metrics = PerformanceMetrics::default();
        metrics.record_value(MetricType::CpuUsage, 50.0);
        metrics.check_thresholds(MetricType::CpuUsage, 95.0);

        metrics.clear();
        assert!(metrics.available_types().is_empty());
        assert!(metrics.recent_alerts().is_empty());
    }

    #[test]
    fn sync_quality_measurement() {
        let measurement = SyncQualityMeasurement::new(128.0, 128.0, 0, Instant::now());
        assert_eq!(measurement.bpm_difference, 0.0);
        assert_eq!(measurement.phase_offset_samples, 0);
        assert!((measurement.quality_score - 1.0).abs() < 0.01);
    }

    #[test]
    fn sync_quality_measurement_degraded() {
        let measurement = SyncQualityMeasurement::new(128.0, 140.0, 2000, Instant::now());
        assert!(measurement.bpm_difference > 0.0);
        assert!(measurement.quality_score < 1.0);
        assert!(measurement.quality_score >= 0.0);
    }

    #[test]
    fn sync_quality_acceptable() {
        let measurement = SyncQualityMeasurement::new(128.0, 128.0, 0, Instant::now());
        assert!(measurement.is_acceptable(0.9));
    }

    #[test]
    fn sync_trend_names() {
        assert_eq!(SyncTrend::Improving.display_name(), "Improving");
        assert_eq!(SyncTrend::Stable.display_name(), "Stable");
        assert_eq!(SyncTrend::Degrading.display_name(), "Degrading");
        assert_eq!(
            SyncTrend::InsufficientData.display_name(),
            "Insufficient Data"
        );
    }

    #[test]
    fn sync_trend_indicators() {
        assert_eq!(SyncTrend::Improving.indicator(), "↑");
        assert_eq!(SyncTrend::Stable.indicator(), "→");
        assert_eq!(SyncTrend::Degrading.indicator(), "↓");
        assert_eq!(SyncTrend::InsufficientData.indicator(), "?");
    }

    #[test]
    fn performance_report() {
        let mut metrics = PerformanceMetrics::default();
        metrics.record_value(MetricType::CpuUsage, 50.0);
        metrics.record_value(MetricType::MemoryUsage, 512.0);
        metrics.check_thresholds(MetricType::CpuUsage, 95.0);

        let report = PerformanceReport::generate(&metrics);
        let summary = report.summary();
        assert!(summary.contains("Performance Report"));
        assert!(summary.contains("Duration"));
        assert!(summary.contains("Metrics: 2"));
    }

    #[test]
    fn performance_metrics_sample_limit() {
        let metrics = PerformanceMetrics::new(5, 100);
        let mut metrics = metrics; // Make mutable.

        for i in 0..10 {
            metrics.record_value(MetricType::CpuUsage, i as f64);
        }

        let samples = metrics.get_samples(MetricType::CpuUsage);
        // Should be capped at max_samples_per_metric.
        assert!(samples.len() <= 5);
    }
}
