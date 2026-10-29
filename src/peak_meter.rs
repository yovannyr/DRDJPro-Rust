//! Latest complete 50ms producer sample peak, across all channels.
//! No hardware/true-peak/freshness claim; stopped renderers retain their last window.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Default)]
pub(crate) struct ClipLatch {
    events: AtomicU64,
    cleared: AtomicU64,
}
impl ClipLatch {
    pub fn active(&self) -> bool {
        let baseline = self.cleared.load(Ordering::Acquire);
        self.events.load(Ordering::Acquire) != baseline
    }
    pub fn clear(&self) {
        // Events completed after this observation survive the reset store.
        let observed = self.events.load(Ordering::Acquire);
        self.cleared.store(observed, Ordering::Release);
    }
}
pub(crate) struct ChannelMeter {
    window: AtomicU64,
    clip: Arc<ClipLatch>,
}
impl ChannelMeter {
    pub fn new(clip: Arc<ClipLatch>) -> Arc<Self> {
        Arc::new(Self {
            window: AtomicU64::new(0),
            clip,
        })
    }
    pub fn peak(&self) -> Option<f32> {
        PeakAccumulator::read_peak(&self.window)
    }
}
pub(crate) struct Metered<S> {
    source: S,
    meter: Arc<ChannelMeter>,
    peak: PeakAccumulator,
    channel: u16,
    frame_clip: bool,
}
impl<S: Source> Metered<S> {
    pub fn new(source: S, meter: Arc<ChannelMeter>) -> Self {
        assert!(source.channels() > 0);
        let peak = PeakAccumulator::new(source.sample_rate());
        Self {
            source,
            meter,
            peak,
            channel: 0,
            frame_clip: false,
        }
    }
}
impl<S: Source> Iterator for Metered<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.source.next()?;
        self.peak.sample(sample);
        self.frame_clip |= !sample.is_finite() || sample.abs() >= 1.;
        self.channel += 1;
        if self.channel == self.source.channels() {
            self.channel = 0;
            if self.frame_clip {
                self.meter.clip.events.fetch_add(1, Ordering::Release);
            }
            self.frame_clip = false;
            self.peak.complete_frame(&self.meter.window);
        }
        Some(sample)
    }
}
impl<S: Source> Source for Metered<S> {
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
    fn channel_meter_publishes_complete_frames_and_holds_clip_until_cleared() {
        let clip = Arc::new(ClipLatch::default());
        let state = ChannelMeter::new(Arc::clone(&clip));
        let samples = vec![1.25, 0.25, 0.1, -0.1, 0.2, -0.2];
        let source = rodio::buffer::SamplesBuffer::new(2, 20, samples.clone());
        let mut output = Metered::new(source, Arc::clone(&state));
        assert_eq!(output.next(), Some(1.25));
        assert_eq!(state.peak(), None);
        assert!(!clip.active());
        assert_eq!(output.next(), Some(0.25));
        assert_eq!(state.peak(), Some(1.25));
        assert!(clip.active());
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let flag = Arc::new(ClipLatch::default());
            let state = ChannelMeter::new(Arc::clone(&flag));
            let source = rodio::buffer::SamplesBuffer::new(1, 20, vec![invalid, 0.]);
            let mut output = Metered::new(source, Arc::clone(&state));
            assert_eq!(output.next().unwrap().to_bits(), invalid.to_bits());
            assert!(state.peak().unwrap().is_nan());
            assert!(flag.active());
            flag.clear();
            assert_eq!(output.next(), Some(0.));
            assert_eq!(state.peak(), Some(0.));
            assert!(!flag.active());
        }
        assert_eq!(output.by_ref().take(2).collect::<Vec<_>>(), samples[2..4]);
        assert_eq!(state.peak(), Some(0.1));
        assert!(clip.active());
        clip.clear();
        assert!(!clip.active());
        assert_eq!(output.collect::<Vec<_>>(), samples[4..]);
        assert!(!clip.active());
        // A new completed frame after reset's observation must not be erased by its store.
        let observed = clip.events.load(Ordering::Acquire);
        clip.events.fetch_add(1, Ordering::Release);
        clip.cleared.store(observed, Ordering::Release);
        assert!(clip.active());
    }

    #[test]
    fn channel_meter_sample_and_isolated_render_drop_add_no_heap_operations() {
        let clip = Arc::new(ClipLatch::default());
        let meter = ChannelMeter::new(clip);
        let worker_meter = Arc::clone(&meter);
        let stats = std::thread::spawn(move || {
            let mut source = Metered::new(rodio::source::SineWave::new(1000.), worker_meter);
            crate::looping::measured_heap(|| {
                for _ in 0..10000 {
                    source.next();
                }
                drop(source);
            })
        })
        .join()
        .unwrap();
        assert_eq!(stats.allocations, 0);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.deallocations, 0);
        assert!(meter.peak().is_some());
    }

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
