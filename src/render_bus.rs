//! Fixed-format deck bus with one output-frame clock. This is a render clock, not a device clock.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

#[derive(Default)]
pub(crate) struct RenderClock {
    pub completed: AtomicU64,
}
pub(crate) struct RenderBus<S> {
    source: S,
    clock: Arc<RenderClock>,
    channels: u16,
    rate: u32,
    channel: u16,
}
impl<S: Source> RenderBus<S> {
    pub fn new(source: S) -> (Self, Arc<RenderClock>) {
        let channels = source.channels();
        let rate = source.sample_rate();
        assert!(channels > 0 && rate > 0);
        let clock = Arc::new(RenderClock::default());
        (
            Self {
                source,
                clock: Arc::clone(&clock),
                channels,
                rate,
                channel: 0,
            },
            clock,
        )
    }
}
impl<S: Source> Iterator for RenderBus<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // MixerSource can admit late sources after returning None; retain the bus at silence.
        let sample = self.source.next().unwrap_or(0.);
        self.channel += 1;
        if self.channel == self.channels {
            self.channel = 0;
            self.clock.completed.fetch_add(1, Ordering::Release);
        }
        Some(sample)
    }
}
impl<S: Source> Source for RenderBus<S> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bus_counts_complete_frames_during_silence_and_admits_late_stereo_sources() {
        let (input, source) = rodio::mixer::mixer(2, 48000);
        let (mut bus, clock) = RenderBus::new(source);
        assert_eq!(bus.next(), Some(0.));
        assert_eq!(clock.completed.load(Ordering::Acquire), 0);
        assert_eq!(bus.next(), Some(0.));
        assert_eq!(clock.completed.load(Ordering::Acquire), 1);
        // Insert mid-frame: Rodio delays admission to its next complete channel boundary.
        assert_eq!(bus.next(), Some(0.));
        input.add(rodio::buffer::SamplesBuffer::new(
            2,
            48000,
            vec![0.25, -0.5, 0.5, -0.75],
        ));
        assert_eq!(bus.next(), Some(0.));
        assert_eq!(bus.next(), Some(0.25));
        assert_eq!(bus.next(), Some(-0.5));
        assert_eq!(clock.completed.load(Ordering::Acquire), 3);
        assert_eq!(bus.next(), Some(0.5));
        assert_eq!(bus.next(), Some(-0.75));
        for _ in 0..8 {
            assert_eq!(bus.next(), Some(0.));
        }
        assert_eq!(clock.completed.load(Ordering::Acquire), 8);
        assert_eq!(
            (bus.channels(), bus.sample_rate(), bus.current_span_len()),
            (2, 48000, None)
        );
    }
    #[test]
    fn warmed_bus_pull_adds_no_allocations_and_counts_frames_at_supported_formats() {
        for (channels, rate) in [(1, 44100), (2, 48000), (4, 96000)] {
            let (input, source) = rodio::mixer::mixer(channels, rate);
            input.add(rodio::buffer::SamplesBuffer::new(
                channels,
                rate,
                vec![0.125; 200000],
            ));
            let (mut bus, clock) = RenderBus::new(source);
            for _ in 0..16 {
                bus.next();
            }
            let allocations = crate::looping::counted_allocations(|| {
                for _ in 0..100000 {
                    assert_eq!(bus.next(), Some(0.125));
                }
            });
            assert_eq!(allocations, 0);
            assert_eq!(
                clock.completed.load(Ordering::Acquire),
                100016 / u64::from(channels)
            );
        }
    }
}
