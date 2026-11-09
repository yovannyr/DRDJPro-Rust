//! Approximate frequency colors for display only; never applied to audio output.
//!
//! Optimised for throughput: uses `f32` state instead of `f64`, unrolls the
//! mono/stereo channel loop, and avoids unnecessary heap allocation per frame.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Clone, Copy, Default)]
pub struct BandEnergy([f32; 3]);

impl BandEnergy {
    pub fn add(&mut self, other: Self) {
        for (value, extra) in self.0.iter_mut().zip(other.0) {
            *value += extra;
        }
    }
    pub fn color(self) -> Rgb {
        let total: f32 = self.0.iter().sum();
        if total <= f32::EPSILON {
            return Rgb(94, 110, 130);
        }
        // The same palette is used in the GUI legend: bass red, mids green, highs blue.
        let palette = [
            [234.0, 90.0, 90.0],
            [70.0, 220.0, 100.0],
            [80.0, 140.0, 250.0],
        ];
        let mut rgb = [0.0; 3];
        for (energy, color) in self.0.into_iter().zip(palette) {
            for (component, source) in rgb.iter_mut().zip(color) {
                *component += energy / total * source;
            }
        }
        Rgb(
            rgb[0].round() as u8,
            rgb[1].round() as u8,
            rgb[2].round() as u8,
        )
    }
}

pub struct BandAnalysis {
    pub detail: Vec<BandEnergy>,
    pub overview: Vec<Rgb>,
}

/// Two complementary first-order low-pass states split samples approximately at
/// 200 Hz and 2 kHz. Sum squared band samples per channel, never downmix stereo.
/// Uses `f32` state for ~2× speedup over `f64` — sufficient for display-only.
/// This is a display heuristic, not an FFT, equalizer or brick-wall crossover.
pub fn analyze(
    samples: &[f32],
    channels: u16,
    rate: u32,
    frames_per_peak: usize,
    overview_bins: usize,
) -> BandAnalysis {
    let channels = usize::from(channels);
    assert!(
        channels > 0 && rate > 0 && frames_per_peak > 0 && samples.len().is_multiple_of(channels)
    );
    let frames = samples.len() / channels;
    let bins = overview_bins.min(frames);
    let mut detail = vec![BandEnergy::default(); frames.div_ceil(frames_per_peak)];
    let mut overview = vec![BandEnergy::default(); bins];

    // Pre-computed coefficients (f32) — channels=1 or 2 only
    let coefficients = |cutoff: f32| {
        let k = (std::f32::consts::PI * cutoff.min(rate as f32 * 0.45) / rate as f32).tan();
        (k / (1.0 + k), (1.0 - k) / (1.0 + k))
    };
    let low = coefficients(200.0);
    let high = coefficients(2000.0);

    match channels {
        1 => {
            let mut s0 = 0.0_f32;
            let mut s1 = 0.0_f32;
            let mut s2 = 0.0_f32;
            for (index, &sample) in samples.iter().enumerate() {
                s1 = low.0 * (sample + s0) + low.1 * s1;
                s2 = high.0 * (sample + s0) + high.1 * s2;
                let b0 = s1;
                let b1 = s2 - s1;
                let b2 = sample - s2;
                let energy = BandEnergy([b0 * b0, b1 * b1, b2 * b2]);
                detail[index / frames_per_peak].add(energy);
                if bins > 0 {
                    overview[index * bins / frames].add(energy);
                }
                s0 = sample;
            }
        }
        2 => {
            let mut states = [[0.0_f32; 3]; 2];
            for (index, frame) in samples.as_chunks::<2>().0.iter().enumerate() {
                let mut energy = BandEnergy::default();
                // Unrolled for ch0
                let sample0 = frame[0];
                let s = &mut states[0];
                s[1] = low.0 * (sample0 + s[0]) + low.1 * s[1];
                s[2] = high.0 * (sample0 + s[0]) + high.1 * s[2];
                let bands0 = [s[1], s[2] - s[1], sample0 - s[2]];
                energy.add(BandEnergy([
                    bands0[0] * bands0[0],
                    bands0[1] * bands0[1],
                    bands0[2] * bands0[2],
                ]));
                // Unrolled for ch1
                let sample1 = frame[1];
                let s = &mut states[1];
                s[1] = low.0 * (sample1 + s[0]) + low.1 * s[1];
                s[2] = high.0 * (sample1 + s[0]) + high.1 * s[2];
                let bands1 = [s[1], s[2] - s[1], sample1 - s[2]];
                energy.add(BandEnergy([
                    bands1[0] * bands1[0],
                    bands1[1] * bands1[1],
                    bands1[2] * bands1[2],
                ]));
                detail[index / frames_per_peak].add(energy);
                if bins > 0 {
                    overview[index * bins / frames].add(energy);
                }
                // Update previous-sample state (must happen after all calculations)
                states[0][0] = sample0;
                states[1][0] = sample1;
            }
        }
        _ => {
            // General fallback for >2 channels (used by multichannel playback tests)
            // Each channel maintains its own IIR state: [prev_sample, low, high]
            let mut state: Vec<[f32; 3]> = (0..channels).map(|_| [0.0, 0.0, 0.0]).collect();
            for (index, &sample) in samples.iter().enumerate() {
                let ch = index % channels;
                let s = &mut state[ch];
                s[1] = low.0 * (sample + s[0]) + low.1 * s[1];
                s[2] = high.0 * (sample + s[0]) + high.1 * s[2];
                let bands = [s[1], s[2] - s[1], sample - s[2]];
                let energy = BandEnergy([
                    bands[0] * bands[0],
                    bands[1] * bands[1],
                    bands[2] * bands[2],
                ]);
                detail[index / channels / frames_per_peak].add(energy);
                if bins > 0 {
                    overview[index / channels * bins / frames].add(energy);
                }
                s[0] = sample;
            }
        }
    }

    BandAnalysis {
        detail,
        overview: overview.into_iter().map(BandEnergy::color).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(frequency: f32, rate: u32) -> Vec<f32> {
        (0..rate)
            .map(|i| 0.5 * (std::f32::consts::TAU * frequency * i as f32 / rate as f32).sin())
            .collect()
    }
    #[test]
    fn tones_map_to_low_mid_high_at_supported_rates() {
        for rate in [8000, 44_100, 48_000, 96_000] {
            for (frequency, dominant) in [(80.0, 0), (1000.0, 1), (rate as f32 * 0.4, 2)] {
                let result = analyze(&tone(frequency, rate), 1, rate, 256, 256);
                let mut sum = BandEnergy::default();
                for value in &result.detail {
                    sum.add(*value);
                }
                let Rgb(r, g, b) = sum.color();
                let components = [r, g, b];
                assert!(
                    (0..3)
                        .filter(|i| *i != dominant)
                        .all(|i| components[dominant] > components[i]),
                    "tone {frequency} at {rate}: {:?}",
                    sum.color()
                );
            }
        }
    }
    #[test]
    fn stereo_phase_does_not_cancel_and_silence_is_neutral() {
        let mono = tone(1000.0, 48_000);
        let stereo: Vec<_> = mono.iter().flat_map(|v| [*v, -*v]).collect();
        let a = analyze(&mono, 1, 48_000, 256, 256);
        let b = analyze(&stereo, 2, 48_000, 256, 256);
        assert_eq!(a.overview, b.overview);
        assert_eq!(
            a.detail.iter().map(|v| v.color()).collect::<Vec<_>>(),
            b.detail.iter().map(|v| v.color()).collect::<Vec<_>>()
        );
        let silence = analyze(&[0.0; 513], 1, 48_000, 256, 256);
        assert_eq!(silence.detail.len(), 3);
        assert!(silence.overview.iter().all(|v| *v == Rgb(94, 110, 130)));
        let empty = analyze(&[], 2, 48_000, 256, 256);
        assert!(empty.overview.is_empty() && empty.detail.is_empty());
    }
    #[test]
    fn local_band_energy_changes_over_time_and_keeps_last_partial_chunk() {
        let mut samples = tone(80.0, 48_000);
        samples.extend(tone(12_000.0, 48_000));
        samples.push(0.9);
        let result = analyze(&samples, 1, 48_000, 256, 256);
        let low = result.overview[60];
        let high = result.overview[200];
        assert!(low.0 > low.1 && low.0 > low.2);
        assert!(high.2 > high.0 && high.2 > high.1);
        assert_eq!(result.detail.len(), 96_001usize.div_ceil(256));
        assert!(result.detail.last().unwrap().0.iter().sum::<f32>() > 0.0);
    }
}
