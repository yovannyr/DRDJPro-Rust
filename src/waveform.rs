//! Fixed-size full-track peak overview, computed only while preparing a track.

pub const OVERVIEW_BINS: usize = 256;
const DETAIL_FRAMES_PER_PEAK: usize = 256;

/// A bounded high-resolution envelope, built by the loading worker.
/// Peak and band energy data together retain at most 7 MiB per mono track at the PCM limit.
pub struct DetailPeaks {
    peaks: Vec<f32>,
    frames: usize,
    sample_rate: u32,
    bands: crate::waveform_color::BandAnalysis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetailRange {
    pub start_frame: usize,
    pub end_frame: usize,
}

impl DetailPeaks {
    pub fn new(samples: &[f32], channels: u16, sample_rate: u32) -> Self {
        let channels = usize::from(channels);
        assert!(channels > 0 && sample_rate > 0 && samples.len().is_multiple_of(channels));
        let peaks = samples
            .chunks(DETAIL_FRAMES_PER_PEAK * channels)
            .map(|chunk| {
                chunk
                    .iter()
                    .fold(0.0_f32, |peak, sample| peak.max(sample.abs()))
            })
            .collect();
        Self {
            peaks,
            frames: samples.len() / channels,
            sample_rate,
            bands: crate::waveform_color::analyze(
                samples,
                channels as u16,
                sample_rate,
                DETAIL_FRAMES_PER_PEAK,
                OVERVIEW_BINS,
            ),
        }
    }

    /// Follow the playhead, pinning the viewport to the track boundaries.
    pub fn range(&self, position: std::time::Duration, window_seconds: u32) -> DetailRange {
        let visible = (u64::from(window_seconds.clamp(2, 32)) * u64::from(self.sample_rate))
            .min(self.frames as u64) as usize;
        let frame = (position.as_secs_f64() * f64::from(self.sample_rate))
            .round()
            .clamp(0.0, self.frames as f64) as usize;
        let start = frame.saturating_sub(visible / 2).min(self.frames - visible);
        DetailRange {
            start_frame: start,
            end_frame: start + visible,
        }
    }

    pub fn percent(&self, frame: usize) -> f32 {
        if self.frames == 0 {
            0.0
        } else {
            100.0 * frame as f32 / self.frames as f32
        }
    }

    /// Re-bin only the small visible envelope; never scan PCM on the GUI timer.
    pub fn render(&self, range: DetailRange) -> Vec<f32> {
        self.peak_ranges(range)
            .map(|range| self.peaks[range].iter().copied().fold(0.0_f32, f32::max))
            .collect()
    }

    pub fn overview_colors(&self) -> &[crate::waveform_color::Rgb] {
        &self.bands.overview
    }

    pub fn render_colors(&self, range: DetailRange) -> Vec<crate::waveform_color::Rgb> {
        self.peak_ranges(range)
            .map(|range| {
                let mut energy = crate::waveform_color::BandEnergy::default();
                for value in &self.bands.detail[range] {
                    energy.add(*value);
                }
                energy.color()
            })
            .collect()
    }

    fn peak_ranges(&self, range: DetailRange) -> impl Iterator<Item = std::ops::Range<usize>> + '_ {
        let frames = range.end_frame - range.start_frame;
        let bins = frames.min(OVERVIEW_BINS);
        (0..bins).map(move |i| {
            let first = range.start_frame + i * frames / bins;
            let end = range.start_frame + (i + 1) * frames / bins;
            let start_peak = first / DETAIL_FRAMES_PER_PEAK;
            let end_peak = end.div_ceil(DETAIL_FRAMES_PER_PEAK).min(self.peaks.len());
            start_peak..end_peak
        })
    }
}

/// Each bin covers the same fraction of the track. Channel magnitudes are combined
/// with max, not averaged, so opposite-phase stereo signals cannot cancel out.
pub fn peak_overview(samples: &[f32], channels: u16) -> Vec<f32> {
    let channels = usize::from(channels);
    if channels == 0 || samples.is_empty() || !samples.len().is_multiple_of(channels) {
        return Vec::new();
    }
    let frames = samples.len() / channels;
    let bins = frames.min(OVERVIEW_BINS);
    let mut peaks = vec![0.0_f32; bins];
    for (frame_index, frame) in samples.chunks_exact(channels).enumerate() {
        let bin = frame_index * bins / frames;
        for sample in frame {
            peaks[bin] = peaks[bin].max(sample.abs());
        }
    }
    peaks
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn detail_color_rebin_matches_visible_frequency_content_and_peak_count() {
        let rate = 8000;
        let samples: Vec<_> = (0..rate * 8)
            .map(|frame| {
                let frequency = if frame < rate * 4 { 80.0 } else { 2800.0 };
                0.5 * (std::f32::consts::TAU * frequency * frame as f32 / rate as f32).sin()
            })
            .collect();
        let detail = DetailPeaks::new(&samples, 1, rate);
        let low_range = detail.range(Duration::from_secs(1), 2);
        let high_range = detail.range(Duration::from_secs(6), 2);
        let low = detail.render_colors(low_range);
        let high = detail.render_colors(high_range);
        assert_eq!(low.len(), detail.render(low_range).len());
        assert_eq!(high.len(), detail.render(high_range).len());
        assert!(low[128].0 > low[128].1 && low[128].0 > low[128].2);
        assert!(high[128].2 > high[128].0 && high[128].2 > high[128].1);
        assert_eq!(
            detail.overview_colors().len(),
            peak_overview(&samples, 1).len()
        );
    }

    #[test]
    fn detail_follows_position_and_clamps_zoom_and_track_edges() {
        let detail = DetailPeaks::new(&vec![0.0; 48_000 * 30], 1, 48_000);
        assert_eq!(
            detail.range(Duration::ZERO, 8),
            DetailRange {
                start_frame: 0,
                end_frame: 384_000
            }
        );
        assert_eq!(
            detail.range(Duration::from_secs(15), 8),
            DetailRange {
                start_frame: 528_000,
                end_frame: 912_000
            }
        );
        assert_eq!(
            detail.range(Duration::from_secs(30), 8),
            DetailRange {
                start_frame: 1_056_000,
                end_frame: 1_440_000
            }
        );
        let zoomed = detail.range(Duration::from_secs(15), 4);
        assert_eq!(
            zoomed,
            DetailRange {
                start_frame: 624_000,
                end_frame: 816_000
            }
        );
        assert_eq!(detail.percent(zoomed.start_frame), 100.0 * 13.0 / 30.0);
        assert_eq!(
            detail.range(Duration::ZERO, 0),
            detail.range(Duration::ZERO, 2)
        );
        assert_eq!(
            detail.range(Duration::from_secs(100), 100),
            detail.range(Duration::from_secs(30), 32)
        );
        let short = DetailPeaks::new(&[0.1, -0.7, 0.2], 1, 44_100);
        let range = short.range(Duration::from_secs(1), 8);
        assert_eq!(
            range,
            DetailRange {
                start_frame: 0,
                end_frame: 3
            }
        );
        assert_eq!(short.render(range), vec![0.7; 3]);
    }

    #[test]
    fn detail_preserves_stereo_transients_final_chunk_and_local_content() {
        let mut samples: Vec<_> = (0..48_000 * 20)
            .flat_map(|frame| {
                let value = if frame < 48_000 * 10 { 0.25 } else { 0.75 };
                [value, -value]
            })
            .collect();
        samples.extend_from_slice(&[0.0, -0.9]);
        let detail = DetailPeaks::new(&samples, 2, 48_000);
        assert_eq!(detail.peaks.len(), (48_000usize * 20 + 1).div_ceil(256));
        assert_eq!(detail.peaks.last(), Some(&0.9));
        let start = detail.render(detail.range(Duration::ZERO, 4));
        assert_eq!(start, vec![0.25; OVERVIEW_BINS]);
        let middle = detail.render(detail.range(Duration::from_secs(15), 4));
        assert_eq!(middle, vec![0.75; OVERVIEW_BINS]);
        let final_view = detail.render(detail.range(Duration::from_secs(21), 4));
        assert_eq!(final_view.last(), Some(&0.9));
        let start = std::time::Instant::now();
        for frame in 0..1000 {
            let range = detail.range(Duration::from_millis(frame * 10), 8);
            let peaks = detail.render(range);
            assert!(peaks.len() <= OVERVIEW_BINS);
            assert_eq!(peaks.len(), detail.render_colors(range).len());
        }
        eprintln!("1000 detail peak/color re-bins: {:?}", start.elapsed());
    }

    #[test]
    fn empty_short_and_silent_tracks_have_honest_peaks() {
        assert!(peak_overview(&[], 2).is_empty());
        assert!(peak_overview(&[0.5], 0).is_empty());
        assert!(peak_overview(&[0.5], 2).is_empty());
        assert_eq!(peak_overview(&[0.0; 1024], 2), vec![0.0; OVERVIEW_BINS]);
        assert_eq!(
            peak_overview(&[-0.25, 0.75, -0.5], 1),
            vec![0.25, 0.75, 0.5]
        );
        assert_eq!(peak_overview(&[0.5, -0.5], 2), vec![0.5]);
    }

    #[test]
    fn stereo_peaks_preserve_both_channels_without_cancellation_or_normalization() {
        let samples = [0.0, -0.75, 0.5, -0.5, 1.25, 0.0, 0.0, 0.0];
        assert_eq!(peak_overview(&samples, 2), vec![0.75, 0.5, 1.25, 0.0]);
    }

    #[test]
    fn uneven_bins_include_start_boundaries_and_last_frame() {
        // 513 frames into 256 bins: the first bin contains 3 frames, all others 2.
        let mut samples = vec![0.0; 513];
        samples[0] = -0.2;
        samples[2] = 0.8;
        samples[3] = -0.4;
        samples[512] = -0.9;
        let peaks = peak_overview(&samples, 1);
        assert_eq!(peaks.len(), OVERVIEW_BINS);
        assert_eq!(&peaks[..2], &[0.8, 0.4]);
        assert_eq!(peaks[255], 0.9);
        assert!(peaks[2..255].iter().all(|v| *v == 0.0));
    }

    #[test]
    fn long_track_has_bounded_overview() {
        let samples = vec![0.25; 48_000 * 2 * 30];
        let start = std::time::Instant::now();
        let peaks = peak_overview(&samples, 2);
        eprintln!("30s stereo peak analysis: {:?}", start.elapsed());
        assert_eq!(peaks, vec![0.25; OVERVIEW_BINS]);
    }
}
