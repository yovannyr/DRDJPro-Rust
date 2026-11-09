//! Shared channel EQ parameters; coefficient design belongs to the DSP adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EqSettings {
    pub trim: f64,
    pub low: f64,
    pub mid: f64,
    pub high: f64,
    pub filter: f64,
}
impl EqSettings {
    pub fn valid(self) -> bool {
        self.trim.is_finite()
            && (-12. ..=12.).contains(&self.trim)
            && self.filter.is_finite()
            && (-1. ..=1.).contains(&self.filter)
            && [self.low, self.mid, self.high]
                .iter()
                .all(|v| v.is_finite() && (-24. ..=12.).contains(v))
    }
}
