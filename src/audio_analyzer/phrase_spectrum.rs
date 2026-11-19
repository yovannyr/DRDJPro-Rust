use super::{Complex, CHROMA_FFT_SIZE, CHROMA_HOP};

const BANDS: usize = 8;
type Profile = [f32; BANDS];

/// Per-second mean log band powers, measured on the key analyzer's existing
/// FFT. The one-hour decode limit bounds this worker-local history.
pub(super) struct PhraseSpectrum {
    rate: u32,
    next_center: u64,
    band_ends: [usize; BANDS],
    seconds: Vec<Profile>,
    frames_in_second: u32,
}

impl PhraseSpectrum {
    pub(super) fn new(rate: u32) -> Self {
        let band_ends = [80, 160, 320, 640, 1280, 2560, 5120, 10000]
            .map(|hz| ((hz * CHROMA_FFT_SIZE / rate as usize) + 1).min(CHROMA_FFT_SIZE / 2 + 1));
        Self {
            rate,
            next_center: (CHROMA_FFT_SIZE / 2) as u64,
            band_ends,
            seconds: Vec::new(),
            frames_in_second: 0,
        }
    }

    pub(super) fn push(&mut self, fft: Option<&[Complex<f32>]>) {
        if self.rate < 16_000 {
            return;
        }
        let second = (self.next_center / u64::from(self.rate)) as usize;
        self.next_center += CHROMA_HOP as u64;
        if second >= self.seconds.len() {
            self.seconds.resize(second + 1, [0.; BANDS]);
            self.frames_in_second = 0;
        }
        let mut profile = [0.; BANDS];
        let mut start = 1; // exclude DC
        for (level, &end) in profile.iter_mut().zip(&self.band_ends) {
            let power = fft.map_or(0., |bins| {
                bins[start..end].iter().map(|z| z.norm_sqr()).sum()
            });
            *level = 10. * (power / (CHROMA_FFT_SIZE as f32).powi(2)).max(1e-7).log10();
            start = end;
        }
        // Keep some common loudness, but weight changes of spectral balance more heavily.
        let common = 0.75 * profile.iter().sum::<f32>() / BANDS as f32;
        self.frames_in_second += 1;
        for (mean, level) in self.seconds[second].iter_mut().zip(profile) {
            *mean += (level - common - *mean) / self.frames_in_second as f32;
        }
    }

    pub(super) fn boundaries(&self) -> Vec<u32> {
        boundaries(&self.seconds)
    }
}

fn contrast(before: &[Profile], after: &[Profile]) -> f32 {
    let mut delta = [0.; BANDS];
    for (left, right) in before.iter().zip(after) {
        for ((sum, a), b) in delta.iter_mut().zip(left).zip(right) {
            *sum += b - a;
        }
    }
    (delta.iter().map(|d| d * d).sum::<f32>() / BANDS as f32).sqrt() / before.len() as f32
}

fn boundaries(seconds: &[Profile]) -> Vec<u32> {
    // Require a persistent change at two time scales. Only local maxima can
    // become cues, and distinct candidates must be at least a phrase apart.
    let mut scores = vec![0_f32; seconds.len()];
    for i in 4..seconds.len().saturating_sub(3) {
        let wide = i.min(seconds.len() - i).min(12);
        let short = contrast(&seconds[i - 4..i], &seconds[i..i + 4]);
        let long = contrast(&seconds[i - wide..i], &seconds[i..i + wide]);
        scores[i] = short.min(long);
    }
    let mut peaks: Vec<_> = scores
        .windows(3)
        .enumerate()
        .filter(|(_, w)| w[1] >= 2. && w[1] > w[0] && w[1] >= w[2])
        .map(|(i, w)| (i + 1, w[1]))
        .collect();
    peaks.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut selected = Vec::new();
    for (i, _) in peaks {
        let ms = i as u32 * 1000;
        if selected
            .iter()
            .all(|other: &u32| ms.abs_diff(*other) >= 12_000)
        {
            selected.push(ms);
            if selected.len() == 7 {
                break;
            }
        }
    }
    selected.sort_unstable();
    selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_level_timbre_change_survives_but_a_brief_accent_does_not() {
        let dark = [8., 8., 8., 8., 0., 0., 0., 0.];
        let bright = [0., 0., 0., 0., 8., 8., 8., 8.];
        let mut song = [dark; 80];
        song[40..].fill(bright);
        song[15] = bright;
        assert_eq!(boundaries(&song), [40_000]);
        assert!(boundaries(&[dark; 80]).is_empty());
        assert!(boundaries(&[dark; 3]).is_empty());
    }

    #[test]
    fn fft_features_follow_source_time_across_rates_and_silence() {
        for rate in [16_000, 48_000] {
            let mut history = PhraseSpectrum::new(rate);
            let tone = |hz: usize| {
                let mut bins = vec![Complex::default(); CHROMA_FFT_SIZE];
                bins[hz * CHROMA_FFT_SIZE / rate as usize].re = 1000.;
                bins
            };
            let low = tone(120);
            let high = tone(1900);
            while history.next_center < u64::from(rate) * 60 {
                let second = history.next_center / u64::from(rate);
                history.push(match second {
                    0..12 => None,
                    12..36 => Some(&low),
                    _ => Some(&high),
                });
            }
            assert_eq!(history.boundaries(), [12_000, 36_000]);
        }
    }
}
