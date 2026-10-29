//! Worker/control-plane coefficient design; bounded lock/allocation-free sample processing.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
pub(crate) const MAX_CHANNELS: u16 = 32;
const STAGES: usize = 4;
const COEFFICIENTS: usize = STAGES * 5 + 1;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct EqSettings {
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
    fn coefficients(self, rate: u32) -> [f64; COEFFICIENTS] {
        let mut result = [0.; COEFFICIENTS];
        for (index, (frequency, db)) in [(250_f64, self.low), (1000., self.mid), (4000., self.high)]
            .into_iter()
            .enumerate()
        {
            result[index * 5..index * 5 + 5].copy_from_slice(&biquad(
                index,
                frequency.min(f64::from(rate) * 0.45),
                db,
                rate,
            ));
        }
        result[15..20].copy_from_slice(&filter(self.filter, rate));
        result[20] = 10_f64.powf(self.trim / 20.);
        result
    }
}

// Butterworth LP/HP, logarithmic sweep. Exactly zero is a true bypass.
fn filter(amount: f64, rate: u32) -> [f64; 5] {
    if amount == 0. {
        return [1., 0., 0., 0., 0.];
    }
    let upper = 20000_f64.min(f64::from(rate) * 0.45);
    let lower = 20_f64.min(upper);
    let frequency = if amount < 0. {
        upper * (lower / upper).powf(-amount)
    } else {
        lower * (upper / lower).powf(amount)
    };
    let w = std::f64::consts::TAU * frequency / f64::from(rate);
    let c = w.cos();
    let alpha = w.sin() / std::f64::consts::SQRT_2;
    let (b0, b1) = if amount < 0. {
        ((1. - c) / 2., 1. - c)
    } else {
        ((1. + c) / 2., -(1. + c))
    };
    let a0 = 1. + alpha;
    [b0 / a0, b1 / a0, b0 / a0, -2. * c / a0, (1. - alpha) / a0]
}

// RBJ/W3C Audio EQ Cookbook: low/high shelves S=1, peaking EQ Q=1/sqrt(2).
// https://www.w3.org/TR/audio-eq-cookbook/
fn biquad(kind: usize, frequency: f64, db: f64, rate: u32) -> [f64; 5] {
    if db == 0. {
        return [1., 0., 0., 0., 0.];
    }
    let a = 10_f64.powf(db / 40.);
    let w = std::f64::consts::TAU * frequency / f64::from(rate);
    let c = w.cos();
    let alpha = w.sin() / std::f64::consts::SQRT_2;
    let beta = 2. * a.sqrt() * alpha;
    let (b0, b1, b2, a0, a1, a2) = match kind {
        0 => (
            a * ((a + 1.) - (a - 1.) * c + beta),
            2. * a * ((a - 1.) - (a + 1.) * c),
            a * ((a + 1.) - (a - 1.) * c - beta),
            (a + 1.) + (a - 1.) * c + beta,
            -2. * ((a - 1.) + (a + 1.) * c),
            (a + 1.) + (a - 1.) * c - beta,
        ),
        1 => (
            1. + alpha * a,
            -2. * c,
            1. - alpha * a,
            1. + alpha / a,
            -2. * c,
            1. - alpha / a,
        ),
        _ => (
            a * ((a + 1.) + (a - 1.) * c + beta),
            -2. * a * ((a - 1.) + (a + 1.) * c),
            a * ((a + 1.) + (a - 1.) * c - beta),
            (a + 1.) - (a - 1.) * c + beta,
            2. * ((a - 1.) - (a + 1.) * c),
            (a + 1.) - (a - 1.) * c - beta,
        ),
    };
    [b0 / a0, b1 / a0, b2 / a0, a1 / a0, a2 / a0]
}

pub(crate) struct EqControl {
    sequence: AtomicU64,
    values: [AtomicU64; COEFFICIENTS],
    rate: u32,
}
impl EqControl {
    pub fn new(rate: u32) -> Arc<Self> {
        assert!(rate > 0);
        let values = EqSettings::default().coefficients(rate);
        Arc::new(Self {
            sequence: AtomicU64::new(0),
            values: values.map(|v| AtomicU64::new(v.to_bits())),
            rate,
        })
    }
    // Single control-plane writer, enforced by DualDeckEngine's !Send/!Sync ownership.
    pub fn publish(&self, settings: EqSettings) {
        assert!(settings.valid());
        let values = settings.coefficients(self.rate);
        self.sequence.fetch_add(1, Ordering::SeqCst);
        for (target, value) in self.values.iter().zip(values) {
            target.store(value.to_bits(), Ordering::SeqCst);
        }
        self.sequence.fetch_add(1, Ordering::SeqCst);
    }
    fn read(&self) -> Option<[f64; COEFFICIENTS]> {
        let before = self.sequence.load(Ordering::SeqCst);
        if before & 1 != 0 {
            return None;
        }
        let values = self
            .values
            .each_ref()
            .map(|v| f64::from_bits(v.load(Ordering::SeqCst)));
        (before == self.sequence.load(Ordering::SeqCst)).then_some(values)
    }
}

struct Bank {
    coefficients: [f64; COEFFICIENTS],
    states: [[[f64; 2]; STAGES]; MAX_CHANNELS as usize],
}
impl Bank {
    fn sample(&mut self, channel: usize, input: f32) -> f64 {
        let mut value = f64::from(input);
        for (coefficients, state) in self.coefficients[..STAGES * 5]
            .as_chunks::<5>()
            .0
            .iter()
            .zip(&mut self.states[channel])
        {
            let output = coefficients[0] * value + state[0];
            state[0] = coefficients[1] * value - coefficients[3] * output + state[1];
            state[1] = coefficients[2] * value - coefficients[4] * output;
            for s in state {
                if s.abs() < 1e-30 {
                    *s = 0.;
                }
            }
            value = output;
        }
        value * self.coefficients[STAGES * 5]
    }
}

pub(crate) struct EqSource<S> {
    source: S,
    control: Arc<EqControl>,
    active: Bank,
    next: Bank,
    wanted: [f64; COEFFICIENTS],
    channel: usize,
    channels: usize,
    fade: u32,
    ramp: u32,
}
impl<S: Source> EqSource<S> {
    pub fn new(source: S, control: Arc<EqControl>) -> Self {
        assert!(source.channels() > 0 && source.channels() <= MAX_CHANNELS);
        let coefficients = control
            .read()
            .expect("source installed on the single control writer");
        let channels = usize::from(source.channels());
        let states = [[[0.; 2]; STAGES]; MAX_CHANNELS as usize];
        let next = Bank {
            coefficients,
            states,
        };
        let ramp = source.sample_rate().div_ceil(200).max(1);
        Self {
            source,
            control,
            active: Bank {
                coefficients,
                states,
            },
            next,
            wanted: coefficients,
            channel: 0,
            channels,
            fade: 0,
            ramp,
        }
    }
}
impl<S: Source> Iterator for EqSource<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.source.next()?;
        if self.channel == 0 {
            if let Some(values) = self.control.read() {
                self.wanted = values;
            }
            if self.fade == 0 && self.wanted != self.active.coefficients {
                self.next.coefficients = self.wanted;
                self.next.states.fill([[0.; 2]; STAGES]);
                self.fade = 1;
            }
        }
        let input = if sample.is_finite() { sample } else { 0. };
        let a = self.active.sample(self.channel, input);
        let value = if self.fade == 0 {
            a
        } else {
            let b = self.next.sample(self.channel, input);
            a + (b - a) * f64::from(self.fade) / f64::from(self.ramp)
        };
        self.channel += 1;
        if self.channel == self.channels {
            self.channel = 0;
            if self.fade == self.ramp {
                std::mem::swap(&mut self.active, &mut self.next);
                self.fade = 0;
            } else if self.fade != 0 {
                self.fade += 1;
            }
        }
        Some(if value.is_finite() && value.abs() <= f64::from(f32::MAX) {
            value as f32
        } else {
            0.
        })
    }
}
impl<S: Source> Source for EqSource<S> {
    fn channels(&self) -> u16 {
        self.source.channels()
    }
    fn sample_rate(&self) -> u32 {
        self.source.sample_rate()
    }
    fn current_span_len(&self) -> Option<usize> {
        self.source.current_span_len()
    }
    fn total_duration(&self) -> Option<Duration> {
        self.source.total_duration()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn response(rate: u32, frequency: f64, settings: EqSettings) -> f64 {
        let input: Vec<f32> = (0..rate)
            .map(|i| {
                (std::f64::consts::TAU * frequency * f64::from(i) / f64::from(rate)).sin() as f32
                    * 0.1
            })
            .collect();
        let control = EqControl::new(rate);
        control.publish(settings);
        let mut output = EqSource::new(rodio::buffer::SamplesBuffer::new(1, rate, input), control);
        let sum: f64 = output
            .by_ref()
            .skip(rate as usize / 2)
            .map(|x| f64::from(x).powi(2))
            .sum();
        20. * ((sum / (f64::from(rate) / 2.)).sqrt() / (0.1 / 2_f64.sqrt())).log10()
    }
    #[test]
    fn eq_state_replacement_and_drop_on_render_thread_free_no_buffers() {
        let control = EqControl::new(48000);
        let worker_control = Arc::clone(&control);
        let stats = std::thread::spawn(move || {
            let mut old = EqSource::new(
                rodio::source::SineWave::new(1000.),
                Arc::clone(&worker_control),
            );
            old.next();
            let replacement = EqSource::new(
                rodio::source::SineWave::new(2000.),
                Arc::clone(&worker_control),
            );
            crate::looping::measured_heap(|| {
                let retired = std::mem::replace(&mut old, replacement);
                drop(retired);
                drop(old);
            })
        })
        .join()
        .unwrap();
        assert_eq!(stats.allocations, 0);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.deallocations, 0);
    }
    #[test]
    fn eq_atomic_snapshots_never_mix_control_generations() {
        let control = EqControl::new(48000);
        let a = EqSettings {
            trim: -12.,
            low: 12.,
            mid: -24.,
            high: 12.,
            filter: -0.5,
        };
        let b = EqSettings {
            trim: 12.,
            low: -24.,
            mid: 12.,
            high: -24.,
            filter: 0.5,
        };
        let expected = [
            EqSettings::default().coefficients(48000),
            a.coefficients(48000),
            b.coefficients(48000),
        ];
        let writer = Arc::clone(&control);
        let task = std::thread::spawn(move || {
            for _ in 0..2000 {
                writer.publish(a);
                writer.publish(b);
            }
        });
        for _ in 0..10000 {
            if let Some(values) = control.read() {
                assert!(expected.contains(&values));
            }
        }
        task.join().unwrap();
        control.sequence.store(1, Ordering::SeqCst);
        assert!(
            control.read().is_none(),
            "busy writer never causes a retry loop"
        );
    }
    #[test]
    fn bipolar_filter_response_bypass_and_extremes_are_measured() {
        for rate in [44100, 48000, 96000] {
            let low = EqSettings {
                filter: -0.5,
                ..Default::default()
            };
            let high = EqSettings {
                filter: 0.5,
                ..Default::default()
            };
            assert!(response(rate, 100., low).abs() < 0.1);
            assert!(response(rate, 10000., low) < -40.);
            assert!(response(rate, 40., high) < -40.);
            assert!(response(rate, 5000., high).abs() < 0.1);
            for amount in [-1., 1.] {
                let value = response(
                    rate,
                    1000.,
                    EqSettings {
                        filter: amount,
                        ..Default::default()
                    },
                );
                assert!(value.is_finite() && value < -40.);
            }
        }
        for invalid in [f64::NAN, f64::INFINITY, -1.01, 1.01] {
            assert!(!EqSettings {
                filter: invalid,
                ..Default::default()
            }
            .valid());
        }
        assert_eq!(filter(0., 48000), [1., 0., 0., 0., 0.]);
    }
    #[test]
    fn eq_frequency_response_trim_and_neutral_are_measured() {
        for rate in [44100, 48000, 96000] {
            for db in [-24., 12.] {
                for (frequency, settings) in [
                    (
                        40.,
                        EqSettings {
                            low: db,
                            ..Default::default()
                        },
                    ),
                    (
                        1000.,
                        EqSettings {
                            mid: db,
                            ..Default::default()
                        },
                    ),
                    (
                        16000.,
                        EqSettings {
                            high: db,
                            ..Default::default()
                        },
                    ),
                ] {
                    assert!((response(rate, frequency, settings) - db).abs() < 0.35);
                }
            }
            assert!(
                (response(
                    rate,
                    1000.,
                    EqSettings {
                        trim: 6.,
                        ..Default::default()
                    }
                ) - 6.)
                    .abs()
                    < 0.01
            );
        }
        let samples = vec![0.25, -0.7, 0., 0.1];
        let output = EqSource::new(
            rodio::buffer::SamplesBuffer::new(2, 48000, samples.clone()),
            EqControl::new(48000),
        );
        assert_eq!(output.collect::<Vec<_>>(), samples);
    }
    #[test]
    fn eq_changes_are_frame_coherent_smoothed_and_allocate_nothing() {
        let control = EqControl::new(48000);
        let source = rodio::buffer::SamplesBuffer::new(2, 48000, vec![0.1; 100000]);
        let mut output = EqSource::new(source, Arc::clone(&control));
        assert_eq!(output.next(), Some(0.1));
        control.publish(EqSettings {
            trim: 12.,
            ..Default::default()
        });
        assert_eq!(output.next(), Some(0.1));
        let stats = crate::looping::measured_heap(|| {
            let mut previous = 0.1;
            for frame in 0..10000 {
                if frame % 100 == 0 {
                    control.publish(EqSettings {
                        trim: if frame % 200 == 0 { -12. } else { 12. },
                        low: 12.,
                        mid: -24.,
                        high: 12.,
                        filter: if frame % 200 == 0 { -0.5 } else { 0.5 },
                    });
                }
                let left = output.next().unwrap();
                let right = output.next().unwrap();
                assert_eq!(left, right);
                assert!(left.is_finite() && left.abs() < 10.);
                if frame < 50 {
                    assert!((left - previous).abs() < 0.02);
                }
                previous = left;
            }
        });
        assert_eq!(stats.allocations, 0);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.deallocations, 0);
        control.publish(EqSettings {
            trim: -6.,
            ..Default::default()
        });
        for _ in 0..4000 {
            output.next();
        }
        let expected = 0.1 * 10_f32.powf(-6. / 20.);
        assert!(
            (output.next().unwrap() - expected).abs() < 1e-5,
            "latest pending setting eventually wins"
        );
        assert!(!EqSettings {
            trim: f64::NAN,
            ..Default::default()
        }
        .valid());
        assert!(!EqSettings {
            low: -25.,
            ..Default::default()
        }
        .valid());
    }
}
