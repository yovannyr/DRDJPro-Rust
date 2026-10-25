//! Validated coupled speed/pitch setting for a fixed-format source.
use std::num::NonZeroU32;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaybackSpeed {
    requested: f64,
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
            requested,
            base,
            output: NonZeroU32::new(rate as u32).unwrap(),
        })
    }
    pub fn requested(self) -> f64 {
        self.requested
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
    fn speed_rate_rounding_and_invalid_values_are_explicit() {
        for base in [44100, 48000, 96000] {
            for requested in [0.5, 0.92, 1., 1.08, 1.333333, 2.] {
                let speed = PlaybackSpeed::new(requested, base).unwrap();
                assert_eq!(speed.requested(), requested);
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
