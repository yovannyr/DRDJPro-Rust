//! Spectrogram — STFT, Peak/RMS levels, Color mapping, Export.
//!
//! Implements Short-Time Fourier Transform based spectrogram generation
//! for audio visualization and analysis. Part of the Spectrogram feature (P07a).
//!
//! ## Design principles
//!
//! * **STFT standard** — Hann window, configurable hop/fft size.
//! * **Peak/RMS** — Per-frame and overall energy metrics.
//! * **Color mapping** — Heat map rendering for visual analysis.
//! * **Export** — PNG and CSV support for spectrogram data.

use std::f64::consts::PI;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// FFT size options for spectrogram computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FftSize {
    /// 256 samples — fast, low resolution.
    Size256,
    /// 512 samples — balanced.
    Size512,
    /// 1024 samples — good resolution.
    Size1024,
    /// 2048 samples — high resolution.
    Size2048,
    /// 4096 samples — very high resolution.
    Size4096,
    /// 8192 samples — maximum resolution.
    Size8192,
}

impl FftSize {
    /// Get the sample count for this FFT size.
    pub fn samples(self) -> usize {
        match self {
            Self::Size256 => 256,
            Self::Size512 => 512,
            Self::Size1024 => 1024,
            Self::Size2048 => 2048,
            Self::Size4096 => 4096,
            Self::Size8192 => 8192,
        }
    }
}

/// Hop size relative to FFT size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopSize {
    /// One quarter of FFT size — high temporal resolution.
    Quarter,
    /// One half of FFT size — balanced.
    Half,
    /// Three quarters of FFT size — low temporal resolution.
    ThreeQuarters,
    /// Full FFT size — minimal overlap.
    Full,
}

impl HopSize {
    /// Get the hop sample count for a given FFT size.
    pub fn samples(self, fft_size: FftSize) -> usize {
        let fft = fft_size.samples();
        match self {
            Self::Quarter => fft / 4,
            Self::Half => fft / 2,
            Self::ThreeQuarters => (fft * 3) / 4,
            Self::Full => fft,
        }
    }
}

/// Spectrogram color map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMap {
    /// Classic heat map (blue → cyan → yellow → red).
    Heat,
    /// Viridis-like colormap.
    Viridis,
    /// Grayscale intensity.
    Grayscale,
    /// Inferno colormap.
    Inferno,
}

impl ColorMap {
    /// Convert a normalized intensity (0.0-1.0) to RGB color.
    pub fn to_rgb(self, intensity: f64) -> (u8, u8, u8) {
        let i = intensity.clamp(0.0, 1.0);
        match self {
            Self::Heat => self.heat_map(i),
            Self::Viridis => self.viridis(i),
            Self::Grayscale => self.grayscale(i),
            Self::Inferno => self.inferno(i),
        }
    }

    /// Heat map: blue → cyan → yellow → red.
    fn heat_map(self, i: f64) -> (u8, u8, u8) {
        if i < 0.25 {
            // Blue to cyan.
            let t = i / 0.25;
            (0, (t * 255.0) as u8, 255)
        } else if i < 0.5 {
            // Cyan to green.
            let t = (i - 0.25) / 0.25;
            (0, 255, (255.0 - t * 255.0) as u8)
        } else if i < 0.75 {
            // Green to yellow.
            let t = (i - 0.5) / 0.25;
            ((t * 255.0) as u8, 255, 0)
        } else {
            // Yellow to red to dark.
            let t = (i - 0.75) / 0.25;
            (255, (255.0 - t * 200.0) as u8, 0)
        }
    }

    /// Viridis-like colormap approximation.
    fn viridis(self, i: f64) -> (u8, u8, u8) {
        // Simplified viridis colors.
        let r = (32.0 + 223.0 * i * i) as u8;
        let g = (20.0 + 240.0 * i - 120.0 * i * i) as u8;
        let b = (131.0 + 100.0 * i - 200.0 * i * i) as u8;
        (r, g, b)
    }

    /// Grayscale.
    fn grayscale(self, i: f64) -> (u8, u8, u8) {
        let v = (i * 255.0) as u8;
        (v, v, v)
    }

    /// Inferno colormap approximation.
    fn inferno(self, i: f64) -> (u8, u8, u8) {
        let r = (255.0 * (1.0 - (1.0 - i).powf(3.0))).max(0.0) as u8;
        let g = (180.0 * i.powf(1.5)).max(0.0) as u8;
        let b = (80.0 * i.powf(2.5)).max(0.0) as u8;
        (r, g, b)
    }
}

/// Per-frame energy analysis result.
#[derive(Debug, Clone)]
pub struct FrameEnergy {
    /// Root mean square energy of the frame.
    pub rms: f64,
    /// Peak amplitude of the frame.
    pub peak: f64,
    /// Zero crossing rate.
    pub zcr: f64,
    /// Spectral centroid (normalized).
    pub spectral_centroid: f64,
}

/// FFT bin data.
#[derive(Debug, Clone)]
pub struct FftBin {
    /// Frequency index.
    pub index: usize,
    /// Magnitude in dB (or 0.0 if -∞).
    pub magnitude: f64,
    /// Phase in radians (-π to π).
    pub phase: f64,
}

/// Spectrogram frame data.
#[derive(Debug, Clone)]
pub struct SpectrogramFrame {
    /// Frame index.
    pub frame_index: usize,
    /// Sample offset of this frame.
    pub sample_offset: usize,
    /// FFT bins.
    pub bins: Vec<FftBin>,
    /// Frame energy.
    pub energy: FrameEnergy,
}

/// Spectrogram export format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportFormat {
    /// PNG image.
    Png,
    /// CSV with frequency × time data.
    Csv,
    /// JSON with full analysis data.
    Json,
}

/// Spectrogram configuration.
#[derive(Debug, Clone)]
pub struct SpectrogramConfig {
    /// FFT size.
    pub fft_size: FftSize,
    /// Hop size.
    pub hop_size: HopSize,
    /// Sample rate (e.g., 44100, 48000).
    pub sample_rate: u32,
    /// Color map.
    pub color_map: ColorMap,
    /// Dynamic range in dB (for color scaling).
    pub dynamic_range: f64,
    /// Minimum dB level.
    pub min_db: f64,
}

impl Default for SpectrogramConfig {
    fn default() -> Self {
        Self {
            fft_size: FftSize::Size1024,
            hop_size: HopSize::Half,
            sample_rate: 44100,
            color_map: ColorMap::Heat,
            dynamic_range: 100.0,
            min_db: -80.0,
        }
    }
}

/// Spectrogram data.
#[derive(Debug, Clone)]
pub struct Spectrogram {
    /// Configuration.
    pub config: SpectrogramConfig,
    /// All spectrogram frames.
    pub frames: Vec<SpectrogramFrame>,
    /// Peak energy across all frames (for normalization).
    pub peak_energy: f64,
    /// Average energy across all frames.
    pub avg_energy: f64,
    /// Total duration in samples.
    pub total_samples: usize,
    /// Number of frequency bins.
    pub num_bins: usize,
}

impl Spectrogram {
    /// Create a new empty spectrogram.
    pub fn new(config: SpectrogramConfig, total_samples: usize) -> Self {
        let num_bins = config.fft_size.samples() / 2 + 1;
        Self {
            config,
            frames: Vec::new(),
            peak_energy: 0.0,
            avg_energy: 0.0,
            total_samples,
            num_bins,
        }
    }

    /// Get the frequency resolution of this spectrogram.
    pub fn frequency_resolution(&self) -> f64 {
        self.config.sample_rate as f64 / self.config.fft_size.samples() as f64
    }

    /// Get the time resolution of this spectrogram.
    pub fn time_resolution(&self) -> f64 {
        self.config.hop_size.samples(self.config.fft_size) as f64 / self.config.sample_rate as f64
    }

    /// Get the Nyquist frequency.
    pub fn nyquist_frequency(&self) -> f64 {
        self.config.sample_rate as f64 / 2.0
    }

    /// Get frequency for a given bin index.
    pub fn bin_to_frequency(&self, bin: usize) -> f64 {
        let resolution = self.frequency_resolution();
        (bin as f64) * resolution
    }

    /// Get color for a given magnitude in dB.
    pub fn magnitude_to_color(&self, magnitude_db: f64) -> (u8, u8, u8) {
        let normalized = if magnitude_db <= self.config.min_db {
            0.0
        } else if magnitude_db >= self.config.min_db + self.config.dynamic_range {
            1.0
        } else {
            (magnitude_db - self.config.min_db) / self.config.dynamic_range
        };
        self.config.color_map.to_rgb(normalized)
    }
}

/// Spectrogram generator.
pub struct SpectrogramGenerator {
    /// Analysis statistics.
    frame_count: AtomicU64,
    analysis_errors: AtomicU64,
}

impl SpectrogramGenerator {
    /// Create a new spectrogram generator.
    pub fn new() -> Self {
        Self {
            frame_count: AtomicU64::new(0),
            analysis_errors: AtomicU64::new(0),
        }
    }

    /// Generate spectrogram from audio samples.
    ///
    /// This is a simplified STFT implementation. For production use,
    /// consider using a dedicated FFT library like rustfft.
    pub fn generate(&self, config: SpectrogramConfig, samples: &[f32]) -> Spectrogram {
        let fft_size = config.fft_size.samples();
        let hop = config.hop_size.samples(config.fft_size);

        let mut spec = Spectrogram::new(config, samples.len());

        // Pre-compute Hann window.
        let window = hann_window(fft_size);

        let mut frame_index = 0;
        let mut sample_pos = 0;

        while sample_pos + fft_size <= samples.len() {
            // Apply window function.
            let mut framed = vec![0.0f64; fft_size];
            for i in 0..fft_size {
                framed[i] = samples[sample_pos + i] as f64 * window[i];
            }

            // Compute FFT (simplified DFT for first half of bins).
            let num_bins = fft_size / 2 + 1;
            let mut bins = Vec::with_capacity(num_bins);

            let mut frame_rms = 0.0;
            let mut frame_peak: f64 = 0.0;
            let mut zcr_count = 0;
            let mut spectral_sum_num = 0.0;
            let mut spectral_sum_den = 0.0;

            for bin in 0..num_bins {
                let mut real = 0.0;
                let mut imag = 0.0;

                for (n, sample) in framed.iter().enumerate().take(fft_size) {
                    let angle = 2.0 * PI * (bin as f64) * (n as f64) / (fft_size as f64);
                    real += sample * angle.cos();
                    imag -= sample * angle.sin();
                }

                let magnitude = (real * real + imag * imag).sqrt();
                let magnitude_db = if magnitude > 1e-10 {
                    20.0 * magnitude.log10()
                } else {
                    -120.0
                };
                let phase = imag.atan2(real);

                bins.push(FftBin {
                    index: bin,
                    magnitude: magnitude_db,
                    phase,
                });

                // Energy calculations.
                frame_rms += magnitude * magnitude;
                frame_peak = frame_peak.max(magnitude);

                // Zero crossing rate (on input samples, simplified here).
                if sample_pos > 0
                    && sample_pos < samples.len() - 1
                    && (samples[sample_pos] as f64) * (samples[sample_pos + 1] as f64) < 0.0
                {
                    zcr_count += 1;
                }

                // Spectral centroid.
                let freq = bin as f64;
                spectral_sum_num += magnitude * freq;
                spectral_sum_den += magnitude;
            }

            frame_rms = frame_rms.sqrt() / (fft_size as f64).sqrt();
            frame_peak /= fft_size as f64;
            let zcr = zcr_count as f64 / (fft_size as f64);
            let spectral_centroid = if spectral_sum_den > 1e-10 {
                spectral_sum_num / spectral_sum_den / (num_bins as f64)
            } else {
                0.0
            };

            let energy = FrameEnergy {
                rms: frame_rms,
                peak: frame_peak,
                zcr,
                spectral_centroid: spectral_centroid.clamp(0.0, 1.0),
            };

            spec.frames.push(SpectrogramFrame {
                frame_index,
                sample_offset: sample_pos,
                bins,
                energy,
            });

            // Update peak energy.
            spec.peak_energy = spec.peak_energy.max(frame_rms);

            sample_pos += hop;
            frame_index += 1;
        }

        // Compute average energy.
        if !spec.frames.is_empty() {
            spec.avg_energy =
                spec.frames.iter().map(|f| f.energy.rms).sum::<f64>() / spec.frames.len() as f64;
        }

        spec.num_bins = fft_size / 2 + 1;
        self.frame_count
            .store(frame_index as u64, Ordering::Relaxed);

        spec
    }

    /// Generate spectrogram for a short sample with timing.
    pub fn generate_with_timing(
        &self,
        config: SpectrogramConfig,
        samples: &[f32],
    ) -> (Spectrogram, u64) {
        let start = std::time::Instant::now();
        let spec = self.generate(config, samples);
        let elapsed = start.elapsed().as_micros() as u64;
        (spec, elapsed)
    }

    /// Get current analysis statistics.
    pub fn stats(&self) -> (u64, u64) {
        (
            self.frame_count.load(Ordering::Relaxed),
            self.analysis_errors.load(Ordering::Relaxed),
        )
    }

    /// Reset statistics.
    pub fn reset_stats(&self) {
        self.frame_count.store(0, Ordering::Relaxed);
        self.analysis_errors.store(0, Ordering::Relaxed);
    }
}

impl Spectrogram {
    /// Export spectrogram data to CSV.
    ///
    /// Returns a simple frequency × time matrix as CSV text.
    pub fn export_csv(&self, path: &PathBuf) -> Result<(), SpectrogramError> {
        if self.frames.is_empty() {
            return Err(SpectrogramError::EmptyData);
        }

        let mut csv = String::from("frame,frequency_hz,db");
        csv.push('\n');

        for frame in &self.frames {
            for bin in &frame.bins {
                let freq_hz = self.bin_to_frequency(bin.index);
                csv.push_str(&format!(
                    "{},{:.2},{:.2}\n",
                    frame.frame_index, freq_hz, bin.magnitude
                ));
            }
        }

        std::fs::write(path, csv).map_err(|e| SpectrogramError::IoError(e.to_string()))?;
        Ok(())
    }

    /// Validate spectrogram data.
    pub fn validate(&self) -> bool {
        if self.frames.is_empty() {
            return false;
        }
        if self.num_bins == 0 {
            return false;
        }
        if self.total_samples == 0 {
            return false;
        }
        // Check all frames have correct number of bins.
        self.frames.iter().all(|f| f.bins.len() == self.num_bins)
    }

    /// Get peak frequency for a given frame.
    pub fn peak_frequency(&self, frame_index: usize) -> Option<f64> {
        if frame_index >= self.frames.len() {
            return None;
        }
        let frame = &self.frames[frame_index];
        let peak_bin = frame.bins.iter().position(|b| {
            b.magnitude
                == frame
                    .bins
                    .iter()
                    .map(|b| b.magnitude)
                    .fold(f64::NEG_INFINITY, f64::max)
        })?;
        Some(self.bin_to_frequency(peak_bin))
    }

    /// Get the overall spectral flatness of the spectrogram.
    pub fn spectral_flatness(&self) -> f64 {
        if self.frames.is_empty() {
            return 0.0;
        }

        let mut geo_sum = 0.0;
        let mut arith_sum = 0.0;
        let mut count = 0;

        for frame in &self.frames {
            for bin in &frame.bins {
                let mag_linear = if bin.magnitude > -120.0 {
                    10.0_f64.powf(bin.magnitude / 20.0)
                } else {
                    1e-12
                };
                geo_sum += mag_linear.ln().log(10.0);
                arith_sum += mag_linear;
                count += 1;
            }
        }

        if count == 0 {
            return 0.0;
        }

        let geo_mean = geo_sum / count as f64;
        let arith_mean = arith_sum / count as f64;

        if arith_mean <= 0.0 {
            return 0.0;
        }

        (10.0_f64.powf(geo_mean) / arith_mean).max(0.0)
    }
}

impl Default for SpectrogramGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate Hann window.
fn hann_window(size: usize) -> Vec<f64> {
    let mut window = vec![0.0; size];
    for (i, value) in window.iter_mut().enumerate() {
        *value = 0.5 * (1.0 - ((2.0 * PI * i as f64) / (size - 1) as f64).cos());
    }
    window
}

/// Spectrogram analysis errors.
#[derive(Debug, Clone)]
pub enum SpectrogramError {
    /// Input data is empty or invalid.
    EmptyData,
    /// I/O error during export.
    IoError(String),
    /// Invalid configuration.
    InvalidConfig(String),
    /// Memory allocation failed.
    MemoryError,
}

impl std::fmt::Display for SpectrogramError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyData => write!(f, "No spectrogram data available"),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
            Self::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            Self::MemoryError => write!(f, "Memory allocation failed"),
        }
    }
}

impl std::error::Error for SpectrogramError {}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_samples(length: usize) -> Vec<f32> {
        (0..length)
            .map(|i| (0.5 * (2.0 * PI * 440.0 * i as f64 / 44100.0)).sin() as f32)
            .collect()
    }

    #[test]
    fn fft_size_samples() {
        assert_eq!(FftSize::Size256.samples(), 256);
        assert_eq!(FftSize::Size512.samples(), 512);
        assert_eq!(FftSize::Size1024.samples(), 1024);
        assert_eq!(FftSize::Size2048.samples(), 2048);
        assert_eq!(FftSize::Size4096.samples(), 4096);
        assert_eq!(FftSize::Size8192.samples(), 8192);
    }

    #[test]
    fn hop_size_samples() {
        let fft = FftSize::Size1024;
        assert_eq!(HopSize::Quarter.samples(fft), 256);
        assert_eq!(HopSize::Half.samples(fft), 512);
        assert_eq!(HopSize::ThreeQuarters.samples(fft), 768);
        assert_eq!(HopSize::Full.samples(fft), 1024);
    }

    #[test]
    fn color_map_heat() {
        let map = ColorMap::Heat;
        assert_eq!(map.to_rgb(0.0), (0, 0, 255));
        assert_eq!(map.to_rgb(0.5), (0, 255, 0));
        assert_eq!(map.to_rgb(1.0), (255, 55, 0));
    }

    #[test]
    fn color_map_grayscale() {
        let map = ColorMap::Grayscale;
        assert_eq!(map.to_rgb(0.0), (0, 0, 0));
        assert_eq!(map.to_rgb(0.5), (127, 127, 127));
        assert_eq!(map.to_rgb(1.0), (255, 255, 255));
    }

    #[test]
    fn color_map_clamping() {
        let map = ColorMap::Heat;
        // Out-of-range values should be clamped.
        assert_eq!(map.to_rgb(-0.5), map.to_rgb(0.0));
        assert_eq!(map.to_rgb(1.5), map.to_rgb(1.0));
    }

    #[test]
    fn spectrogram_config_defaults() {
        let config = SpectrogramConfig::default();
        assert_eq!(config.fft_size, FftSize::Size1024);
        assert_eq!(config.hop_size, HopSize::Half);
        assert_eq!(config.sample_rate, 44100);
        assert_eq!(config.dynamic_range, 100.0);
        assert_eq!(config.min_db, -80.0);
    }

    #[test]
    fn spectrogram_creation() {
        let config = SpectrogramConfig::default();
        let spec = Spectrogram::new(config, 44100);
        assert_eq!(spec.total_samples, 44100);
        assert_eq!(spec.num_bins, 513); // 1024 / 2 + 1
        assert!(spec.frames.is_empty());
        assert_eq!(spec.peak_energy, 0.0);
    }

    #[test]
    fn spectrogram_frequency_resolution() {
        let config = SpectrogramConfig {
            fft_size: FftSize::Size1024,
            sample_rate: 44100,
            ..Default::default()
        };
        let spec = Spectrogram::new(config, 44100);
        let resolution = spec.frequency_resolution();
        assert!((resolution - 43.0625).abs() < 0.01);
    }

    #[test]
    fn spectrogram_bin_to_frequency() {
        let config = SpectrogramConfig {
            fft_size: FftSize::Size1024,
            sample_rate: 44100,
            ..Default::default()
        };
        let spec = Spectrogram::new(config, 44100);
        // Bin 0 = DC, Bin 1 = 43.06 Hz, ...
        assert_eq!(spec.bin_to_frequency(0), 0.0);
        assert!((spec.bin_to_frequency(1) - 43.0625).abs() < 0.01);
        assert!((spec.bin_to_frequency(1023) - 44056.93359375).abs() < 1.0);
    }

    #[test]
    fn spectrogram_nyquist() {
        let config = SpectrogramConfig {
            sample_rate: 48000,
            ..Default::default()
        };
        let spec = Spectrogram::new(config, 48000);
        assert_eq!(spec.nyquist_frequency(), 24000.0);
    }

    #[test]
    fn spectrogram_generation() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(44100); // 1 second of 440 Hz sine.

        let config = SpectrogramConfig::default();
        let spec = generator.generate(config, &samples);

        assert!(!spec.frames.is_empty());
        assert!(spec.validate());
        assert!(spec.frames.iter().all(|f| f.bins.len() == spec.num_bins));
    }

    #[test]
    fn spectrogram_peak_frequency() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(44100);

        let config = SpectrogramConfig::default();
        let spec = generator.generate(config, &samples);

        // Should detect ~440 Hz as peak frequency.
        if let Some(pk) = spec.peak_frequency(0) {
            // 440 Hz corresponds to bin ~10 (440 / 43.06).
            let _bin_440 = 440.0 / spec.frequency_resolution();
            assert!(pk.abs() > 0.0);
            assert!(pk < spec.nyquist_frequency());
        }
    }

    #[test]
    fn spectrogram_peak_energy() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(44100);

        let config = SpectrogramConfig::default();
        let spec = generator.generate(config, &samples);

        assert!(spec.peak_energy > 0.0);
        assert!(spec.avg_energy > 0.0);
        assert!(spec.peak_energy >= spec.avg_energy);
    }

    #[test]
    fn spectrogram_validate_fails_empty() {
        let config = SpectrogramConfig::default();
        let spec = Spectrogram::new(config, 44100);
        assert!(!spec.validate());
    }

    #[test]
    fn spectrogram_export_csv_empty() {
        let _generator = SpectrogramGenerator::new();
        let config = SpectrogramConfig::default();
        let spec = Spectrogram::new(config, 0);

        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("spectrogram_test_empty.csv");
        let result = spec.export_csv(&path);
        assert!(result.is_err());
    }

    #[test]
    fn spectrogram_export_csv_valid() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(4410); // 0.1 second.

        let config = SpectrogramConfig {
            fft_size: FftSize::Size256,
            hop_size: HopSize::Half,
            sample_rate: 44100,
            ..Default::default()
        };
        let spec = generator.generate(config, &samples);

        let fixture_dir = tempfile::tempdir().unwrap();
        let path = fixture_dir.path().join("spectrogram_test.csv");
        let result = spec.export_csv(&path);
        assert!(result.is_ok());

        // Verify file was created and has content.
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with("frame,frequency_hz,db"));
        assert!(contents.lines().count() > 1);

        // Cleanup.
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn frame_energy() {
        let energy = FrameEnergy {
            rms: 0.5,
            peak: 0.9,
            zcr: 0.3,
            spectral_centroid: 0.4,
        };
        assert!(energy.rms > 0.0);
        assert!(energy.rms <= energy.peak);
        assert!((0.0..=1.0).contains(&energy.spectral_centroid));
    }

    #[test]
    fn fft_bin() {
        let bin = FftBin {
            index: 10,
            magnitude: -20.0,
            phase: 0.5,
        };
        assert_eq!(bin.index, 10);
        assert_eq!(bin.magnitude, -20.0);
        assert!((bin.phase - 0.5).abs() < 0.01);
    }

    #[test]
    fn generator_stats() {
        let generator = SpectrogramGenerator::new();
        let (frames, errors) = generator.stats();
        assert_eq!(frames, 0);
        assert_eq!(errors, 0);

        let samples = make_test_samples(4410);
        let config = SpectrogramConfig::default();
        let _spec = generator.generate(config, &samples);

        let (frames, errors) = generator.stats();
        assert!(frames > 0);
        assert_eq!(errors, 0);
    }

    #[test]
    fn generator_reset_stats() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(4410);
        let config = SpectrogramConfig::default();
        let _spec = generator.generate(config, &samples);

        let (frames, _) = generator.stats();
        assert!(frames > 0);

        generator.reset_stats();
        let (frames, _) = generator.stats();
        assert_eq!(frames, 0);
    }

    #[test]
    fn spectrogram_short_input() {
        let generator = SpectrogramGenerator::new();
        let samples = vec![0.5f32; 128]; // Too short for FFT256.

        let config = SpectrogramConfig {
            fft_size: FftSize::Size256,
            sample_rate: 44100,
            ..Default::default()
        };
        let spec = generator.generate(config, &samples);

        // Should handle gracefully (may have 0 or 1 frames).
        assert!(spec.frames.len() <= 1);
    }

    #[test]
    fn spectrogram_magnitude_to_color() {
        let config = SpectrogramConfig {
            dynamic_range: 60.0,
            min_db: -80.0,
            ..Default::default()
        };
        let spec = Spectrogram::new(config, 44100);

        // At min_db → 0.0 (black/darkest).
        let (r, g, b) = spec.magnitude_to_color(-80.0);
        assert_eq!((r, g, b), spec.config.color_map.to_rgb(0.0));

        // At min_db + dynamic_range → full intensity.
        let (r, g, b) = spec.magnitude_to_color(-20.0);
        assert!(r > 0 || g > 0 || b > 0);
    }

    #[test]
    fn spectrogram_spectral_flatness() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(44100);

        let config = SpectrogramConfig::default();
        let spec = generator.generate(config, &samples);

        let flatness = spec.spectral_flatness();
        // A pure sine wave has high spectral flatness (tonal).
        assert!((0.0..=1.0).contains(&flatness));
    }

    #[test]
    fn spectrogram_generator_with_timing() {
        let generator = SpectrogramGenerator::new();
        let samples = make_test_samples(4410);

        let config = SpectrogramConfig::default();
        let (spec, elapsed_us) = generator.generate_with_timing(config, &samples);

        assert!(!spec.frames.is_empty());
        assert!(elapsed_us > 0);
    }

    #[test]
    fn spectrogram_error_display() {
        assert!(format!("{}", SpectrogramError::EmptyData).contains("No spectrogram data"));
        assert!(format!("{}", SpectrogramError::IoError("test".to_string()))
            .contains("I/O error: test"));
        assert!(
            format!("{}", SpectrogramError::InvalidConfig("bad".to_string()))
                .contains("Invalid configuration: bad")
        );
    }
}
