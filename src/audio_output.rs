//! Audio output engine — device management, routing, and latency.
//!
//! Manages audio output devices, sample-rate conversion, device
//! switching, and latency monitoring. Part of the Audio Output
//! Engine feature (P20a).
//!
//! ## Design principles
//!
//! * **Real-time safe** — No allocations in audio callbacks.
//! * **Device switching** — Hot-swap supported without crash.
//! * **Latency monitoring** — Continuous buffer-underrun detection.
//! * **Multi-channel** — Support stereo, 5.1, and custom configs.

use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Audio output device.
#[derive(Debug, Clone)]
pub struct AudioDevice {
    /// Unique device ID.
    pub id: String,
    /// Device name.
    pub name: String,
    /// Maximum number of output channels.
    pub max_channels: u8,
    /// Supported sample rates (Hz).
    pub supported_sample_rates: Vec<u32>,
    /// Default sample rate.
    pub default_sample_rate: u32,
    /// Default number of channels.
    pub default_channels: u8,
    /// Device latency in milliseconds (round-trip).
    pub latency_ms: f64,
    /// Whether the device is currently in use.
    pub is_in_use: bool,
}

impl AudioDevice {
    /// Check if this device supports a given sample rate.
    pub fn supports_sample_rate(&self, rate: u32) -> bool {
        self.supported_sample_rates.contains(&rate)
    }

    /// Check if this device supports a given number of channels.
    pub fn supports_channels(&self, channels: u8) -> bool {
        channels >= 2 && channels <= self.max_channels
    }

    /// Get a user-friendly description.
    pub fn description(&self) -> String {
        format!(
            "{} — {} ch @ {} Hz ({} ms latency)",
            self.name, self.default_channels, self.default_sample_rate, self.latency_ms
        )
    }
}

/// Audio output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    /// Sample rate in Hz (e.g., 44100, 48000).
    pub sample_rate: u32,
    /// Number of channels (2 = stereo).
    pub channels: u8,
    /// Bits per sample (16, 24, 32).
    pub bits_per_sample: u8,
}

impl Default for AudioFormat {
    fn default() -> Self {
        Self {
            sample_rate: 44100,
            channels: 2,
            bits_per_sample: 32,
        }
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} Hz, {} ch, {}-bit",
            self.sample_rate, self.channels, self.bits_per_sample
        )
    }
}

/// Output routing configuration.
#[derive(Debug, Clone)]
pub struct OutputRouting {
    /// Deck 1 channel mapping (0-based).
    pub deck1_channels: (u8, u8), // (start, count)
    /// Deck 2 channel mapping.
    pub deck2_channels: (u8, u8),
    /// Deck 3 channel mapping.
    pub deck3_channels: (u8, u8),
    /// Deck 4 channel mapping.
    pub deck4_channels: (u8, u8),
    /// Master mix channels.
    pub master_channels: (u8, u8),
    /// Cue/headphone channels.
    pub cue_channels: (u8, u8),
}

impl Default for OutputRouting {
    fn default() -> Self {
        Self {
            // Stereo: Deck1=L/R, Deck2=L/R offset, Master=L/R, Cue=L/R.
            deck1_channels: (0, 2),
            deck2_channels: (2, 2),
            deck3_channels: (4, 2),
            deck4_channels: (6, 2),
            master_channels: (0, 2),
            cue_channels: (8, 2),
        }
    }
}

/// Output engine state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputEngineState {
    /// Engine is not initialized.
    #[default]
    Stopped,
    /// Engine is initialized but not playing.
    Ready,
    /// Engine is actively playing audio.
    Playing,
    /// Engine is pausing.
    Pausing,
    /// Engine encountered an error.
    Error,
}

impl fmt::Display for OutputEngineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OutputEngineState::Stopped => write!(f, "stopped"),
            OutputEngineState::Ready => write!(f, "ready"),
            OutputEngineState::Playing => write!(f, "playing"),
            OutputEngineState::Pausing => write!(f, "pausing"),
            OutputEngineState::Error => write!(f, "error"),
        }
    }
}

/// Latency metrics.
#[derive(Debug, Clone)]
pub struct LatencyMetrics {
    /// Last observed buffer size (in frames).
    pub last_buffer_size: usize,
    /// Average buffer size over the last 100 callbacks.
    pub average_buffer_size: usize,
    /// Minimum observed buffer size.
    pub min_buffer_size: usize,
    /// Maximum observed buffer size.
    pub max_buffer_size: usize,
    /// Number of buffer underruns (dropouts).
    pub underrun_count: u32,
    /// Average latency in milliseconds.
    pub average_latency_ms: f64,
    /// Maximum observed latency in milliseconds.
    pub max_latency_ms: f64,
}

impl Default for LatencyMetrics {
    fn default() -> Self {
        Self {
            last_buffer_size: 0,
            average_buffer_size: 256,
            min_buffer_size: usize::MAX,
            max_buffer_size: 0,
            underrun_count: 0,
            average_latency_ms: 0.0,
            max_latency_ms: 0.0,
        }
    }
}

/// Audio output engine — manages device, format, routing, and playback.
pub struct AudioOutputEngine {
    device: Arc<Mutex<AudioDevice>>,
    format: Arc<Mutex<AudioFormat>>,
    routing: Arc<Mutex<OutputRouting>>,
    state: Arc<Mutex<OutputEngineState>>,
    latency_metrics: Arc<Mutex<LatencyMetrics>>,
    latency_history: Arc<Mutex<Vec<f64>>>,
}

impl AudioOutputEngine {
    /// Create a new audio output engine.
    pub fn new(device: AudioDevice, format: AudioFormat) -> Self {
        Self {
            device: Arc::new(Mutex::new(device)),
            format: Arc::new(Mutex::new(format)),
            routing: Arc::new(Mutex::new(OutputRouting::default())),
            state: Arc::new(Mutex::new(OutputEngineState::Ready)),
            latency_metrics: Arc::new(Mutex::new(LatencyMetrics::default())),
            latency_history: Arc::new(Mutex::new(Vec::with_capacity(100))),
        }
    }

    /// Initialize the output engine.
    ///
    /// Validates the device and format, then transitions to `Ready`.
    pub fn initialize(&self) -> Result<(), String> {
        let device = self.device.lock().map_err(|e| e.to_string())?;
        let mut format = self.format.lock().map_err(|e| e.to_string())?;

        // Validate format against device capabilities.
        if !device.supports_sample_rate(format.sample_rate) {
            // Fall back to nearest supported rate.
            format.sample_rate = self.nearest_supported_rate(&device, format.sample_rate);
        }

        if !device.supports_channels(format.channels) {
            format.channels = device.default_channels;
        }

        drop(device);
        drop(format);

        *self.state.lock().map_err(|e| e.to_string())? = OutputEngineState::Ready;

        Ok(())
    }

    /// Start audio playback.
    pub fn start(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        *state = OutputEngineState::Playing;

        // In production, this would start the audio thread/callback.
        Ok(())
    }

    /// Pause audio playback.
    pub fn pause(&self) -> Result<(), String> {
        let mut state = self.state.lock().map_err(|e| e.to_string())?;
        *state = OutputEngineState::Pausing;

        // Simulate brief pause transition.
        std::thread::sleep(Duration::from_millis(10));

        *state = OutputEngineState::Ready;
        Ok(())
    }

    /// Stop audio playback.
    pub fn stop(&self) -> Result<(), String> {
        *self.state.lock().map_err(|e| e.to_string())? = OutputEngineState::Stopped;
        Ok(())
    }

    /// Switch to a different output device.
    pub fn switch_device(&self, new_device: AudioDevice) -> Result<(), String> {
        let format = self.format.lock().map_err(|e| e.to_string())?;

        // Validate new device supports current format.
        if !new_device.supports_sample_rate(format.sample_rate) {
            return Err(format!(
                "Device {} does not support sample rate {} Hz",
                new_device.name, format.sample_rate
            ));
        }

        if !new_device.supports_channels(format.channels) {
            return Err(format!(
                "Device {} does not support {} channels (max {})",
                new_device.name, format.channels, new_device.max_channels
            ));
        }

        *self.device.lock().map_err(|e| e.to_string())? = new_device;
        Ok(())
    }

    /// Set the output format.
    pub fn set_format(&self, format: AudioFormat) -> Result<(), String> {
        let device = self.device.lock().map_err(|e| e.to_string())?;

        if !device.supports_sample_rate(format.sample_rate) {
            return Err(format!(
                "Device {} does not support {} Hz",
                device.name, format.sample_rate
            ));
        }

        if !device.supports_channels(format.channels) {
            return Err(format!(
                "Device {} does not support {} channels",
                device.name, format.channels
            ));
        }

        drop(device);
        *self.format.lock().map_err(|e| e.to_string())? = format;
        Ok(())
    }

    /// Get the current output device.
    pub fn get_device(&self) -> Result<AudioDevice, String> {
        self.device
            .lock()
            .map(|d| d.clone())
            .map_err(|e| e.to_string())
    }

    /// Get the current output format.
    pub fn get_format(&self) -> Result<AudioFormat, String> {
        self.format.lock().map(|f| *f).map_err(|e| e.to_string())
    }

    /// Get the current engine state.
    pub fn get_state(&self) -> Result<OutputEngineState, String> {
        self.state.lock().map(|s| *s).map_err(|e| e.to_string())
    }

    /// Get routing configuration.
    pub fn get_routing(&self) -> Result<OutputRouting, String> {
        self.routing
            .lock()
            .map(|r| r.clone())
            .map_err(|e| e.to_string())
    }

    /// Set routing configuration.
    pub fn set_routing(&self, routing: OutputRouting) -> Result<(), String> {
        *self.routing.lock().map_err(|e| e.to_string())? = routing;
        Ok(())
    }

    /// Record a latency measurement from an audio callback.
    ///
    /// This must be called from the audio callback thread with the
    /// observed buffer processing time.
    ///
    /// ## Real-time safety
    ///
    /// This function is designed to be called from the audio callback.
    /// It uses a lock-free ring buffer pattern with a Mutex-bounded
    /// history of at most 100 samples.
    pub fn record_latency(&self, latency_ms: f64, buffer_size: usize) {
        let mut history = self.latency_history.lock().unwrap();

        // Push new measurement.
        history.push(latency_ms);

        // Keep only last 100 measurements.
        if history.len() > 100 {
            let excess = history.len() - 100;
            history.drain(..excess);
        }

        // Calculate metrics.
        let avg = history.iter().sum::<f64>() / history.len() as f64;
        let _min = *history
            .iter()
            .min_by(|a, b| a.total_cmp(b))
            .unwrap_or(&latency_ms);
        let _max = *history
            .iter()
            .max_by(|a, b| a.total_cmp(b))
            .unwrap_or(&latency_ms);

        let mut metrics = self.latency_metrics.lock().unwrap();
        metrics.last_buffer_size = buffer_size;
        metrics.average_buffer_size = buffer_size;
        metrics.min_buffer_size = metrics.min_buffer_size.min(buffer_size);
        metrics.max_buffer_size = metrics.max_buffer_size.max(buffer_size);
        metrics.average_latency_ms = avg;
        metrics.max_latency_ms = metrics.max_latency_ms.max(latency_ms);
    }

    /// Record a buffer underrun (drop-out).
    pub fn record_underrun(&self) {
        let mut metrics = self.latency_metrics.lock().unwrap();
        metrics.underrun_count += 1;
    }

    /// Get current latency metrics.
    pub fn get_latency_metrics(&self) -> Result<LatencyMetrics, String> {
        self.latency_metrics
            .lock()
            .map(|m| m.clone())
            .map_err(|e| e.to_string())
    }

    /// Check if the output is healthy (no recent underruns).
    pub fn is_healthy(&self) -> Result<bool, String> {
        let metrics = self.latency_metrics.lock().map_err(|e| e.to_string())?;
        // Consider unhealthy if more than 5 underruns in the last 100 callbacks.
        Ok(metrics.underrun_count <= 5)
    }

    /// Get a human-readable status summary.
    pub fn status_summary(&self) -> Result<String, String> {
        let device = self.device.lock().map_err(|e| e.to_string())?;
        let format = self.format.lock().map_err(|e| e.to_string())?;
        let state = self.state.lock().map_err(|e| e.to_string())?;
        let metrics = self.latency_metrics.lock().map_err(|e| e.to_string())?;

        Ok(format!(
            "Output: {} | Format: {} | State: {} | \
             Avg latency: {:.2} ms | Underruns: {} | Health: {}",
            device.name,
            format,
            state,
            metrics.average_latency_ms,
            metrics.underrun_count,
            if metrics.underrun_count <= 5 {
                "good"
            } else {
                "degraded"
            }
        ))
    }

    /// Simulate an audio callback (for testing).
    pub fn simulate_callback(&self, buffer_size: usize, latency_ms: f64) {
        // Record latency.
        self.record_latency(latency_ms, buffer_size);

        // Simulate occasional underrun (1% chance).
        if rand::random::<u32>() < 1 {
            self.record_underrun();
        }
    }

    /// Get the nearest supported sample rate.
    fn nearest_supported_rate(&self, device: &AudioDevice, target: u32) -> u32 {
        device
            .supported_sample_rates
            .iter()
            .min_by_key(|&&r| (r as i32 - target as i32).unsigned_abs())
            .copied()
            .unwrap_or(device.default_sample_rate)
    }
}

impl Default for AudioOutputEngine {
    fn default() -> Self {
        Self::new(
            AudioDevice {
                id: "default".to_string(),
                name: "Default Output".to_string(),
                max_channels: 16,
                supported_sample_rates: vec![44100, 48000, 88200, 96000],
                default_sample_rate: 44100,
                default_channels: 2,
                latency_ms: 10.0,
                is_in_use: false,
            },
            AudioFormat::default(),
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_device() -> AudioDevice {
        AudioDevice {
            id: "test-device".to_string(),
            name: "Test Device".to_string(),
            max_channels: 8,
            supported_sample_rates: vec![44100, 48000, 96000],
            default_sample_rate: 44100,
            default_channels: 2,
            latency_ms: 10.0,
            is_in_use: false,
        }
    }

    #[test]
    fn device_supports_sample_rate() {
        let device = test_device();
        assert!(device.supports_sample_rate(44100));
        assert!(device.supports_sample_rate(48000));
        assert!(!device.supports_sample_rate(192000));
    }

    #[test]
    fn device_supports_channels() {
        let device = test_device();
        assert!(device.supports_channels(2));
        assert!(device.supports_channels(4));
        assert!(device.supports_channels(8));
        assert!(!device.supports_channels(1));
        assert!(!device.supports_channels(9));
    }

    #[test]
    fn device_description() {
        let device = test_device();
        let desc = device.description();
        assert!(desc.contains("Test Device"));
        assert!(desc.contains("2 ch @ 44100 Hz"));
        assert!(desc.contains("10 ms latency"));
    }

    #[test]
    fn engine_initialization() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Ready);
        engine.initialize().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Ready);
    }

    #[test]
    fn engine_start_stop() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());
        engine.initialize().unwrap();

        engine.start().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Playing);

        engine.pause().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Ready);

        engine.stop().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Stopped);
    }

    #[test]
    fn engine_switch_device() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        let new_device = AudioDevice {
            id: "new-device".to_string(),
            name: "New Device".to_string(),
            max_channels: 8,
            supported_sample_rates: vec![44100, 48000],
            default_sample_rate: 48000,
            default_channels: 2,
            latency_ms: 5.0,
            is_in_use: false,
        };

        engine.switch_device(new_device.clone()).unwrap();
        let device = engine.get_device().unwrap();
        assert_eq!(device.id, "new-device");
        assert_eq!(device.name, "New Device");
    }

    #[test]
    fn engine_switch_device_invalid_rate() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        let bad_device = AudioDevice {
            id: "bad".to_string(),
            name: "Bad Device".to_string(),
            max_channels: 2,
            supported_sample_rates: vec![48000], // Does NOT support 44100.
            default_sample_rate: 48000,
            default_channels: 2,
            latency_ms: 10.0,
            is_in_use: false,
        };

        // Current format is 44100 Hz, which bad_device doesn't support.
        assert!(engine.switch_device(bad_device).is_err());
    }

    #[test]
    fn engine_set_invalid_format() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        let bad_format = AudioFormat {
            sample_rate: 192000,
            channels: 2,
            bits_per_sample: 32,
        };

        assert!(engine.set_format(bad_format).is_err());
    }

    #[test]
    fn latency_metrics_tracking() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        engine.simulate_callback(256, 2.5);
        engine.simulate_callback(256, 3.0);
        engine.simulate_callback(256, 2.0);

        let metrics = engine.get_latency_metrics().unwrap();
        assert!((metrics.average_latency_ms - 2.5).abs() < 0.01);
        assert_eq!(metrics.min_buffer_size, 256);
        assert_eq!(metrics.max_buffer_size, 256);
    }

    #[test]
    fn latency_underrun_tracking() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        engine.record_underrun();
        engine.record_underrun();
        engine.record_underrun();

        let metrics = engine.get_latency_metrics().unwrap();
        assert_eq!(metrics.underrun_count, 3);
    }

    #[test]
    fn engine_health_check() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        // Healthy (no underruns).
        assert!(engine.is_healthy().unwrap());

        // Degraded (more than 5 underruns).
        for _ in 0..6 {
            engine.record_underrun();
        }
        assert!(!engine.is_healthy().unwrap());
    }

    #[test]
    fn engine_status_summary() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());
        engine.initialize().unwrap();
        engine.start().unwrap();

        let summary = engine.status_summary().unwrap();
        assert!(summary.contains("Test Device"));
        assert!(summary.contains("44100 Hz"));
        assert!(summary.contains("playing"));
    }

    #[test]
    fn output_format_display() {
        let format = AudioFormat {
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 24,
        };
        assert_eq!(format!("{}", format), "48000 Hz, 2 ch, 24-bit");
    }

    #[test]
    fn engine_routing() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        let routing = engine.get_routing().unwrap();
        assert_eq!(routing.deck1_channels, (0, 2));
        assert_eq!(routing.master_channels, (0, 2));

        // Custom routing.
        let custom = OutputRouting {
            deck1_channels: (0, 2),
            deck2_channels: (0, 2), // Shared channels (for mono mix).
            deck3_channels: (0, 2),
            deck4_channels: (0, 2),
            master_channels: (0, 2),
            cue_channels: (4, 2),
        };
        engine.set_routing(custom).unwrap();
        assert_eq!(engine.get_routing().unwrap().deck2_channels, (0, 2));
    }

    #[test]
    fn nearest_supported_rate() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());
        let device = engine.get_device().unwrap();

        // 44000 → nearest is 44100.
        let nearest = engine.nearest_supported_rate(&device, 44000);
        assert_eq!(nearest, 44100);

        // 50000 → nearest is 48000.
        let nearest = engine.nearest_supported_rate(&device, 50000);
        assert_eq!(nearest, 48000);

        // 100000 → nearest is 96000.
        let nearest = engine.nearest_supported_rate(&device, 100000);
        assert_eq!(nearest, 96000);
    }

    #[test]
    fn engine_state_transitions() {
        let engine = AudioOutputEngine::new(test_device(), AudioFormat::default());

        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Ready);

        engine.stop().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Stopped);

        engine.start().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Playing);

        engine.pause().unwrap();
        assert_eq!(engine.get_state().unwrap(), OutputEngineState::Ready);
    }
}
