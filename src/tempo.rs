//! Bounded offline tempo suggestion. Not a beat/downbeat detector or sync engine.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TempoEstimate {
    pub bpm: f64,
}

/// Analyze at most the first 60 seconds, with at least 8 seconds required.
/// Squared channel samples preserve opposite-phase stereo. A smoothed onset
/// envelope is correlated over 40..240 BPM; octave ambiguity remains possible.
pub fn estimate(samples: &[f32], channels: u16, rate: u32) -> Option<TempoEstimate> {
    let channels = usize::from(channels);
    if channels == 0 || rate < 8000 || !samples.len().is_multiple_of(channels) {
        return None;
    }
    let frames = (samples.len() / channels).min(rate as usize * 60);
    if frames < rate as usize * 8 {
        return None;
    }
    let hop = (rate as usize / 200).max(1);
    let envelope_rate = f64::from(rate) / hop as f64;
    let mut smooth_ring = [0.0_f64; 10];
    let mut history_ring = [0.0_f64; 10];
    let (mut smooth_sum, mut history_sum, mut energy_sum, mut novelty_sum) = (0.0, 0.0, 0.0, 0.0);
    let mut novelty = Vec::with_capacity(frames.div_ceil(hop));
    for (index, chunk) in samples[..frames * channels]
        .chunks(hop * channels)
        .enumerate()
    {
        let power = chunk.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / chunk.len() as f64;
        if !power.is_finite() {
            return None;
        }
        let slot = index % 10;
        smooth_sum += power - smooth_ring[slot];
        smooth_ring[slot] = power;
        let smoothed = smooth_sum / 10.0;
        let onset = if index < 20 {
            0.0
        } else {
            (smoothed - history_sum / 10.0).max(0.0)
        };
        history_sum += smoothed - history_ring[slot];
        history_ring[slot] = smoothed;
        energy_sum += smoothed;
        novelty_sum += onset;
        novelty.push(onset);
    }
    // Reject steady/weakly modulated audio rather than inventing a tempo for it.
    if energy_sum <= f64::EPSILON || novelty_sum / energy_sum < 0.07 {
        return None;
    }
    let min_lag = (envelope_rate * 60.0 / 240.0).floor() as usize;
    let max_lag = (envelope_rate * 60.0 / 40.0).ceil() as usize;
    let scores: Vec<_> = (min_lag.saturating_sub(1)..=max_lag + 1)
        .map(|lag| {
            let (mut dot, mut a, mut b) = (0.0, 0.0, 0.0);
            for index in lag..novelty.len() {
                let left = novelty[index];
                let right = novelty[index - lag];
                dot += left * right;
                a += left * left;
                b += right * right;
            }
            if a > 0.0 && b > 0.0 {
                (dot / (a * b).sqrt()).clamp(0.0, 1.0)
            } else {
                0.0
            }
        })
        .collect();
    let best = scores[1..scores.len() - 1]
        .iter()
        .copied()
        .fold(0.0_f64, f64::max);
    if best < 0.4 {
        return None;
    }
    // Prefer the first strong local maximum, avoiding slower integer multiples
    // of a pulse period when their correlation scores are essentially tied.
    let selected = (1..scores.len() - 1).find(|i| {
        scores[*i] >= best * 0.92 && scores[*i] >= scores[*i - 1] && scores[*i] >= scores[*i + 1]
    })?;
    let denominator = scores[selected - 1] - 2.0 * scores[selected] + scores[selected + 1];
    let delta = if denominator.abs() > 1e-12 {
        (0.5 * (scores[selected - 1] - scores[selected + 1]) / denominator).clamp(-0.5, 0.5)
    } else {
        0.0
    };
    let lag = (min_lag - 1 + selected) as f64 + delta;
    Some(TempoEstimate {
        bpm: (envelope_rate * 60.0 / lag).clamp(40.0, 240.0),
    })
}

#[cfg(test)]
pub(crate) fn pulse_train(bpm: f64, rate: u32, seconds: usize) -> Vec<f32> {
    (0..rate as usize * seconds)
        .map(|frame| {
            let within = (frame as f64 / f64::from(rate)).rem_euclid(60.0 / bpm);
            if within < 0.02 {
                (0.7 * (1.0 - within / 0.02)) as f32
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimates_known_pulse_rates_without_half_or_double_tempo_on_these_fixtures() {
        let start = std::time::Instant::now();
        for rate in [8000, 44_100, 48_000] {
            for bpm in [40.0, 60.0, 90.0, 120.0, 128.5, 180.0, 240.0] {
                let result =
                    estimate(&pulse_train(bpm, rate, 12), 1, rate).expect("periodic pulses");
                assert!(
                    (result.bpm - bpm).abs() < 1.0,
                    "expected {bpm} at {rate}, got {result:?}"
                );
            }
        }
        eprintln!("21 synthetic tempo estimates: {:?}", start.elapsed());
    }
    #[test]
    fn silence_short_audio_steady_tones_and_invalid_input_have_no_suggestion() {
        assert!(estimate(&[], 0, 48_000).is_none());
        assert!(estimate(&[0.0], 2, 48_000).is_none());
        assert!(estimate(&[0.0; 10], 1, 0).is_none());
        assert!(estimate(&vec![f32::NAN; 8000 * 8], 1, 8000).is_none());
        assert!(estimate(&pulse_train(120.0, 8000, 3), 1, 8000).is_none());
        assert!(estimate(&vec![0.0; 8000 * 10], 1, 8000).is_none());
        assert!(estimate(&vec![0.25; 8000 * 10], 1, 8000).is_none());
        for frequency in [37.0, 80.0, 440.0, 1000.0] {
            let samples: Vec<_> = (0..8000 * 10)
                .map(|i| (std::f32::consts::TAU * frequency * i as f32 / 8000.0).sin() * 0.5)
                .collect();
            assert!(
                estimate(&samples, 1, 8000).is_none(),
                "steady {frequency} Hz"
            );
        }
        let mut seed = 42_u32;
        let noise: Vec<_> = (0..8000 * 10)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                (f64::from(seed) / f64::from(u32::MAX) - 0.5) as f32
            })
            .collect();
        assert!(estimate(&noise, 1, 8000).is_none());
    }
    #[test]
    fn stereo_phase_and_amplitude_do_not_change_tempo_and_work_is_capped() {
        let mono = pulse_train(120.0, 8000, 60);
        let stereo: Vec<_> = mono.iter().flat_map(|v| [*v * 0.2, -*v * 0.2]).collect();
        let a = estimate(&mono, 1, 8000).unwrap();
        let b = estimate(&stereo, 2, 8000).unwrap();
        assert!((a.bpm - b.bpm).abs() < 0.001);
        let mut longer = mono.clone();
        longer.extend(pulse_train(90.0, 8000, 15));
        assert_eq!(estimate(&longer, 1, 8000), Some(a));
    }
}
