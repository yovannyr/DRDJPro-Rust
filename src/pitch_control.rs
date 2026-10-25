//! GUI pitch ranges and integer 0.01 percentage-point control steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PitchRange {
    index: usize,
}
impl Default for PitchRange {
    fn default() -> Self {
        Self { index: 3 }
    }
}
impl PitchRange {
    pub fn from_index(index: i32) -> Result<Self, String> {
        match index {
            0..=3 => Ok(Self {
                index: index as usize,
            }),
            _ => Err("Choose a supported pitch range".into()),
        }
    }
    pub fn index(self) -> i32 {
        self.index as i32
    }
    pub fn bounds(self) -> (f64, f64) {
        [(-8., 8.), (-16., 16.), (-50., 50.), (-50., 100.)][self.index]
    }
    pub fn contains(self, factor: f64) -> bool {
        let (min, max) = self.bounds();
        factor.is_finite() && factor >= 1. + min / 100. && factor <= 1. + max / 100.
    }
    pub fn factor(self, percent: f64) -> Result<f64, String> {
        if !self.contains(1. + percent / 100.) {
            return Err("Pitch is outside the selected range".into());
        }
        Ok(1. + (percent * 100.).round() / 10000.)
    }
    pub fn step(self, factor: f64, up: bool) -> Result<f64, String> {
        if !self.contains(factor) {
            return Err("Widen the pitch range or reset before fine adjustment".into());
        }
        let units = ((factor - 1.) * 10000.).round() as i32 + if up { 1 } else { -1 };
        let target = 1. + f64::from(units) / 10000.;
        if self.contains(target) {
            Ok(target)
        } else {
            Err("Pitch range limit reached".into())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn range_edges_and_integer_fine_steps_are_stable() {
        for (index, min, max) in [
            (0, -8., 8.),
            (1, -16., 16.),
            (2, -50., 50.),
            (3, -50., 100.),
        ] {
            let range = PitchRange::from_index(index).unwrap();
            assert_eq!(range.bounds(), (min, max));
            assert!(range.factor(min).is_ok());
            assert!(range.factor(max).is_ok());
            assert!(range.factor(min - 0.01).is_err());
            assert!(range.factor(max + 0.01).is_err());
            let mut factor = 1.;
            for _ in 0..100 {
                factor = range.step(factor, true).unwrap();
            }
            assert_eq!(factor, 1.01);
            for _ in 0..100 {
                factor = range.step(factor, false).unwrap();
            }
            assert_eq!(factor, 1.);
            assert!(range.step(range.factor(max).unwrap(), true).is_err());
            assert!(range.step(range.factor(min).unwrap(), false).is_err());
        }
        assert!(PitchRange::from_index(-1).is_err());
        assert!(PitchRange::from_index(4).is_err());
        assert!(PitchRange::default().factor(f64::NAN).is_err());
        assert!(PitchRange::default().factor(f64::INFINITY).is_err());
        assert_eq!(PitchRange::default().factor(3.14678).unwrap(), 1.0315);
    }
}
