//! Performance Monitoring — CPU/Memory/Callback-Zeit, Latenz, Puffer, Threading, Alerts, Export.
//!
//! Implements performance monitoring for real-time audio processing.
//! Tracks CPU load, memory usage, callback timing, buffer levels,
//! thread congestion, and latency — with alerts and export support.
//! Part of the Performance Monitoring feature (P15a).
//!
//! ## Design principles
//!
//! * **Non-blocking** — Monitoring must not affect real-time audio.
//! * **Low overhead** — Monitoring overhead <1% of CPU.
//! * **Ring buffers** — Circular buffers for time-series metrics.
//! * **Threshold alerts** — Configurable warning/error thresholds.

use crate::atomic_float::AtomicF32;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Performance metric type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetricType {
    /// CPU usage percentage (0-100).
    CpuUsage,
    /// Memory usage in MB.
    MemoryUsage,
    /// Audio callback duration (μs).
    CallbackDuration,
    /// Audio latency (ms).
    Latency,
    /// Buffer fill level (0-100).
    BufferLevel,
    /// Dropouts per minute.
    Dropouts,
    /// Thread count.
    ThreadCount,
    /// Thread congestion (blocked time ratio).
    ThreadCongestion,
    /// Disk I/O wait (ms).
    DiskWait,
    /// Network latency (ms).
    NetworkLatency,
}

impl MetricType {
    /// Get display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::CpuUsage => "CPU Usage",
            Self::MemoryUsage => "Memory Usage",
            Self::CallbackDuration => "Callback Duration",
            Self::Latency => "Latency",
            Self::BufferLevel => "Buffer Level",
            Self::Dropouts => "Dropouts",
            Self::ThreadCount => "Thread Count",
            Self::ThreadCongestion => "Thread Congestion",
            Self::DiskWait => "Disk Wait",
            Self::NetworkLatency => "Network Latency",
        }
    }

    /// Get unit string.
    pub fn unit(&self) -> &'static str {
        match self {
            Self::CpuUsage => "%",
            Self::MemoryUsage => "MB",
            Self::CallbackDuration => "μs",
            Self::Latency => "ms",
            Self::BufferLevel => "%",
            Self::Dropouts => "/min",
            Self::ThreadCount => "threads",
            Self::ThreadCongestion => "%",
            Self::DiskWait => "ms",
            Self::NetworkLatency => "ms",
        }
    }
}

/// Metric value with timestamp.
#[derive(Debug, Clone)]
pub struct MetricSample {
    /// Metric type.
    pub metric: MetricType,
    /// Value.
    pub value: f64,
    /// Timestamp.
    pub timestamp: Instant,
}

/// Performance alert level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertLevel {
    /// Informational.
    Info,
    /// Warning.
    Warning,
    /// Error.
    Error,
    /// Critical.
    Critical,
}

impl AlertLevel {
    /// Get display color (R, G, B).
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            Self::Info => (0, 120, 255),     // Blue
            Self::Warning => (255, 165, 0),  // Orange
            Self::Error => (255, 0, 0),      // Red
            Self::Critical => (255, 0, 255), // Magenta
        }
    }
}

/// Performance alert.
#[derive(Debug, Clone)]
pub struct PerformanceAlert {
    /// Alert level.
    pub level: AlertLevel,
    /// Alert message.
    pub message: String,
    /// Metric that triggered the alert.
    pub metric: MetricType,
    /// Threshold that was exceeded.
    pub threshold: f64,
    /// Actual value.
    pub value: f64,
    /// Alert timestamp.
    pub timestamp: Instant,
    /// Acknowledged.
    pub acknowledged: bool,
}

impl PerformanceAlert {
    /// Create a new alert.
    pub fn new(
        level: AlertLevel,
        message: String,
        metric: MetricType,
        threshold: f64,
        value: f64,
    ) -> Self {
        Self {
            level,
            message,
            metric,
            threshold,
            value,
            timestamp: Instant::now(),
            acknowledged: false,
        }
    }
}

/// Performance threshold configuration.
#[derive(Debug, Clone)]
pub struct ThresholdConfig {
    /// CPU usage warning (percentage).
    pub cpu_warning: f64,
    /// CPU usage error (percentage).
    pub cpu_error: f64,
    /// Memory usage warning (MB).
    pub memory_warning: f64,
    /// Memory usage error (MB).
    pub memory_error: f64,
    /// Callback duration warning (μs).
    pub callback_warning: f64,
    /// Callback duration error (μs).
    pub callback_error: f64,
    /// Latency warning (ms).
    pub latency_warning: f64,
    /// Latency error (ms).
    pub latency_error: f64,
    /// Buffer level warning (percentage).
    pub buffer_warning: f64,
    /// Buffer level error (percentage).
    pub buffer_error: f64,
}

impl Default for ThresholdConfig {
    fn default() -> Self {
        Self {
            cpu_warning: 70.0,
            cpu_error: 90.0,
            memory_warning: 512.0,
            memory_error: 1024.0,
            callback_warning: 500.0, // 500μs = 0.5ms
            callback_error: 1000.0,  // 1ms
            latency_warning: 10.0,   // 10ms
            latency_error: 20.0,     // 20ms
            buffer_warning: 20.0,    // 20%
            buffer_error: 10.0,      // 10%
        }
    }
}

/// Performance stats snapshot.
#[derive(Debug, Clone)]
pub struct PerformanceSnapshot {
    /// CPU usage percentage.
    pub cpu_usage: f64,
    /// Memory usage in MB.
    pub memory_usage: f64,
    /// Average callback duration (μs).
    pub avg_callback_duration: f64,
    /// Max callback duration (μs).
    pub max_callback_duration: f64,
    /// Current latency (ms).
    pub latency: f64,
    /// Buffer fill level (0-100).
    pub buffer_level: f64,
    /// Dropouts in last minute.
    pub dropouts_per_min: u32,
    /// Thread count.
    pub thread_count: u32,
    /// Thread congestion percentage.
    pub thread_congestion: f64,
    /// Disk wait time (ms).
    pub disk_wait_ms: f64,
    /// Network latency (ms).
    pub network_latency_ms: f64,
    /// Snapshot timestamp.
    pub timestamp: Instant,
}

/// Performance monitoring engine.
pub struct PerformanceMonitor {
    /// Threshold configuration.
    thresholds: ThresholdConfig,
    /// Active monitoring.
    monitoring: AtomicBool,
    /// Metric history buffers.
    cpu_history: Arc<std::sync::Mutex<VecDeque<f64>>>,
    memory_history: Arc<std::sync::Mutex<VecDeque<f64>>>,
    callback_history: Arc<std::sync::Mutex<VecDeque<f64>>>,
    latency_history: Arc<std::sync::Mutex<VecDeque<f64>>>,
    buffer_history: Arc<std::sync::Mutex<VecDeque<f64>>>,
    /// Alert history.
    alerts: std::sync::Mutex<Vec<PerformanceAlert>>,
    /// Unacknowledged alert count.
    unacknowledged: AtomicUsize,
    /// Sample counter.
    samples: AtomicU64,
    /// Dropout counter.
    dropouts: AtomicUsize,
    /// Last callback time.
    last_callback: std::sync::Mutex<Option<Instant>>,
    /// Peak callback duration.
    peak_callback: AtomicF32,
}

impl PerformanceMonitor {
    /// Create a new performance monitor.
    pub fn new(thresholds: ThresholdConfig) -> Self {
        Self {
            thresholds,
            monitoring: AtomicBool::new(false),
            cpu_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(300))),
            memory_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(300))),
            callback_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(300))),
            latency_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(300))),
            buffer_history: Arc::new(std::sync::Mutex::new(VecDeque::with_capacity(300))),
            alerts: std::sync::Mutex::new(Vec::new()),
            unacknowledged: AtomicUsize::new(0),
            samples: AtomicU64::new(0),
            dropouts: AtomicUsize::new(0),
            last_callback: std::sync::Mutex::new(None),
            peak_callback: AtomicF32::new(0.0),
        }
    }

    /// Start monitoring.
    pub fn start(&self) {
        self.monitoring.store(true, Ordering::Relaxed);
    }

    /// Stop monitoring.
    pub fn stop(&self) {
        self.monitoring.store(false, Ordering::Relaxed);
    }

    /// Check if monitoring is active.
    pub fn is_monitoring(&self) -> bool {
        self.monitoring.load(Ordering::Relaxed)
    }

    /// Record CPU usage sample.
    pub fn record_cpu(&self, cpu_percent: f64) {
        if !self.monitoring.load(Ordering::Relaxed) {
            return;
        }

        self.samples.fetch_add(1, Ordering::Relaxed);

        let mut history = self.cpu_history.lock().unwrap();
        history.push_back(cpu_percent);
        if history.len() > 300 {
            history.pop_front();
        }

        self.check_thresholds(MetricType::CpuUsage, cpu_percent);
    }

    /// Record memory usage sample.
    pub fn record_memory(&self, memory_mb: f64) {
        if !self.monitoring.load(Ordering::Relaxed) {
            return;
        }

        let mut history = self.memory_history.lock().unwrap();
        history.push_back(memory_mb);
        if history.len() > 300 {
            history.pop_front();
        }

        self.check_thresholds(MetricType::MemoryUsage, memory_mb);
    }

    /// Record audio callback duration.
    pub fn record_callback_duration(&self, duration_us: u64) {
        if !self.monitoring.load(Ordering::Relaxed) {
            return;
        }

        let duration_f = duration_us as f64;
        self.samples.fetch_add(1, Ordering::Relaxed);

        // Update peak.
        let peak = self.peak_callback.load(Ordering::Relaxed);
        if duration_f as f32 > peak {
            self.peak_callback
                .store(duration_f as f32, Ordering::Relaxed);
        }

        let mut history = self.callback_history.lock().unwrap();
        history.push_back(duration_f);
        if history.len() > 300 {
            history.pop_front();
        }

        self.check_thresholds(MetricType::CallbackDuration, duration_f);

        // Check for dropouts.
        if duration_f > self.thresholds.callback_error * 2.0 {
            self.dropouts.fetch_add(1, Ordering::Relaxed);
        }

        *self.last_callback.lock().unwrap() = Some(Instant::now());
    }

    /// Record latency sample.
    pub fn record_latency(&self, latency_ms: f64) {
        if !self.monitoring.load(Ordering::Relaxed) {
            return;
        }

        let mut history = self.latency_history.lock().unwrap();
        history.push_back(latency_ms);
        if history.len() > 300 {
            history.pop_front();
        }

        self.check_thresholds(MetricType::Latency, latency_ms);
    }

    /// Record buffer level.
    pub fn record_buffer_level(&self, level_percent: f64) {
        if !self.monitoring.load(Ordering::Relaxed) {
            return;
        }

        let mut history = self.buffer_history.lock().unwrap();
        history.push_back(level_percent);
        if history.len() > 300 {
            history.pop_front();
        }

        self.check_thresholds(MetricType::BufferLevel, level_percent);
    }

    /// Record dropout event.
    pub fn record_dropout(&self) {
        self.dropouts.fetch_add(1, Ordering::Relaxed);
        self.add_alert(
            AlertLevel::Error,
            format!(
                "Audio dropout detected (buffer level: {:.1}%)",
                self.current_buffer_level()
            ),
            MetricType::Dropouts,
            0.0,
            self.dropouts.load(Ordering::Relaxed) as f64,
        );
    }

    /// Get current performance snapshot.
    pub fn snapshot(&self) -> PerformanceSnapshot {
        let cpu_avg = self.avg_metric(&self.cpu_history);
        let mem_avg = self.avg_metric(&self.memory_history);
        let callback_avg = self.avg_metric(&self.callback_history);
        let callback_max = self.peak_callback.load(Ordering::Relaxed) as f64;
        let latency_avg = self.avg_metric(&self.latency_history);
        let buffer_avg = self.avg_metric(&self.buffer_history);

        let dropouts = self.dropouts.load(Ordering::Relaxed) as u32;

        PerformanceSnapshot {
            cpu_usage: cpu_avg,
            memory_usage: mem_avg,
            avg_callback_duration: callback_avg,
            max_callback_duration: callback_max,
            latency: latency_avg,
            buffer_level: buffer_avg,
            dropouts_per_min: dropouts,
            thread_count: std::thread::available_parallelism()
                .map(|c| c.get() as u32)
                .unwrap_or(4),
            thread_congestion: 0.0,  // Would track thread contention.
            disk_wait_ms: 0.0,       // Would track I/O wait.
            network_latency_ms: 0.0, // Would track network ping.
            timestamp: Instant::now(),
        }
    }

    /// Get average of a metric history.
    fn avg_metric(&self, history: &std::sync::Mutex<VecDeque<f64>>) -> f64 {
        let h = history.lock().unwrap();
        if h.is_empty() {
            return 0.0;
        }
        h.iter().sum::<f64>() / h.len() as f64
    }

    /// Check thresholds and generate alerts.
    fn check_thresholds(&self, metric: MetricType, value: f64) {
        let (level, threshold) = match metric {
            MetricType::CpuUsage => {
                if value >= self.thresholds.cpu_error {
                    (AlertLevel::Error, self.thresholds.cpu_error)
                } else if value >= self.thresholds.cpu_warning {
                    (AlertLevel::Warning, self.thresholds.cpu_warning)
                } else {
                    return;
                }
            }
            MetricType::MemoryUsage => {
                if value >= self.thresholds.memory_error {
                    (AlertLevel::Error, self.thresholds.memory_error)
                } else if value >= self.thresholds.memory_warning {
                    (AlertLevel::Warning, self.thresholds.memory_warning)
                } else {
                    return;
                }
            }
            MetricType::CallbackDuration => {
                if value >= self.thresholds.callback_error {
                    (AlertLevel::Error, self.thresholds.callback_error)
                } else if value >= self.thresholds.callback_warning {
                    (AlertLevel::Warning, self.thresholds.callback_warning)
                } else {
                    return;
                }
            }
            MetricType::Latency => {
                if value >= self.thresholds.latency_error {
                    (AlertLevel::Error, self.thresholds.latency_error)
                } else if value >= self.thresholds.latency_warning {
                    (AlertLevel::Warning, self.thresholds.latency_warning)
                } else {
                    return;
                }
            }
            MetricType::BufferLevel => {
                // Low buffer is the concern.
                if value <= self.thresholds.buffer_error {
                    (AlertLevel::Error, self.thresholds.buffer_error)
                } else if value <= self.thresholds.buffer_warning {
                    (AlertLevel::Warning, self.thresholds.buffer_warning)
                } else {
                    return;
                }
            }
            _ => return,
        };

        self.add_alert(
            level,
            format!(
                "{} exceeded: {:.1} (threshold: {:.1})",
                metric.display_name(),
                value,
                threshold
            ),
            metric,
            threshold,
            value,
        );
    }

    /// Add a performance alert.
    fn add_alert(
        &self,
        level: AlertLevel,
        message: String,
        metric: MetricType,
        threshold: f64,
        value: f64,
    ) {
        let alert = PerformanceAlert::new(level, message, metric, threshold, value);

        let mut alerts = self.alerts.lock().unwrap();
        // Keep only last 1000 alerts.
        if alerts.len() >= 1000 {
            alerts.remove(0);
        }
        alerts.push(alert);
        self.unacknowledged.fetch_add(1, Ordering::Relaxed);
    }

    /// Get recent alerts.
    pub fn get_alerts(&self, include_acknowledged: bool) -> Vec<PerformanceAlert> {
        let alerts = self.alerts.lock().unwrap();
        if include_acknowledged {
            alerts.clone()
        } else {
            alerts.iter().filter(|a| !a.acknowledged).cloned().collect()
        }
    }

    /// Acknowledge all unacknowledged alerts.
    pub fn acknowledge_all(&self) -> usize {
        let mut alerts = self.alerts.lock().unwrap();
        let count = alerts.iter().filter(|a| !a.acknowledged).count();
        for alert in alerts.iter_mut() {
            alert.acknowledged = true;
        }
        self.unacknowledged.store(0, Ordering::Relaxed);
        count
    }

    /// Get unacknowledged alert count.
    pub fn unacknowledged_count(&self) -> usize {
        self.unacknowledged.load(Ordering::Relaxed)
    }

    /// Get current buffer level.
    pub fn current_buffer_level(&self) -> f64 {
        self.avg_metric(&self.buffer_history)
    }

    /// Get current CPU usage.
    pub fn current_cpu(&self) -> f64 {
        self.avg_metric(&self.cpu_history)
    }

    /// Get current memory usage.
    pub fn current_memory(&self) -> f64 {
        self.avg_metric(&self.memory_history)
    }

    /// Get current callback duration.
    pub fn current_callback_duration(&self) -> f64 {
        self.avg_metric(&self.callback_history)
    }

    /// Get current latency.
    pub fn current_latency(&self) -> f64 {
        self.avg_metric(&self.latency_history)
    }

    /// Get peak callback duration.
    pub fn peak_callback_duration(&self) -> f64 {
        f64::from(self.peak_callback.load(Ordering::Relaxed))
    }

    /// Get total samples recorded.
    pub fn total_samples(&self) -> u64 {
        self.samples.load(Ordering::Relaxed)
    }

    /// Get total dropouts.
    pub fn total_dropouts(&self) -> usize {
        self.dropouts.load(Ordering::Relaxed)
    }

    /// Reset all metrics.
    pub fn reset(&self) {
        self.cpu_history.lock().unwrap().clear();
        self.memory_history.lock().unwrap().clear();
        self.callback_history.lock().unwrap().clear();
        self.latency_history.lock().unwrap().clear();
        self.buffer_history.lock().unwrap().clear();
        self.peak_callback.store(0.0, Ordering::Relaxed);
        self.dropouts.store(0, Ordering::Relaxed);
        self.samples.store(0, Ordering::Relaxed);
        self.alerts.lock().unwrap().clear();
        self.unacknowledged.store(0, Ordering::Relaxed);
    }

    /// Export metrics as CSV string.
    pub fn export_csv(&self) -> String {
        let mut csv = String::from("timestamp,metric,value");
        csv.push('\n');

        let now = Instant::now();
        let metrics = vec![
            (MetricType::CpuUsage, self.current_cpu()),
            (MetricType::MemoryUsage, self.current_memory()),
            (
                MetricType::CallbackDuration,
                self.current_callback_duration(),
            ),
            (MetricType::Latency, self.current_latency()),
            (MetricType::BufferLevel, self.current_buffer_level()),
        ];

        for (metric, value) in metrics {
            let elapsed = now.duration_since(Instant::now()).as_millis();
            csv.push_str(&format!(
                "{},{},{}\n",
                elapsed,
                metric.display_name(),
                value
            ));
        }

        csv
    }

    /// Check if performance is within acceptable bounds.
    pub fn is_acceptable(&self) -> bool {
        let snap = self.snapshot();
        snap.cpu_usage < self.thresholds.cpu_error
            && snap.memory_usage < self.thresholds.memory_error
            && snap.avg_callback_duration < self.thresholds.callback_error
            && snap.latency < self.thresholds.latency_error
            && snap.buffer_level > self.thresholds.buffer_error
    }

    /// Get health status.
    pub fn health_status(&self) -> (AlertLevel, String) {
        let snap = self.snapshot();

        if snap.cpu_usage >= self.thresholds.cpu_error
            || snap.memory_usage >= self.thresholds.memory_error
            || snap.avg_callback_duration >= self.thresholds.callback_error * 2.0
        {
            (
                AlertLevel::Critical,
                "Critical performance degradation".to_string(),
            )
        } else if snap.cpu_usage >= self.thresholds.cpu_warning
            || snap.memory_usage >= self.thresholds.memory_warning
            || snap.avg_callback_duration >= self.thresholds.callback_warning
        {
            (AlertLevel::Warning, "Performance degraded".to_string())
        } else if self.unacknowledged.load(Ordering::Relaxed) > 0 {
            (
                AlertLevel::Info,
                format!("{} unacknowledged alerts", self.unacknowledged_count()),
            )
        } else {
            (
                AlertLevel::Info,
                "All metrics within normal range".to_string(),
            )
        }
    }

    /// Update threshold configuration.
    pub fn update_thresholds(&mut self, config: ThresholdConfig) {
        self.thresholds = config;
    }

    /// Get current thresholds.
    pub fn thresholds(&self) -> &ThresholdConfig {
        &self.thresholds
    }
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new(ThresholdConfig::default())
    }
}

/// Performance monitoring configuration for audio callback.
pub struct AudioCallbackMonitor {
    /// Time budget per callback (μs).
    time_budget_us: u64,
    /// Deadline check interval.
    deadline_interval: Duration,
    /// Last deadline check.
    last_check: std::sync::Mutex<Instant>,
    /// Missed deadlines.
    missed_deadlines: AtomicU64,
    /// Total callbacks.
    total_callbacks: AtomicU64,
}

impl AudioCallbackMonitor {
    /// Create a new audio callback monitor.
    pub fn new(time_budget_us: u64) -> Self {
        Self {
            time_budget_us,
            deadline_interval: Duration::from_millis(100),
            last_check: std::sync::Mutex::new(Instant::now()),
            missed_deadlines: AtomicU64::new(0),
            total_callbacks: AtomicU64::new(0),
        }
    }

    /// Enter callback — records start time.
    pub fn enter(&self) {
        self.total_callbacks.fetch_add(1, Ordering::Relaxed);
    }

    /// Exit callback — checks deadline.
    pub fn exit(&self, actual_duration_us: u64) {
        if actual_duration_us > self.time_budget_us {
            self.missed_deadlines.fetch_add(1, Ordering::Relaxed);
        }

        let mut last = self.last_check.lock().unwrap();
        if last.elapsed() > self.deadline_interval {
            *last = Instant::now();
        }
    }

    /// Get deadline miss rate.
    pub fn miss_rate(&self) -> f64 {
        let total = self.total_callbacks.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let missed = self.missed_deadlines.load(Ordering::Relaxed);
        missed as f64 / total as f64
    }

    /// Get total callbacks.
    pub fn total_callbacks(&self) -> u64 {
        self.total_callbacks.load(Ordering::Relaxed)
    }

    /// Get missed deadlines.
    pub fn missed_deadlines(&self) -> u64 {
        self.missed_deadlines.load(Ordering::Relaxed)
    }

    /// Get time budget in microseconds.
    pub fn time_budget(&self) -> u64 {
        self.time_budget_us
    }
}

impl Default for AudioCallbackMonitor {
    fn default() -> Self {
        Self::new(1000) // 1ms budget per callback.
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_type_display() {
        assert_eq!(MetricType::CpuUsage.display_name(), "CPU Usage");
        assert_eq!(MetricType::MemoryUsage.display_name(), "Memory Usage");
        assert_eq!(
            MetricType::CallbackDuration.display_name(),
            "Callback Duration"
        );
        assert_eq!(MetricType::Latency.display_name(), "Latency");
        assert_eq!(MetricType::BufferLevel.display_name(), "Buffer Level");
        assert_eq!(MetricType::Dropouts.display_name(), "Dropouts");
    }

    #[test]
    fn metric_type_unit() {
        assert_eq!(MetricType::CpuUsage.unit(), "%");
        assert_eq!(MetricType::MemoryUsage.unit(), "MB");
        assert_eq!(MetricType::CallbackDuration.unit(), "μs");
        assert_eq!(MetricType::Latency.unit(), "ms");
        assert_eq!(MetricType::BufferLevel.unit(), "%");
    }

    #[test]
    fn threshold_config_defaults() {
        let config = ThresholdConfig::default();
        assert_eq!(config.cpu_warning, 70.0);
        assert_eq!(config.cpu_error, 90.0);
        assert_eq!(config.memory_warning, 512.0);
        assert_eq!(config.memory_error, 1024.0);
        assert_eq!(config.callback_warning, 500.0);
        assert_eq!(config.callback_error, 1000.0);
        assert_eq!(config.latency_warning, 10.0);
        assert_eq!(config.latency_error, 20.0);
        assert_eq!(config.buffer_warning, 20.0);
        assert_eq!(config.buffer_error, 10.0);
    }

    #[test]
    fn alert_level_color() {
        assert_eq!(AlertLevel::Info.color(), (0, 120, 255));
        assert_eq!(AlertLevel::Warning.color(), (255, 165, 0));
        assert_eq!(AlertLevel::Error.color(), (255, 0, 0));
        assert_eq!(AlertLevel::Critical.color(), (255, 0, 255));
    }

    #[test]
    fn performance_monitor_start_stop() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        assert!(!monitor.is_monitoring());

        monitor.start();
        assert!(monitor.is_monitoring());

        monitor.stop();
        assert!(!monitor.is_monitoring());
    }

    #[test]
    fn performance_monitor_record_cpu() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_cpu(50.0);
        monitor.record_cpu(60.0);
        monitor.record_cpu(70.0);

        assert!((monitor.current_cpu() - 60.0).abs() < 0.1);
    }

    #[test]
    fn performance_monitor_record_memory() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_memory(256.0);
        monitor.record_memory(512.0);

        assert!((monitor.current_memory() - 384.0).abs() < 0.1);
    }

    #[test]
    fn performance_monitor_record_callback() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_callback_duration(300);
        monitor.record_callback_duration(400);
        monitor.record_callback_duration(500);

        assert!((monitor.current_callback_duration() - 400.0).abs() < 0.1);
        assert!(monitor.peak_callback_duration() >= 500.0);
    }

    #[test]
    fn performance_monitor_record_latency() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_latency(5.0);
        monitor.record_latency(8.0);

        assert!((monitor.current_latency() - 6.5).abs() < 0.1);
    }

    #[test]
    fn performance_monitor_record_buffer() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_buffer_level(50.0);
        monitor.record_buffer_level(60.0);

        assert!((monitor.current_buffer_level() - 55.0).abs() < 0.1);
    }

    #[test]
    fn performance_monitor_snapshot() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_cpu(50.0);
        monitor.record_memory(256.0);
        monitor.record_callback_duration(300);
        monitor.record_latency(5.0);
        monitor.record_buffer_level(50.0);

        let snap = monitor.snapshot();
        assert!((snap.cpu_usage - 50.0).abs() < 0.1);
        assert!((snap.memory_usage - 256.0).abs() < 0.1);
        assert!((snap.avg_callback_duration - 300.0).abs() < 0.1);
        assert!((snap.latency - 5.0).abs() < 0.1);
        assert!((snap.buffer_level - 50.0).abs() < 0.1);
    }

    #[test]
    fn performance_monitor_threshold_warning() {
        let config = ThresholdConfig {
            cpu_warning: 50.0,
            cpu_error: 80.0,
            ..ThresholdConfig::default()
        };
        let monitor = PerformanceMonitor::new(config);
        monitor.start();

        // Below warning.
        monitor.record_cpu(40.0);
        assert_eq!(monitor.get_alerts(false).len(), 0);

        // Above warning, below error.
        monitor.record_cpu(60.0);
        let alerts = monitor.get_alerts(false);
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].level, AlertLevel::Warning);
    }

    #[test]
    fn performance_monitor_threshold_error() {
        let config = ThresholdConfig {
            cpu_warning: 50.0,
            cpu_error: 60.0,
            ..ThresholdConfig::default()
        };
        let monitor = PerformanceMonitor::new(config);
        monitor.start();

        monitor.record_cpu(70.0);
        let alerts = monitor.get_alerts(false);
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].level, AlertLevel::Error);
        assert_eq!(alerts[0].metric, MetricType::CpuUsage);
    }

    #[test]
    fn performance_monitor_alert_acknowledgment() {
        let config = ThresholdConfig {
            cpu_error: 40.0,
            ..ThresholdConfig::default()
        };
        let monitor = PerformanceMonitor::new(config);
        monitor.start();

        monitor.record_cpu(50.0);
        assert_eq!(monitor.unacknowledged_count(), 1);

        let count = monitor.acknowledge_all();
        assert!(count > 0);
        assert_eq!(monitor.unacknowledged_count(), 0);
    }

    #[test]
    fn performance_monitor_reset() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_cpu(50.0);
        monitor.record_memory(256.0);
        monitor.record_callback_duration(300);

        monitor.reset();

        assert_eq!(monitor.total_samples(), 0);
        assert_eq!(monitor.total_dropouts(), 0);
        assert_eq!(monitor.alerts.lock().unwrap().len(), 0);
    }

    #[test]
    fn performance_monitor_export_csv() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_cpu(50.0);
        monitor.record_memory(256.0);
        monitor.record_callback_duration(300);

        let csv = monitor.export_csv();
        assert!(csv.contains("timestamp,metric,value"));
        assert!(csv.contains("CPU Usage"));
        assert!(csv.contains("Memory Usage"));
        assert!(csv.contains("Callback Duration"));
    }

    #[test]
    fn performance_monitor_is_acceptable() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_cpu(50.0);
        monitor.record_memory(256.0);
        monitor.record_callback_duration(300);
        monitor.record_latency(5.0);
        monitor.record_buffer_level(50.0);

        assert!(monitor.is_acceptable());
    }

    #[test]
    fn performance_monitor_health_status_normal() {
        let monitor = PerformanceMonitor::new(ThresholdConfig::default());
        monitor.start();

        monitor.record_cpu(40.0);
        monitor.record_memory(256.0);
        monitor.record_callback_duration(200);

        let (level, msg) = monitor.health_status();
        assert_eq!(level, AlertLevel::Info);
        assert!(msg.contains("normal") || msg.contains("alert"));
    }

    #[test]
    fn performance_monitor_health_status_critical() {
        let config = ThresholdConfig {
            cpu_error: 50.0,
            callback_error: 300.0,
            ..ThresholdConfig::default()
        };
        let monitor = PerformanceMonitor::new(config);
        monitor.start();

        monitor.record_cpu(60.0);
        monitor.record_callback_duration(800);

        let (level, _) = monitor.health_status();
        assert_eq!(level, AlertLevel::Critical);
    }

    #[test]
    fn audio_callback_monitor() {
        let monitor = AudioCallbackMonitor::new(1000); // 1ms budget.

        assert_eq!(monitor.time_budget(), 1000);
        assert_eq!(monitor.total_callbacks(), 0);
        assert_eq!(monitor.missed_deadlines(), 0);

        // Callback within budget.
        monitor.enter();
        monitor.exit(500);

        assert_eq!(monitor.total_callbacks(), 1);
        assert_eq!(monitor.missed_deadlines(), 0);
        assert_eq!(monitor.miss_rate(), 0.0);

        // Callback exceeding budget.
        monitor.enter();
        monitor.exit(1500);

        assert_eq!(monitor.total_callbacks(), 2);
        assert_eq!(monitor.missed_deadlines(), 1);
        assert_eq!(monitor.miss_rate(), 0.5);
    }

    #[test]
    fn performance_monitor_dropout() {
        let config = ThresholdConfig {
            callback_error: 500.0,
            ..ThresholdConfig::default()
        };
        let monitor = PerformanceMonitor::new(config);
        monitor.start();

        monitor.record_callback_duration(1200); // More than 2x error threshold.
        assert_eq!(monitor.total_dropouts(), 1);
    }

    #[test]
    fn performance_monitor_buffer_low_warning() {
        let config = ThresholdConfig {
            buffer_warning: 30.0,
            buffer_error: 15.0,
            ..ThresholdConfig::default()
        };
        let monitor = PerformanceMonitor::new(config);
        monitor.start();

        monitor.record_buffer_level(10.0);
        let alerts = monitor.get_alerts(false);
        assert!(!alerts.is_empty());
        assert_eq!(alerts[0].metric, MetricType::BufferLevel);
    }

    #[test]
    fn performance_monitor_update_thresholds() {
        let mut monitor = PerformanceMonitor::new(ThresholdConfig::default());

        let new_config = ThresholdConfig {
            cpu_warning: 80.0,
            cpu_error: 95.0,
            ..ThresholdConfig::default()
        };
        monitor.update_thresholds(new_config);

        assert_eq!(monitor.thresholds().cpu_warning, 80.0);
        assert_eq!(monitor.thresholds().cpu_error, 95.0);
    }

    #[test]
    fn performance_metrics() {
        let sample = MetricSample {
            metric: MetricType::CpuUsage,
            value: 45.0,
            timestamp: Instant::now(),
        };
        assert_eq!(sample.metric, MetricType::CpuUsage);
        assert!((sample.value - 45.0).abs() < 0.01);
    }

    #[test]
    fn alert_creation() {
        let alert = PerformanceAlert::new(
            AlertLevel::Warning,
            "CPU high".to_string(),
            MetricType::CpuUsage,
            70.0,
            75.0,
        );
        assert_eq!(alert.level, AlertLevel::Warning);
        assert_eq!(alert.message, "CPU high");
        assert_eq!(alert.threshold, 70.0);
        assert_eq!(alert.value, 75.0);
        assert!(!alert.acknowledged);
    }
}
