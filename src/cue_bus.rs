//! Pre-fader stereo cue carried in logical channels 3/4 of the fixed-format bus.
//! The owning engine retains controls across renderer retirement.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

pub struct DeckMixControl(AtomicU64, AtomicBool);
impl DeckMixControl {
    pub fn new() -> Arc<Self> {
        Arc::new(Self(
            AtomicU64::new(u64::from(1f32.to_bits())),
            AtomicBool::new(false),
        ))
    }
    #[cfg(test)]
    pub fn gain(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Acquire) as u32)
    }
    pub fn set_gain(&self, gain: f32) {
        // One control-plane writer; renderer only reads one coherent frame snapshot.
        let state = self.0.load(Ordering::Acquire);
        self.0.store(
            (state & (1 << 32)) | u64::from(gain.to_bits()),
            Ordering::Release,
        );
    }
    pub fn set_cue(&self, enabled: bool) {
        let state = self.0.load(Ordering::Acquire);
        self.0.store(
            (state & 0xffff_ffff) | (u64::from(enabled) << 32),
            Ordering::Release,
        );
    }
    pub fn cue(&self) -> bool {
        self.0.load(Ordering::Acquire) >> 32 != 0
    }
    pub fn rendered_cue(&self) -> bool {
        self.1.load(Ordering::Acquire)
    }
}
pub struct CueSend<S> {
    source: S,
    control: Arc<DeckMixControl>,
    channel: u16,
    stereo: [f32; 2],
    gain: f32,
    cue: bool,
}
impl<S: Source> CueSend<S> {
    pub fn new(source: S, control: Arc<DeckMixControl>) -> Self {
        Self {
            source,
            control,
            channel: 0,
            stereo: [0.; 2],
            gain: 1.,
            cue: false,
        }
    }
}
impl<S: Source> Iterator for CueSend<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        let sample = self.source.next()?;
        if self.channel == 0 {
            let state = self.control.0.load(Ordering::Acquire);
            self.gain = f32::from_bits(state as u32);
            self.cue = state >> 32 != 0;
            // Publish what this actual source frame contains, not a later UI request.
            self.control.1.store(self.cue, Ordering::Release);
        }
        let output = match self.channel {
            0..=1 => {
                self.stereo[usize::from(self.channel)] = sample;
                sample * self.gain
            }
            2..=3 if self.source.channels() >= 4 => {
                if self.cue {
                    self.stereo[usize::from(self.channel - 2)]
                } else {
                    sample * self.gain
                }
            }
            _ => sample * self.gain,
        };
        self.channel = (self.channel + 1) % self.source.channels();
        Some(output)
    }
}
impl<S: Source> Source for CueSend<S> {
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
    struct StereoFrame(u16);
    impl Iterator for StereoFrame {
        type Item = f32;
        fn next(&mut self) -> Option<f32> {
            let sample = [1., -0.5, 0., 0.][usize::from(self.0)];
            self.0 = (self.0 + 1) % 4;
            Some(sample)
        }
    }
    impl Source for StereoFrame {
        fn channels(&self) -> u16 {
            4
        }
        fn sample_rate(&self) -> u32 {
            48000
        }
        fn current_span_len(&self) -> Option<usize> {
            None
        }
        fn total_duration(&self) -> Option<Duration> {
            None
        }
    }
    #[test]
    fn cue_frame_snapshot_preserves_stereo_and_has_no_sample_or_drop_heap_operations() {
        let control = DeckMixControl::new();
        control.set_gain(0.25);
        control.set_cue(true);
        let mut source = CueSend::new(StereoFrame(0), Arc::clone(&control));
        assert_eq!(source.next(), Some(0.25));
        control.set_gain(0.);
        control.set_cue(false);
        assert_eq!(source.next(), Some(-0.125));
        assert_eq!(source.next(), Some(1.));
        assert_eq!(source.next(), Some(-0.5));
        for _ in 0..4 {
            assert_eq!(source.next(), Some(0.));
        }
        control.set_cue(true);
        let stats = crate::looping::measured_heap(|| {
            for _ in 0..20000 {
                std::hint::black_box(source.next());
            }
            drop(source);
        });
        assert_eq!(stats.allocations, 0);
        assert_eq!(stats.reallocations, 0);
        assert_eq!(stats.deallocations, 0);
    }
}
