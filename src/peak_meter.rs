//! Latest complete 50ms producer sample peak, across all channels.
//! No hardware/true-peak/freshness claim; stopped renderers retain their last window.
use std::sync::atomic::{AtomicU64, Ordering};

pub(crate) struct PeakAccumulator {
    peak: f32,
    invalid: bool,
    frames: u32,
    window_frames: u32,
    sequence: u32,
}

impl PeakAccumulator {
    pub fn new(rate: u32) -> Self {
        assert!(rate > 0);
        let window_frames = rate.div_ceil(20);
        PeakAccumulator {
            peak: 0.0,
            invalid: false,
            frames: 0,
            window_frames,
            sequence: 0,
        }
    }

    pub fn sample(&mut self, sample: f32) {
        if !sample.is_finite() {
            self.invalid = true;
        } else {
            let abs = sample.abs();
            if abs > self.peak {
                self.peak = abs;
            }
        }
    }

    pub fn complete_frame(&mut self, published: &AtomicU64) {
        self.frames += 1;
        if self.frames >= self.window_frames {
            self.sequence = self.sequence.wrapping_add(1).max(1);

            let peak_bits = if self.invalid {
                f32::NAN.to_bits()
            } else {
                self.peak.to_bits()
            };

            let packed = ((self.sequence as u64) << 32) | (peak_bits as u64);
            published.store(packed, Ordering::Release);

            // Reset state for next window
            self.peak = 0.0;
            self.invalid = false;
            self.frames = 0;
        }
    }

    pub(crate) fn read_peak(published: &AtomicU64) -> Option<f32> {
        let val = published.load(Ordering::Acquire);
        let seq = (val >> 32) as u32;
        if seq == 0 {
            return None;
        }
        let bits = val as u32;
        Some(f32::from_bits(bits))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peak_windows_round_up_frames_preserve_overrange_and_reset_invalid_samples() {
        for rate in [1, 44_100, 44_101, 48_000, 96_000] {
            let published = AtomicU64::new(0);
            let mut meter = PeakAccumulator::new(rate);
            let length = rate.div_ceil(20);
            assert_eq!(PeakAccumulator::read_peak(&published), None);
            for frame in 0..length {
                meter.sample(-0.25);
                meter.sample(if frame + 1 == length { -1.5 } else { 0.5 });
                assert_eq!(PeakAccumulator::read_peak(&published), None);
                meter.complete_frame(&published);
            }
            assert_eq!(PeakAccumulator::read_peak(&published), Some(1.5));
            for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                for frame in 0..length {
                    meter.sample(if frame == 0 { invalid } else { 0. });
                    meter.complete_frame(&published);
                }
                assert!(PeakAccumulator::read_peak(&published).unwrap().is_nan());
                for _ in 0..length {
                    meter.sample(0.);
                    meter.complete_frame(&published);
                }
                assert_eq!(PeakAccumulator::read_peak(&published), Some(0.));
            }
            meter.sequence = u32::MAX;
            for _ in 0..length {
                meter.complete_frame(&published);
            }
            assert_eq!(published.load(Ordering::Acquire) >> 32, 1);
            assert_eq!(PeakAccumulator::read_peak(&published), Some(0.));
        }
    }

    #[test]
    fn peak_publication_is_coherent_during_concurrent_reads() {
        let published = std::sync::Arc::new(AtomicU64::new(0));
        let writer = std::sync::Arc::clone(&published);
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer_done = std::sync::Arc::clone(&done);
        let thread = std::thread::spawn(move || {
            let mut meter = PeakAccumulator::new(20);
            for i in 0..20_000 {
                meter.sample(if i % 2 == 0 { -0.25 } else { 2. });
                meter.complete_frame(&writer);
            }
            writer_done.store(true, Ordering::Release);
        });
        while !done.load(Ordering::Acquire) {
            if let Some(value) = PeakAccumulator::read_peak(&published) {
                assert!(value == 0.25 || value == 2.);
            }
        }
        thread.join().unwrap();
        assert_eq!(PeakAccumulator::read_peak(&published), Some(2.));
    }
}
