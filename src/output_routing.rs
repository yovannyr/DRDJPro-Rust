//! Frame-boundary selection of a stereo output pair; no callback allocation or locks.
use rodio::Source;
use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        Arc,
    },
    time::Duration,
};

pub struct RoutingControl {
    channels: u16,
    requested: AtomicU16,
}
impl RoutingControl {
    pub fn pair(&self) -> u16 {
        self.requested.load(Ordering::Acquire)
    }
    pub fn channels(&self) -> u16 {
        self.channels
    }
    pub fn set_pair(&self, pair: u16) -> Result<(), String> {
        if pair != 0 && u32::from(pair) * 2 + 1 >= u32::from(self.channels) {
            return Err("Output pair is unavailable on the current audio device".into());
        }
        self.requested.store(pair, Ordering::Release);
        Ok(())
    }
}
pub struct PairRouter<S> {
    source: S,
    control: Arc<RoutingControl>,
    channel: u16,
    pair: u16,
    stereo: [f32; 2],
}
impl<S: Source> PairRouter<S> {
    pub fn new(source: S) -> (Self, Arc<RoutingControl>) {
        assert!(source.channels() > 0);
        let control = Arc::new(RoutingControl {
            channels: source.channels(),
            requested: AtomicU16::new(0),
        });
        (
            Self {
                source,
                control: Arc::clone(&control),
                channel: 0,
                pair: 0,
                stereo: [0.; 2],
            },
            control,
        )
    }
}
impl<S: Source> Iterator for PairRouter<S> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            self.pair = self.control.pair();
        }
        let input = self.source.next()?;
        if self.channel < 2 {
            self.stereo[usize::from(self.channel)] = input;
        }
        let output = if self.pair == 0 {
            // Keep the original mapping, including mono and existing multichannel sources.
            input
        } else if self.channel / 2 == self.pair {
            self.stereo[usize::from(self.channel % 2)]
        } else {
            0.
        };
        self.channel = (self.channel + 1) % self.control.channels;
        Some(output)
    }
}
impl<S: Source> Source for PairRouter<S> {
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
    #[test]
    fn routes_complete_stereo_frames_and_preserves_default_mapping() {
        let data = [1., -1., 7., 8., 2., -2., 9., 10., 3., -3., 11., 12.];
        let (mut router, control) =
            PairRouter::new(rodio::buffer::SamplesBuffer::new(4, 48000, data.to_vec()));
        assert_eq!(router.next(), Some(1.));
        control.set_pair(1).unwrap();
        assert_eq!(
            router.by_ref().take(3).collect::<Vec<_>>(),
            vec![-1., 7., 8.]
        );
        assert_eq!(router.next(), Some(0.));
        control.set_pair(0).unwrap();
        assert_eq!(
            router.by_ref().take(3).collect::<Vec<_>>(),
            vec![0., 2., -2.]
        );
        assert_eq!(router.collect::<Vec<_>>(), vec![3., -3., 11., 12.]);
        assert!(control.set_pair(2).is_err());
        assert_eq!(control.pair(), 0);
        for channels in [1, 2, 3] {
            let (router, control) = PairRouter::new(rodio::buffer::SamplesBuffer::new(
                channels,
                48000,
                vec![0.25; usize::from(channels) * 2],
            ));
            assert!(control.set_pair(1).is_err());
            assert_eq!(
                router.collect::<Vec<_>>(),
                vec![0.25; usize::from(channels) * 2]
            );
        }
        for channels in [4, 6, 8] {
            let mut frame = vec![0.; usize::from(channels)];
            frame[0] = 0.25;
            frame[1] = -0.5;
            let (router, control) =
                PairRouter::new(rodio::buffer::SamplesBuffer::new(channels, 48000, frame));
            let pair = channels / 2 - 1;
            control.set_pair(pair).unwrap();
            let actual = router.collect::<Vec<_>>();
            for (channel, value) in actual.into_iter().enumerate() {
                let expected = if channel == usize::from(pair) * 2 {
                    0.25
                } else if channel == usize::from(pair) * 2 + 1 {
                    -0.5
                } else {
                    0.
                };
                assert_eq!(value, expected);
            }
        }
    }
}
