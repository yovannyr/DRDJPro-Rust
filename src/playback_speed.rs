//! Validated coupled speed/pitch setting for a fixed-format source.
use std::num::NonZeroU32;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackSpeed {
    base: NonZeroU32,
    output: NonZeroU32,
}
impl PlaybackSpeed {
    pub fn new(requested: f64, base: u32) -> Result<Self, String> {
        if !requested.is_finite() || !(0.5..=2.0).contains(&requested) {
            return Err("Playback speed must be finite and between 0.5 and 2.0".into());
        }
        let base = NonZeroU32::new(base).ok_or("Invalid source sample rate")?;
        let rate = (f64::from(base.get()) * requested).round();
        if rate < 1. || rate > f64::from(u32::MAX) {
            return Err("Playback sample rate is outside the supported range".into());
        }
        Ok(Self {
            base,
            output: NonZeroU32::new(rate as u32).unwrap(),
        })
    }
    pub fn nudged(base: f64, direction: i32, rate: u32) -> Result<Self, String> {
        if !(-1..=1).contains(&direction) {
            return Err("Nudge direction must be -1, 0 or 1".into());
        }
        Self::new(base, rate)?;
        Self::new(base * (1. + f64::from(direction) * 0.02), rate)
    }
    pub fn output_rate(self) -> NonZeroU32 {
        self.output
    }
    pub fn effective_factor(self) -> f64 {
        f64::from(self.output.get()) / f64::from(self.base.get())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nudge_combines_with_base_and_rejects_limits_before_rounding() {
        for rate in [44100, 48000, 96000] {
            for base in [0.6, 1., 1.8] {
                for direction in [-1, 0, 1] {
                    let speed = PlaybackSpeed::nudged(base, direction, rate).unwrap();
                    let expected = base * (1. + f64::from(direction) * 0.02) * f64::from(rate);
                    assert!((f64::from(speed.output_rate().get()) - expected).abs() <= 0.5);
                }
            }
        }
        assert!(PlaybackSpeed::nudged(0.5, -1, 48000).is_err());
        assert!(PlaybackSpeed::nudged(2., 1, 48000).is_err());
        assert!(PlaybackSpeed::nudged(1., 2, 48000).is_err());
        assert!(PlaybackSpeed::nudged(f64::NAN, 0, 48000).is_err());
        assert!(PlaybackSpeed::nudged(1., 1, u32::MAX).is_err());
    }
    #[test]
    fn speed_rate_rounding_and_invalid_values_are_explicit() {
        for base in [44100, 48000, 96000] {
            for requested in [0.5, 0.92, 1., 1.08, 1.333333, 2.] {
                let speed = PlaybackSpeed::new(requested, base).unwrap();
                assert!(
                    (f64::from(speed.output_rate().get()) - f64::from(base) * requested).abs()
                        <= 0.5
                );
                assert_eq!(
                    speed.effective_factor(),
                    f64::from(speed.output_rate().get()) / f64::from(base)
                );
            }
        }
        for value in [f64::NAN, f64::INFINITY, -1., 0., 0.49999, 2.00001] {
            assert!(PlaybackSpeed::new(value, 48000).is_err());
        }
        assert!(PlaybackSpeed::new(1., 0).is_err());
        assert!(PlaybackSpeed::new(2., u32::MAX).is_err());
    }
}
