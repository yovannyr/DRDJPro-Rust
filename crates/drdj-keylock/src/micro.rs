//! Bounded WSOLA correction around a worker-prepared tempo (at most +/-2%).
//! All state is inline. Search occurs once per 20-ms hop and shares anchors across channels.
pub struct MicroStretch {
    hop: u32,
    radius: i32,
    age: u32,
    old: f64,
    current: f64,
    active: bool,
    neutral_hops: u8,
}
impl MicroStretch {
    pub fn new(output_rate: u32) -> Self {
        Self {
            hop: (output_rate / 50).max(2),
            radius: (output_rate / 50).clamp(2, 3840) as i32,
            age: 0,
            old: 0.,
            current: 0.,
            active: false,
            neutral_hops: 0,
        }
    }
    pub fn reset(&mut self) {
        self.active = false;
        self.age = 0;
        self.neutral_hops = 0;
    }
    pub fn frame(
        &mut self,
        position: f64,
        neutral_step: f64,
        corrected: bool,
        channels: u16,
        sample: impl Fn(f64, u16) -> f32,
    ) -> [f32; 2] {
        let channels = channels.min(2);
        if !self.active && !corrected {
            return std::array::from_fn(|c| {
                if c < usize::from(channels) {
                    sample(position, c as u16)
                } else {
                    0.
                }
            });
        }
        if !self.active {
            self.old = position - f64::from(self.hop) * neutral_step;
            self.current = position;
            self.age = 0;
            self.active = true;
        }
        if self.age >= self.hop {
            self.old = self.current;
            self.current = if corrected {
                self.neutral_hops = 0;
                self.align(position, neutral_step, channels, &sample)
            } else {
                self.neutral_hops += 1;
                position
            };
            self.age = 0;
            if self.neutral_hops >= 2 {
                self.reset();
                return std::array::from_fn(|c| {
                    if c < usize::from(channels) {
                        sample(position, c as u16)
                    } else {
                        0.
                    }
                });
            }
        }
        let old = self.old + f64::from(self.hop + self.age) * neutral_step;
        let new = self.current + f64::from(self.age) * neutral_step;
        let weight = self.age as f32 / self.hop as f32;
        let result = std::array::from_fn(|c| {
            if c < usize::from(channels) {
                sample(old, c as u16) * (1. - weight) + sample(new, c as u16) * weight
            } else {
                0.
            }
        });
        self.age += 1;
        result
    }
    fn align(
        &self,
        position: f64,
        step: f64,
        channels: u16,
        sample: &impl Fn(f64, u16) -> f32,
    ) -> f64 {
        let mut reference = [[0.; 2]; 16];
        let mut energy = 0_f64;
        for (i, pair) in reference.iter_mut().enumerate() {
            let frame =
                self.old + (f64::from(self.hop) + i as f64 * f64::from(self.hop) / 16.) * step;
            for c in 0..channels {
                pair[usize::from(c)] = sample(frame, c);
                energy += f64::from(pair[usize::from(c)]).powi(2);
            }
        }
        if energy < 1e-12 {
            return position;
        }
        let score = |delta: i32| {
            let mut error = energy * 0.0001 * (f64::from(delta) / f64::from(self.radius)).powi(2);
            for (i, pair) in reference.iter().enumerate() {
                let frame =
                    position + (f64::from(delta) + i as f64 * f64::from(self.hop) / 16.) * step;
                for c in 0..channels {
                    let d = f64::from(pair[usize::from(c)]) - f64::from(sample(frame, c));
                    error += d * d;
                }
            }
            error
        };
        let mut best = 0;
        let mut error = score(0);
        for delta in (-self.radius..=self.radius).step_by(8) {
            let candidate = score(delta);
            if candidate < error {
                best = delta;
                error = candidate;
            }
        }
        let coarse = best;
        for delta in (coarse - 7).max(-self.radius)..=(coarse + 7).min(self.radius) {
            let candidate = score(delta);
            if candidate < error {
                best = delta;
                error = candidate;
            }
        }
        position + f64::from(best) * step
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn varying_sync_correction_retains_pitch_and_stereo() {
        let rate = 48000;
        let mut stretch = MicroStretch::new(rate);
        let mut position = f64::from(rate);
        let sample = |frame: f64, c: u16| {
            let value = (std::f64::consts::TAU * 440. * frame / f64::from(rate)).sin() as f32 * 0.2;
            if c == 0 {
                value
            } else {
                -value
            }
        };
        let mut crossings = 0;
        let mut previous = 0.;
        for frame in 0..rate * 3 {
            let factor = [1.02, 0.98, 1.005, 0.995, 1.][(frame / 1584) as usize % 5];
            let [a, b] = stretch.frame(position, 1., factor != 1., 2, sample);
            assert!(a.is_finite() && (a + b).abs() < 1e-6);
            if frame >= rate {
                crossings += usize::from(previous <= 0. && a > 0.);
            }
            previous = a;
            position += factor;
        }
        let hz = crossings as f64 / 2.;
        assert!((hz - 440.).abs() < 2., "Dynamic correction pitch: {hz}");
    }
    #[test]
    fn small_dynamic_corrections_preserve_tone_stereo_and_return_to_exact_neutral() {
        for rate in [44100, 48000, 96000] {
            for hz in [50., 110., 440., 1000.] {
                for ratio in [0.98, 1.02] {
                    let mut stretch = MicroStretch::new(rate);
                    let sample = |frame: f64, c: u16| {
                        let x = (std::f64::consts::TAU * hz * frame / f64::from(rate)).sin() as f32
                            * 0.2;
                        if c == 0 {
                            x
                        } else {
                            -x
                        }
                    };
                    let mut position = f64::from(rate);
                    let mut crossings = 0;
                    let mut previous = 0.;
                    let mut power = 0_f64;
                    for i in 0..rate * 3 {
                        let [a, b] = stretch.frame(position, 1., true, 2, sample);
                        assert!((a + b).abs() < 1e-6);
                        if i >= rate {
                            crossings += usize::from(previous <= 0. && a > 0.);
                            power += f64::from(a * a);
                        }
                        previous = a;
                        position += ratio;
                    }
                    let measured = crossings as f64 / 2.;
                    assert!(
                        (measured - hz).abs() < 2.,
                        "rate={rate} tone={hz} ratio={ratio} measured={measured}"
                    );
                    let rms = (power / f64::from(rate * 2)).sqrt();
                    assert!(rms > 0.1 && rms < 0.16, "rms={rms}");
                    for _ in 0..rate / 10 {
                        stretch.frame(position, 1., false, 2, sample);
                        position += 1.;
                    }
                    assert_eq!(
                        stretch.frame(position, 1., false, 2, sample),
                        [sample(position, 0), sample(position, 1)]
                    );
                }
            }
        }
    }
}
