//! Frame-aligned manual loops and immutable shared PCM. No locks or allocation in next().
use rodio::Source;
use std::{sync::Arc, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameLoop {
    pub start: usize,
    pub end: usize,
}
impl FrameLoop {
    pub fn new(start: usize, end: usize, frames: usize) -> Result<Self, String> {
        if start >= end || end > frames {
            return Err("Loop needs In < Out within complete track frames".into());
        }
        Ok(Self { start, end })
    }
    /// Supported lengths are powers of two from 1/8 to 32 beats. No beat snapping.
    pub fn from_beats(
        start: usize,
        eighths: u16,
        bpm: f64,
        rate: u32,
        frames: usize,
    ) -> Result<Self, String> {
        if !matches!(eighths, 1 | 2 | 4 | 8 | 16 | 32 | 64 | 128 | 256)
            || !bpm.is_finite()
            || !(40.0..=240.0).contains(&bpm)
            || rate == 0
        {
            return Err("Choose 1/8–32 beats and a valid manual BPM".into());
        }
        let length = (f64::from(rate) * 60.0 * f64::from(eighths) / (bpm * 8.0)).round() as usize;
        let end = start
            .checked_add(length)
            .ok_or("Loop frame range overflow")?;
        Self::new(start, end, frames)
    }
    pub fn resized(self, double: bool, frames: usize) -> Result<Self, String> {
        let length = self.end - self.start;
        let length = if double {
            length.checked_mul(2).ok_or("Loop frame range overflow")?
        } else {
            length / 2
        };
        Self::new(
            self.start,
            self.start
                .checked_add(length)
                .ok_or("Loop frame range overflow")?,
            frames,
        )
    }
    pub fn wrap(self, frame: usize) -> usize {
        if frame < self.end {
            frame
        } else {
            self.start + (frame - self.start) % (self.end - self.start)
        }
    }
}
pub fn frame_at(time: Duration, rate: u32) -> usize {
    (time.as_nanos().saturating_mul(u128::from(rate)) / 1_000_000_000).min(usize::MAX as u128)
        as usize
}
/// Round up fractional nanoseconds so converting this frame time back never loses a frame.
pub fn frame_time(frame: usize, rate: u32) -> Duration {
    Duration::from_nanos(((frame as u128 * 1_000_000_000).div_ceil(u128::from(rate))) as u64)
}
#[derive(Clone)]
pub struct PcmSource {
    data: Arc<Vec<f32>>,
    channels: u16,
    playback_rate: u32,
    cursor: usize,
    region: Option<FrameLoop>,
}
impl PcmSource {
    pub fn new(channels: u16, rate: u32, data: Vec<f32>) -> Self {
        assert!(channels > 0 && rate > 0 && data.len().is_multiple_of(usize::from(channels)));
        Self {
            data: Arc::new(data),
            channels,
            playback_rate: rate,
            cursor: 0,
            region: None,
        }
    }
    /// Configure on the control plane; iterator/sample storage stays unchanged.
    #[cfg(test)]
    pub fn set_playback_rate(&mut self, rate: std::num::NonZeroU32) {
        self.playback_rate = rate.get();
    }
    pub(crate) fn shared_samples(&self) -> Arc<Vec<f32>> {
        Arc::clone(&self.data)
    }
    pub(crate) fn cursor_frame(&self) -> usize {
        self.cursor / usize::from(self.channels)
    }
    pub(crate) fn sample_at(&self, frame: usize, channel: u16) -> f32 {
        self.data
            .get(frame * usize::from(self.channels) + usize::from(channel))
            .copied()
            .unwrap_or(0.)
    }
    pub(crate) fn loop_region(&self) -> Option<FrameLoop> {
        self.region
    }
    #[cfg(test)]
    pub(crate) fn storage_weak(&self) -> std::sync::Weak<Vec<f32>> {
        Arc::downgrade(&self.data)
    }
    pub fn frames(&self) -> usize {
        self.data.len() / usize::from(self.channels)
    }
    #[cfg(test)]
    pub fn set_loop(&mut self, region: FrameLoop) -> Result<(), String> {
        FrameLoop::new(region.start, region.end, self.frames())?;
        self.cursor = region.start * usize::from(self.channels);
        self.region = Some(region);
        Ok(())
    }
}
impl Iterator for PcmSource {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(region) = self.region {
            if self.cursor >= region.end * usize::from(self.channels) {
                self.cursor = region.start * usize::from(self.channels);
            }
        }
        let sample = self.data.get(self.cursor).copied()?;
        self.cursor += 1;
        Some(sample)
    }
}
impl Source for PcmSource {
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.playback_rate
    }
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<Duration> {
        if self.region.is_some() {
            None
        } else {
            Some(Duration::from_nanos(
                (self.frames() as u128 * 1_000_000_000 / u128::from(self.playback_rate)) as u64,
            ))
        }
    }
    fn try_seek(&mut self, time: Duration) -> Result<(), rodio::source::SeekError> {
        let frame = if self.region.is_none() && self.total_duration().is_some_and(|end| time >= end)
        {
            self.frames()
        } else {
            frame_at(time, self.playback_rate).min(self.frames())
        };
        self.cursor = self.region.map_or(frame, |r| r.wrap(frame)) * usize::from(self.channels);
        Ok(())
    }
}
#[cfg(test)]
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct HeapStats {
    pub allocations: usize,
    pub deallocations: usize,
    pub reallocations: usize,
    pub live_bytes: usize,
    pub peak_bytes: usize,
}
#[cfg(test)]
pub(crate) fn measured_heap(action: impl FnOnce()) -> HeapStats {
    tests::measure_heap(action)
}
#[cfg(test)]
pub(crate) fn counted_allocations(action: impl FnOnce()) -> usize {
    tests::count_allocations(action)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;
    thread_local! {
        static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
        static HEAP: Cell<Option<HeapStats>> = const { Cell::new(None) };
    }
    pub(super) fn measure_heap(action: impl FnOnce()) -> HeapStats {
        HEAP.with(|s| {
            assert!(s.get().is_none());
            s.set(Some(HeapStats::default()));
        });
        action();
        HEAP.with(|s| s.replace(None).unwrap())
    }
    fn heap_update(size: usize, old: usize, reallocation: bool, deallocation: bool) {
        let _ = HEAP.try_with(|cell| {
            if let Some(mut s) = cell.get() {
                if deallocation {
                    s.deallocations += 1;
                    s.live_bytes = s.live_bytes.saturating_sub(old);
                } else {
                    s.allocations += 1;
                    if reallocation {
                        s.reallocations += 1;
                    }
                    s.peak_bytes = s.peak_bytes.max(s.live_bytes.saturating_add(size));
                    s.live_bytes = s.live_bytes.saturating_sub(old).saturating_add(size);
                }
                cell.set(Some(s));
            }
        });
    }
    pub(super) fn count_allocations(action: impl FnOnce()) -> usize {
        COUNT.with(|count| {
            assert!(count.get().is_none());
            count.set(Some(0));
        });
        action();
        COUNT.with(|count| count.replace(None).unwrap())
    }
    struct CountingAllocator;
    unsafe impl GlobalAlloc for CountingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            heap_update(layout.size(), 0, false, false);
            let _ = COUNT.try_with(|count| {
                if let Some(n) = count.get() {
                    count.set(Some(n + 1));
                }
            });
            System.alloc(layout)
        }
        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            heap_update(0, layout.size(), false, true);
            System.dealloc(pointer, layout);
        }
        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            heap_update(size, layout.size(), true, false);
            let _ = COUNT.try_with(|count| {
                if let Some(n) = count.get() {
                    count.set(Some(n + 1));
                }
            });
            System.realloc(pointer, layout, size)
        }
    }
    #[global_allocator]
    static ALLOCATOR: CountingAllocator = CountingAllocator;
    #[test]
    fn shared_pcm_moves_the_existing_vec_buffer_without_copying_it() {
        let data = vec![0.25; 1000000];
        let pointer = data.as_ptr();
        let capacity = data.capacity();
        let source = PcmSource::new(1, 48000, data);
        assert_eq!(source.data.as_ref().as_ptr(), pointer);
        assert_eq!(source.data.capacity(), capacity);
        let clone = source.clone();
        assert!(Arc::ptr_eq(&source.data, &clone.data));
    }
    #[test]
    fn playback_rate_changes_duration_seek_units_and_preserves_stereo_storage() {
        let mut source = PcmSource::new(2, 4, vec![0., 1., 2., 3., 4., 5., 6., 7.]);
        let original = source.clone();
        source.set_playback_rate(std::num::NonZeroU32::new(8).unwrap());
        assert_eq!(source.sample_rate(), 8);
        assert_eq!(source.total_duration(), Some(Duration::from_millis(500)));
        assert_eq!(original.sample_rate(), 4);
        source.try_seek(Duration::from_millis(250)).unwrap();
        assert_eq!(source.next(), Some(4.));
        assert_eq!(source.next(), Some(5.));
        source.try_seek(Duration::from_millis(500)).unwrap();
        assert_eq!(source.next(), None);
        source.set_loop(FrameLoop::new(1, 3, 4).unwrap()).unwrap();
        assert!(source.total_duration().is_none());
        for i in 0..400 {
            assert_eq!(source.next(), Some([2., 3., 4., 5.][i % 4]));
        }
    }
    #[test]
    fn beat_lengths_round_once_and_resize_validates_before_mutation() {
        for rate in [44100, 48000, 96000] {
            for bpm in [40., 123.45, 240.] {
                for eighths in [1, 2, 4, 8, 16, 32, 64, 128, 256] {
                    let region =
                        FrameLoop::from_beats(17, eighths, bpm, rate, rate as usize * 60).unwrap();
                    let ideal = f64::from(rate) * 60. * f64::from(eighths) / (bpm * 8.);
                    assert!(((region.end - region.start) as f64 - ideal).abs() <= 0.5 + 1e-8);
                    assert_eq!(region.start, 17);
                }
            }
        }
        for (units, bpm, rate) in [
            (3, 120., 48000),
            (1, f64::NAN, 48000),
            (1, 39., 48000),
            (1, 120., 0),
        ] {
            assert!(FrameLoop::from_beats(0, units, bpm, rate, 48000).is_err());
        }
        assert!(FrameLoop::from_beats(47000, 8, 120., 48000, 48000).is_err());
        let region = FrameLoop::new(2, 7, 16).unwrap();
        assert_eq!(
            region.resized(false, 16).unwrap(),
            FrameLoop { start: 2, end: 4 }
        );
        assert_eq!(
            region.resized(true, 16).unwrap(),
            FrameLoop { start: 2, end: 12 }
        );
        assert!(region.resized(true, 8).is_err());
        assert!(FrameLoop::new(0, 1, 1).unwrap().resized(false, 1).is_err());
        // A fractional beat length repeats the identical rounded frame interval, with no accumulated drift.
        let region = FrameLoop::from_beats(3, 1, 120., 1000, 1000).unwrap();
        assert_eq!(region.end - region.start, 63);
        let mut source = PcmSource::new(
            2,
            1000,
            (0..1000)
                .flat_map(|frame| [frame as f32, -(frame as f32)])
                .collect(),
        );
        source.set_loop(region).unwrap();
        for i in 0..6300 {
            let value = (3 + i % 63) as f32;
            assert_eq!(source.next(), Some(value));
            assert_eq!(source.next(), Some(-value));
        }
    }
    #[test]
    fn repeated_loop_output_allocates_nothing_after_preparation() {
        let mut source = PcmSource::new(2, 48_000, vec![0.25, -0.25, 0.5, -0.5]);
        source.set_loop(FrameLoop::new(0, 2, 2).unwrap()).unwrap();
        source.set_playback_rate(std::num::NonZeroU32::new(72_000).unwrap());
        COUNT.with(|c| c.set(Some(0)));
        let started = std::time::Instant::now();
        for _ in 0..1_000_000 {
            std::hint::black_box(source.next());
        }
        let elapsed = started.elapsed();
        let allocations = COUNT.with(|c| c.replace(None).unwrap());
        assert_eq!(allocations, 0);
        eprintln!("LoopSource: 1000000 samples in {elapsed:?}, {allocations} allocations (diagnostic, not a real-time deadline)");
    }
    #[test]
    fn stereo_one_frame_loop_and_many_wraps_have_exact_channel_order() {
        let mut source = PcmSource::new(2, 48_000, vec![0., 1., 2., 3., 4., 5., 6., 7.]);
        assert!(FrameLoop::new(1, 1, 4).is_err());
        assert!(FrameLoop::new(0, 5, 4).is_err());
        source.set_loop(FrameLoop::new(1, 3, 4).unwrap()).unwrap();
        assert!(source.total_duration().is_none());
        for i in 0..100_000 {
            assert_eq!(source.next(), Some([2., 3., 4., 5.][i % 4]));
        }
        source.set_loop(FrameLoop::new(3, 4, 4).unwrap()).unwrap();
        for _ in 0..1000 {
            assert_eq!(source.next(), Some(6.));
            assert_eq!(source.next(), Some(7.));
        }
    }
    #[test]
    fn integer_frame_times_do_not_drift_and_eof_is_finite_without_loop() {
        for rate in [44100, 48000, 96000] {
            for frame in [0, 1, 17, rate as usize, rate as usize * 3600 + 1] {
                assert_eq!(frame_at(frame_time(frame, rate), rate), frame);
            }
        }
        let mut fractional = PcmSource::new(2, 44100, vec![1., -1., 2., -2.]);
        fractional
            .try_seek(fractional.total_duration().unwrap())
            .unwrap();
        assert_eq!(fractional.next(), None);
        let mut source = PcmSource::new(1, 2, vec![1., 2., 3., 4.]);
        source.try_seek(Duration::from_secs(1)).unwrap();
        assert_eq!(source.collect::<Vec<_>>(), vec![3., 4.]);
    }
}
