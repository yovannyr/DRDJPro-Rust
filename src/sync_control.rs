//! Bounded control-plane rate correction. No seeking or permanent pitch mutation.
pub fn step(
    current: f64,
    beat_error: f64,
    nominal: f64,
    base: f64,
    elapsed_frames: u64,
    output_rate: u32,
    alignment: crate::phase_sync::Alignment,
) -> Result<f64, String> {
    if !current.is_finite()
        || !(0.98..=1.02).contains(&current)
        || !beat_error.is_finite()
        || !(-alignment.half_period_beats()..=alignment.half_period_beats()).contains(&beat_error)
        || !nominal.is_finite()
        || !base.is_finite()
        || nominal <= 0.
        || base <= 0.
        || output_rate == 0
    {
        return Err("Invalid sync correction input".into());
    }
    let seconds = elapsed_frames as f64 / f64::from(output_rate);
    if seconds > 0.1 {
        return Err("Sync control interval exceeded 100 ms".into());
    }
    let desired = (nominal / base * (1. + beat_error * 0.1)).clamp(0.98, 1.02);
    let limit = seconds * 0.02;
    Ok((current + (desired - current).clamp(-limit, limit)).clamp(0.98, 1.02))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correction_direction_slew_limits_and_invalid_inputs() {
        assert_eq!(
            step(
                1.,
                0.5,
                1.,
                1.,
                0,
                48000,
                crate::phase_sync::Alignment::Beat
            )
            .unwrap(),
            1.
        );
        assert!(
            (step(
                1.,
                0.5,
                1.,
                1.,
                2400,
                48000,
                crate::phase_sync::Alignment::Beat
            )
            .unwrap()
                - 1.001)
                .abs()
                < 1e-12
        );
        assert!(
            (step(
                1.,
                -0.5,
                1.,
                1.,
                2400,
                48000,
                crate::phase_sync::Alignment::Beat
            )
            .unwrap()
                - 0.999)
                .abs()
                < 1e-12
        );
        assert_eq!(
            step(
                1.02,
                0.5,
                1.,
                1.,
                4800,
                48000,
                crate::phase_sync::Alignment::Beat
            )
            .unwrap(),
            1.02
        );
        assert!(step(
            1.,
            0.,
            1.,
            1.,
            4801,
            48000,
            crate::phase_sync::Alignment::Beat
        )
        .is_err());
        assert!(step(
            1.,
            f64::NAN,
            1.,
            1.,
            1,
            48000,
            crate::phase_sync::Alignment::Beat
        )
        .is_err());
        assert!(step(
            1.03,
            0.,
            1.,
            1.,
            1,
            48000,
            crate::phase_sync::Alignment::Beat
        )
        .is_err());
        assert!(step(1., 0., 1., 0., 1, 48000, crate::phase_sync::Alignment::Beat).is_err());
        let mut error = 0.2;
        let mut correction = 1.;
        for _ in 0..3000 {
            correction = step(
                correction,
                error,
                1.,
                1.,
                480,
                48000,
                crate::phase_sync::Alignment::Beat,
            )
            .unwrap();
            error -= (correction - 1.) * 2. * 0.01;
            assert!((0.98..=1.02).contains(&correction));
        }
        assert!(error.abs() < 0.002);
    }

    #[test]
    fn bar4_regression_and_synthetic_feedback() {
        // error1.0 rejected Beat but accepted Bar4, desired correction positive with .02/s slew
        assert!(step(
            1.,
            1.0,
            1.,
            1.,
            2400,
            48000,
            crate::phase_sync::Alignment::Beat
        )
        .is_err());
        let res = step(
            1.,
            1.0,
            1.,
            1.,
            2400,
            48000,
            crate::phase_sync::Alignment::Bar4,
        )
        .unwrap();
        assert!(res > 1.0);
        assert!(res <= 1.02);

        // error2.001 rejected Bar4
        assert!(step(
            1.,
            2.001,
            1.,
            1.,
            2400,
            48000,
            crate::phase_sync::Alignment::Bar4
        )
        .is_err());

        // Synthetic120BPM bar feedback starting1 whole beat offset converges <.002beats over9000 steps of480frames at48000Hz
        let mut error = 1.0;
        let mut correction = 1.0;
        for _ in 0..9000 {
            correction = step(
                correction,
                error,
                1.,
                1.,
                480,
                48000,
                crate::phase_sync::Alignment::Bar4,
            )
            .unwrap();
            error -= (correction - 1.) * 2. * 0.01;
            assert!((0.98..=1.02).contains(&correction));
        }
        assert!(error.abs() < 0.002);
    }
}
