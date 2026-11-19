//! Offline phrase suggestions from spectral transitions or the stored waveform envelope.
pub use crate::neural_sync_engine::PhraseType;

/// Find sustained energy contrasts on a bar grid. Uncertain or uniform input
/// yields no cues. Results are proposals in source milliseconds.
pub fn detect(
    peaks: &[u8],
    duration_ms: u64,
    bpm: f32,
    anchor_ms: Option<u32>,
) -> Vec<(u64, PhraseType)> {
    detect_with_boundaries(peaks, duration_ms, bpm, anchor_ms, &[])
}

/// Snap measured spectral transitions to the current grid. Older analyses
/// and tracks without spectral evidence retain the amplitude-based detector.
pub fn detect_with_boundaries(
    peaks: &[u8],
    duration_ms: u64,
    bpm: f32,
    anchor_ms: Option<u32>,
    boundaries_ms: &[u32],
) -> Vec<(u64, PhraseType)> {
    if peaks.len() < 32 || duration_ms == 0 || !(40.0..=240.0).contains(&bpm) {
        return Vec::new();
    }
    let bar = 240_000.0 / f64::from(bpm);
    let start = f64::from(anchor_ms.unwrap_or(0));
    let bars = ((duration_ms as f64 - start) / bar).floor() as usize;
    if !(8..=4096).contains(&bars) || bars > peaks.len() {
        return Vec::new();
    }
    let at = |i: usize| start + i as f64 * bar;
    let bin = |i| ((at(i) / duration_ms as f64 * peaks.len() as f64) as usize).min(peaks.len());
    let energy: Vec<f64> = (0..bars)
        .map(|i| {
            let slice = &peaks[bin(i)..bin(i + 1)];
            slice.iter().map(|x| f64::from(*x)).sum::<f64>() / slice.len().max(1) as f64
        })
        .collect();
    let change = |i: usize| {
        let before = (energy[i - 2] + energy[i - 1]) * 0.5;
        let after = (energy[i] + energy[i + 1]) * 0.5;
        (before, after)
    };
    let kind = |i: usize, delta: f64| {
        if delta.abs() < 8. {
            PhraseType::Custom
        } else if delta < 0. && i >= bars * 3 / 4 {
            PhraseType::Outro
        } else if delta < 0. {
            PhraseType::Break
        } else if energy[i + 1] > energy[i] + 8. && energy[i] > energy[i - 1] + 8. {
            PhraseType::Buildup
        } else {
            PhraseType::Chorus
        }
    };
    let mut selected = Vec::new();
    if boundaries_ms.is_empty() {
        let mut candidates = Vec::new();
        for i in 2..bars - 1 {
            let (before, after) = change(i);
            let delta = after - before;
            if delta.abs() >= 18. && delta.abs() >= before.max(after) * 0.18 {
                candidates.push((i, delta));
            }
        }
        candidates.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
        for (i, delta) in candidates {
            if selected
                .iter()
                .all(|(other, _): &(usize, PhraseType)| other.abs_diff(i) >= 3)
            {
                selected.push((i, kind(i, delta)));
                if selected.len() == 7 {
                    break;
                }
            }
        }
    } else {
        for ms in boundaries_ms.iter().take(7) {
            let i = ((f64::from(*ms) - start) / bar).round() as usize;
            if (2..bars - 1).contains(&i) && !selected.iter().any(|v| v.0 == i) {
                let (before, after) = change(i);
                selected.push((i, kind(i, after - before)));
            }
        }
    }
    if selected.is_empty() {
        return Vec::new();
    }
    selected.sort_by_key(|v| v.0);
    if energy[0] < energy.iter().copied().fold(0., f64::max) * 0.8 {
        selected.insert(0, (0, PhraseType::Intro));
    }
    selected
        .into_iter()
        .map(|(i, kind)| (at(i).round() as u64, kind))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timbral_boundaries_follow_the_current_grid_without_inventing_a_label() {
        let result = detect_with_boundaries(&[90; 160], 80_000, 120., Some(500), &[16_000, 49_000]);
        assert_eq!(
            result,
            [(16_500, PhraseType::Custom), (48_500, PhraseType::Custom)]
        );
        assert!(
            detect_with_boundaries(&[90; 160], 80_000, 120., Some(45_000), &[16_000]).is_empty()
        );
    }
    #[test]
    fn bar_aligned_sections_follow_measured_contrasts() {
        let peaks: Vec<_> = [45, 180, 55, 200, 35]
            .into_iter()
            .flat_map(|x| std::iter::repeat_n(x, 32))
            .collect();
        let cues = detect(&peaks, 80_000, 120., None);
        assert_eq!(
            cues,
            [
                (0, PhraseType::Intro),
                (16_000, PhraseType::Chorus),
                (32_000, PhraseType::Break),
                (48_000, PhraseType::Chorus),
                (64_000, PhraseType::Outro)
            ]
        );
        assert!(detect(&[90; 160], 80_000, 120., None).is_empty());
        assert!(detect(&peaks, 80_000, 0., None).is_empty());
    }
}
