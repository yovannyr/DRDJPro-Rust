//! Control-plane, one-shot beat-phase alignment; no downbeat or latched sync.
use crate::beatgrid::{BeatGrid, MAX_ANCHOR};
use std::time::Duration;

fn phase_fraction(grid: BeatGrid, position: Duration) -> Result<(i128, i128), String> {
    if position > MAX_ANCHOR {
        return Err("Phase sync supports positions up to seven days".into());
    }
    // Preserve the exact binary BPM fraction before taking the beat remainder.
    let bits = grid.bpm().to_bits();
    let mantissa = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
    let denominator = 60_000_000_000_i128 << (1075 - ((bits >> 52) & 0x7ff));
    let delta = (position.as_nanos() as i128 - grid.anchor().as_nanos() as i128) * mantissa;
    Ok((delta.rem_euclid(denominator), denominator))
}

pub fn target_frame(
    own: BeatGrid,
    position: Duration,
    reference: BeatGrid,
    reference_position: Duration,
    rate: u32,
    frames: usize,
) -> Result<usize, String> {
    if rate == 0 || frames == 0 || position > MAX_ANCHOR {
        return Err("Invalid track coordinates for phase sync".into());
    }
    let (own_phase, own_denominator) = phase_fraction(own, position)?;
    let (reference_phase, reference_denominator) = phase_fraction(reference, reference_position)?;
    // Both divisors are 60e9 times a power of two: exact common-denominator scaling.
    let denominator = own_denominator.max(reference_denominator);
    let mut delta = reference_phase * (denominator / reference_denominator)
        - own_phase * (denominator / own_denominator);
    // Choose displacement in (-half beat, +half beat], so exact ties move later.
    if delta * 2 <= -denominator {
        delta += denominator;
    } else if delta * 2 > denominator {
        delta -= denominator;
    }
    let seconds = position.as_secs_f64() + delta as f64 / denominator as f64 * 60. / own.bpm();
    let frame = (seconds * f64::from(rate)).round();
    if frame < 0. || frame >= frames as f64 {
        return Err("Nearest phase alignment is outside the loaded track".into());
    }
    Ok(frame as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn phase(grid: BeatGrid, position: Duration) -> Result<f64, String> {
        let (remainder, denominator) = phase_fraction(grid, position)?;
        Ok(remainder as f64 / denominator as f64)
    }
    #[test]
    fn exact_half_beat_ties_use_later_even_with_distant_nonzero_anchors() {
        let own = BeatGrid::new(200., Duration::from_millis(59298)).unwrap();
        let reference = BeatGrid::new(200., Duration::ZERO).unwrap();
        let tie = Duration::from_millis(59448);
        for (offset, expected) in [(-1_i64, 2846304), (0, 2860704), (1, 2860704)] {
            let position = Duration::from_nanos((tie.as_nanos() as i64 + offset) as u64);
            assert_eq!(
                target_frame(own, position, reference, Duration::ZERO, 48000, 5760000).unwrap(),
                expected
            );
        }
        for bpm in [40., 123.45, 128.5, 200., 240.] {
            let grid = BeatGrid::new(bpm, MAX_ANCHOR).unwrap();
            assert_eq!(
                target_frame(grid, Duration::ZERO, grid, Duration::ZERO, 96000, 96000).unwrap(),
                0
            );
        }
    }
    #[test]
    fn nearest_phase_handles_anchors_ties_boundaries_and_fractional_bpm() {
        let grid = |bpm, ms| BeatGrid::new(bpm, Duration::from_millis(ms)).unwrap();
        for anchor in [0, 1000] {
            assert_eq!(
                target_frame(
                    grid(120., anchor),
                    Duration::from_millis(100),
                    grid(120., 0),
                    Duration::from_millis(125),
                    48000,
                    192000
                )
                .unwrap(),
                6000
            );
        }
        assert_eq!(
            target_frame(
                grid(120., 0),
                Duration::from_millis(250),
                grid(120., 0),
                Duration::ZERO,
                48000,
                192000
            )
            .unwrap(),
            24000
        );
        assert!(target_frame(
            grid(120., 0),
            Duration::ZERO,
            grid(120., 0),
            Duration::from_millis(450),
            48000,
            192000
        )
        .is_err());
        assert!(target_frame(
            grid(120., 0),
            Duration::from_millis(3900),
            grid(120., 0),
            Duration::from_millis(25),
            48000,
            192000
        )
        .is_err());
        for rate in [44100, 48000, 96000] {
            let own = grid(123.45, 1250);
            let reference = grid(128.5, 500);
            let refpos = Duration::from_millis(3456);
            let frame = target_frame(
                own,
                Duration::from_secs(10000),
                reference,
                refpos,
                rate,
                rate as usize * 20000,
            )
            .unwrap();
            let aligned = Duration::from_secs_f64(frame as f64 / f64::from(rate));
            let error = phase(own, aligned).unwrap() - phase(reference, refpos).unwrap();
            assert!(error.abs() <= own.bpm() / 60. / f64::from(rate) / 2. + 1e-8);
        }
        assert!(phase(grid(120., 0), MAX_ANCHOR + Duration::from_secs(1)).is_err());
        assert!(target_frame(
            grid(120., 0),
            Duration::ZERO,
            grid(120., 0),
            Duration::ZERO,
            0,
            1
        )
        .is_err());
    }
}
