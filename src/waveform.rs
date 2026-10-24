//! Fixed-size full-track peak overview, computed only while preparing a track.

pub const OVERVIEW_BINS: usize = 256;

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
