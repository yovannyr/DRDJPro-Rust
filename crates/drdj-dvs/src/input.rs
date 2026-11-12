//! Bounded single-producer PCM handoff; no native or decoder work in callback.
use crate::{Decoder, Error, Observation, Record, MAX_BLOCK_FRAMES};
use std::{
    cell::Cell,
    marker::PhantomData,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const CAPACITY: usize = 8192;
struct Ring {
    pcm: Box<[AtomicU32]>,
    times: Box<[AtomicU64]>,
    started: Instant,
    head: AtomicUsize,
    tail: AtomicUsize,
    lost: AtomicU64,
    failed: Arc<AtomicBool>,
}
impl Ring {
    fn new() -> Self {
        Self {
            pcm: (0..CAPACITY).map(|_| AtomicU32::new(0)).collect(),
            times: (0..CAPACITY).map(|_| AtomicU64::new(0)).collect(),
            started: Instant::now(),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            lost: AtomicU64::new(0),
            failed: Arc::new(AtomicBool::new(false)),
        }
    }
    fn push(&self, left: i16, right: i16, timestamp: u64) {
        let head = self.head.load(Ordering::Relaxed);
        if head.wrapping_sub(self.tail.load(Ordering::Acquire)) == CAPACITY {
            self.lost.fetch_add(1, Ordering::Relaxed);
            return;
        }
        self.pcm[head % CAPACITY].store(
            u32::from(left as u16) | (u32::from(right as u16) << 16),
            Ordering::Relaxed,
        );
        self.times[head % CAPACITY].store(timestamp, Ordering::Relaxed);
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }
    fn pop(&self) -> Option<[i16; 2]> {
        let tail = self.tail.load(Ordering::Relaxed);
        if tail == self.head.load(Ordering::Acquire) {
            return None;
        }
        let value = self.pcm[tail % CAPACITY].load(Ordering::Relaxed);
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some([value as i16, (value >> 16) as i16])
    }
    fn len(&self) -> usize {
        self.head
            .load(Ordering::Acquire)
            .wrapping_sub(self.tail.load(Ordering::Relaxed))
    }
    fn micros(&self) -> u64 {
        self.started.elapsed().as_micros() as u64
    }
    fn oldest_time(&self) -> Option<u64> {
        let tail = self.tail.load(Ordering::Relaxed);
        (tail != self.head.load(Ordering::Acquire))
            .then(|| self.times[tail % CAPACITY].load(Ordering::Relaxed))
    }
    fn stale(&self) -> bool {
        self.len() == CAPACITY
            || self
                .oldest_time()
                .is_some_and(|time| self.micros().saturating_sub(time) > 20000)
    }
    fn discard(&self) -> usize {
        let head = self.head.load(Ordering::Acquire);
        let previous = self.tail.swap(head, Ordering::Release);
        head.wrapping_sub(previous)
    }
}
/// Error callback signal. Setting it does not allocate, format or lock.
#[derive(Clone)]
pub struct StreamFailure(Arc<AtomicBool>);
impl StreamFailure {
    /// Mark capture failed; the worker publishes loss before native cleanup.
    pub fn signal(&self) {
        self.0.store(true, Ordering::Release);
    }
}
/// Single PCM producer: not Clone or Sync. Move it into one serialized callback.
/// The input callback copies frames only; all decoding is performed on a worker.
pub struct InputProducer {
    ring: Arc<Ring>,
    _single: PhantomData<Cell<()>>,
}
impl InputProducer {
    /// Separate error callback may signal failure concurrently with data input.
    pub fn failure(&self) -> StreamFailure {
        StreamFailure(Arc::clone(&self.ring.failed))
    }
    /// Feed complete stereo i16 frames. Malformed blocks fail capture explicitly.
    pub fn stereo(&mut self, pcm: &[i16]) {
        let (frames, remainder) = pcm.as_chunks::<2>();
        if !remainder.is_empty() {
            self.failure().signal();
            return;
        }
        let timestamp = self.ring.micros();
        for [left, right] in frames {
            self.ring.push(*left, *right, timestamp);
        }
    }
    pub(crate) fn interleaved<T: Copy>(
        &mut self,
        pcm: &[T],
        channels: usize,
        convert: impl Fn(T) -> Option<i16>,
    ) {
        if channels < 2 || !pcm.len().is_multiple_of(channels) {
            self.failure().signal();
            return;
        }
        let timestamp = self.ring.micros();
        for frame in pcm.chunks_exact(channels) {
            let (Some(left), Some(right)) = (convert(frame[0]), convert(frame[1])) else {
                self.failure().signal();
                return;
            };
            self.ring.push(left, right, timestamp);
        }
    }
}
/// Current worker observation. Tracking does not by itself start playback.
#[derive(Clone, Debug)]
pub enum InputState {
    Starting,
    SignalLost,
    Tracking(Observation),
    Failed(String),
    Stopped,
}
/// An observation timestamp is worker completion time, not hardware arrival time.
#[derive(Clone, Debug)]
pub struct InputSnapshot {
    pub revision: u64,
    pub epoch: u64,
    pub state: InputState,
    pub lost_frames: u64,
    pub observed_at: Instant,
}
/// Owned reader/decoder. Drop signals stop without joining blocked native cleanup.
pub struct InputCapture {
    snapshot: Arc<Mutex<Arc<InputSnapshot>>>,
    stop: Arc<AtomicBool>,
}
/// Two-phase native ownership: return the guard before starting it, so a start
/// failure can be published before a potentially blocking stream destructor.
pub trait InputStream: 'static {
    /// Start exactly once on its owning worker; errors never imply a retry.
    fn start(&mut self) -> Result<(), String>;
}
struct ExitState(Arc<Mutex<Arc<InputSnapshot>>>);
impl Drop for ExitState {
    fn drop(&mut self) {
        let mut slot = self.0.lock().unwrap_or_else(|error| error.into_inner());
        if !matches!(slot.state, InputState::Failed(_) | InputState::Stopped) {
            let mut state = (**slot).clone();
            state.revision = state.revision.saturating_add(1);
            state.state = InputState::Stopped;
            state.observed_at = Instant::now();
            *slot = Arc::new(state);
        }
    }
}
impl InputCapture {
    /// Start an explicitly chosen input. No automatic device replacement/restart.
    pub fn native(
        device: crate::InputDevice,
        record: Record,
        phono: bool,
    ) -> std::io::Result<Self> {
        Self::spawn(device.rate, record, phono, move |producer| {
            crate::native_input::open(device, producer)
        })
    }
    /// Backend factory and guard are constructed/retained/dropped on the worker.
    /// The factory starts one producer, retains it in its guard, and must return
    /// errors before potentially blocking destruction of an established stream.
    pub fn spawn<G: InputStream>(
        rate: u32,
        record: Record,
        phono: bool,
        factory: impl FnOnce(InputProducer) -> Result<G, String> + Send + 'static,
    ) -> std::io::Result<Self> {
        if !(8000..=192000).contains(&rate) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                Error::InvalidRate,
            ));
        }
        let stop = Arc::new(AtomicBool::new(false));
        let snapshot = Arc::new(Mutex::new(Arc::new(InputSnapshot {
            revision: 0,
            epoch: 1,
            state: InputState::Starting,
            lost_frames: 0,
            observed_at: Instant::now(),
        })));
        let worker_stop = Arc::clone(&stop);
        let published = Arc::clone(&snapshot);
        thread::Builder::new()
            .name("dvs-input".into())
            .spawn(move || {
                let mut state = InputSnapshot {
                    revision: 0,
                    epoch: 1,
                    state: InputState::Starting,
                    lost_frames: 0,
                    observed_at: Instant::now(),
                };
                let publish = |state: &mut InputSnapshot| {
                    state.revision = state.revision.saturating_add(1);
                    state.observed_at = Instant::now();
                    *published.lock().unwrap_or_else(|error| error.into_inner()) =
                        Arc::new(state.clone());
                };
                let mut stream = None;
                // Declared after stream: unwind publishes stop before guard cleanup.
                let _exit = ExitState(Arc::clone(&published));
                let result = (|| -> Result<(), String> {
                    // Allocate/filter initialization precedes input capture.
                    let mut decoder =
                        Decoder::new(record, rate, phono).map_err(|error| error.to_string())?;
                    let ring = Arc::new(Ring::new());
                    let producer = InputProducer {
                        ring: Arc::clone(&ring),
                        _single: PhantomData,
                    };
                    if worker_stop.load(Ordering::Acquire) {
                        return Ok(());
                    }
                    stream = Some(factory(producer)?);
                    stream.as_mut().ok_or("Input stream missing")?.start()?;
                    let mut overflow = 0;
                    let mut last_input = Instant::now();
                    let mut pcm = [0; MAX_BLOCK_FRAMES * 2];
                    while !worker_stop.load(Ordering::Acquire) {
                        if ring.failed.load(Ordering::Acquire) {
                            return Err("Audio input stream failed".into());
                        }
                        let lost = ring.lost.load(Ordering::Acquire);
                        if lost != overflow || ring.stale() {
                            state.lost_frames = state
                                .lost_frames
                                .saturating_add(lost.wrapping_sub(overflow))
                                .saturating_add(ring.discard() as u64);
                            overflow = lost;
                            decoder.reset();
                            state.epoch = state.epoch.saturating_add(1);
                            state.state = InputState::SignalLost;
                            publish(&mut state);
                        }
                        let oldest = ring.oldest_time();
                        let mut frames = 0;
                        for frame in pcm.as_chunks_mut::<2>().0 {
                            let Some(value) = ring.pop() else {
                                break;
                            };
                            *frame = value;
                            frames += 1;
                        }
                        if frames != 0 {
                            last_input = Instant::now();
                            if ring.lost.load(Ordering::Acquire) != overflow {
                                // Never emit a valid stamp across a callback overflow.
                                state.lost_frames = state.lost_frames.saturating_add(frames as u64);
                                continue;
                            }
                            let observation = decoder
                                .submit(&pcm[..frames * 2])
                                .map_err(|error| error.to_string())?;
                            if oldest.is_some_and(|time| ring.micros().saturating_sub(time) > 20000)
                            {
                                state.lost_frames = state
                                    .lost_frames
                                    .saturating_add(frames as u64)
                                    .saturating_add(ring.discard() as u64);
                                decoder.reset();
                                state.epoch = state.epoch.saturating_add(1);
                                state.state = InputState::SignalLost;
                                publish(&mut state);
                                continue;
                            }
                            state.state = match observation {
                                Some(observation) => InputState::Tracking(observation),
                                None => {
                                    if matches!(state.state, InputState::Tracking(_)) {
                                        state.epoch = state.epoch.saturating_add(1);
                                    }
                                    InputState::SignalLost
                                }
                            };
                            publish(&mut state);
                        } else {
                            if last_input.elapsed() >= Duration::from_millis(50)
                                && !matches!(state.state, InputState::SignalLost)
                            {
                                decoder.reset();
                                state.epoch = state.epoch.saturating_add(1);
                                state.state = InputState::SignalLost;
                                publish(&mut state);
                            }
                            thread::sleep(Duration::from_millis(1));
                        }
                    }
                    Ok(())
                })();
                state.state = match result {
                    Err(error) => InputState::Failed(error),
                    Ok(()) => InputState::Stopped,
                };
                publish(&mut state); // before a driver's potentially blocking destructor
                drop(stream);
            })?;
        Ok(Self { snapshot, stop })
    }
    /// Nonwaiting UI/control-plane read. Callers must also reject old timestamps
    /// if the native backend/worker stalls. This is not an audio callback API.
    pub fn try_snapshot(&self) -> Option<Arc<InputSnapshot>> {
        self.snapshot
            .try_lock()
            .ok()
            .map(|state| Arc::clone(&state))
    }
}
impl Drop for InputCapture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    #[test]
    fn old_short_input_is_rejected_by_wall_time_not_only_queue_length() {
        let ring = Ring {
            started: Instant::now() - Duration::from_millis(100),
            ..Ring::new()
        };
        ring.push(1, 2, 0);
        assert_eq!(ring.len(), 1);
        assert!(ring.stale());
        assert_eq!(ring.discard(), 1);
        assert!(!ring.stale());
    }
    #[test]
    fn pcm_ring_is_bounded_preserves_stereo_order_and_reports_overflow() {
        let ring = Ring::new();
        for index in 0..CAPACITY + 9 {
            ring.push(index as i16, !(index as i16), 0);
        }
        assert_eq!(ring.len(), CAPACITY);
        assert_eq!(ring.lost.load(Ordering::Acquire), 9);
        for index in 0..CAPACITY {
            assert_eq!(ring.pop(), Some([index as i16, !(index as i16)]));
        }
        assert!(ring.pop().is_none());
        // Exercise modular counters at usize wrap, not only an array index wrap.
        ring.head.store(usize::MAX - 1, Ordering::Relaxed);
        ring.tail.store(usize::MAX - 1, Ordering::Relaxed);
        for _ in 0..4 {
            ring.push(i16::MIN, i16::MAX, 0);
        }
        assert_eq!(ring.len(), 4);
        for _ in 0..4 {
            assert_eq!(ring.pop(), Some([i16::MIN, i16::MAX]));
        }
    }
    #[test]
    fn malformed_pcm_and_separate_error_callback_fail_explicitly() {
        let ring = Arc::new(Ring::new());
        let mut producer = InputProducer {
            ring: Arc::clone(&ring),
            _single: PhantomData,
        };
        producer.stereo(&[1]);
        assert!(ring.failed.load(Ordering::Acquire));
        assert_eq!(ring.len(), 0);
        ring.failed.store(false, Ordering::Release);
        producer.interleaved(&[f32::NAN, 1.0], 2, |sample| {
            sample.is_finite().then_some(0)
        });
        assert!(ring.failed.load(Ordering::Acquire));
        ring.failed.store(false, Ordering::Release);
        producer.failure().signal();
        assert!(ring.failed.load(Ordering::Acquire));
    }
    #[test]
    fn callback_copies_first_pair_without_mixing_or_losing_signs() {
        let ring = Arc::new(Ring::new());
        let mut producer = InputProducer {
            ring: Arc::clone(&ring),
            _single: PhantomData,
        };
        producer.interleaved(&[i16::MIN, i16::MAX, 99, 100, -2, 3, 98, 101], 4, Some);
        assert_eq!(ring.pop(), Some([i16::MIN, i16::MAX]));
        assert_eq!(ring.pop(), Some([-2, 3]));
        assert_eq!(ring.len(), 0);
    }
    struct Guard {
        _producer: InputProducer,
        closing: mpsc::Sender<()>,
        gate: mpsc::Receiver<()>,
        start_error: bool,
    }
    impl InputStream for Guard {
        fn start(&mut self) -> Result<(), String> {
            if self.start_error {
                Err("injected play error".into())
            } else {
                Ok(())
            }
        }
    }
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = self.closing.send(());
            let _ = self.gate.recv();
        }
    }
    #[test]
    fn stream_error_is_published_before_blocked_cleanup_and_stop_never_joins() {
        let (failure_tx, failure_rx) = mpsc::channel();
        let (closing_tx, closing_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let capture = InputCapture::spawn(
            48000,
            Record::SeratoSecondEditionA,
            false,
            move |producer| {
                failure_tx
                    .send((producer.failure(), thread::current().id()))
                    .unwrap();
                Ok(Guard {
                    _producer: producer,
                    closing: closing_tx,
                    gate: release_rx,
                    start_error: false,
                })
            },
        )
        .unwrap();
        let (failure, worker) = failure_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_ne!(worker, thread::current().id());
        failure.signal();
        closing_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let state = capture.try_snapshot().unwrap();
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let dropper = thread::spawn(move || {
            drop(capture);
            dropped_tx.send(()).unwrap();
        });
        let stopped = dropped_rx.recv_timeout(Duration::from_millis(200));
        release_tx.send(()).unwrap();
        dropper.join().unwrap();
        assert!(stopped.is_ok());
        assert!(matches!(state.state, InputState::Failed(_)));
    }
    #[test]
    fn overflowing_or_stale_pcm_is_discarded_before_decoding() {
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let capture = InputCapture::spawn(
            48000,
            Record::SeratoSecondEditionA,
            false,
            move |mut producer| {
                producer.stereo(&vec![0; (CAPACITY + 10) * 2]);
                ready_tx.send(()).unwrap();
                // The guard keeps producer alive; construction returns before decode.
                Ok(Guard {
                    _producer: producer,
                    closing: mpsc::channel().0,
                    gate: release_rx,
                    start_error: false,
                })
            },
        )
        .unwrap();
        ready_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let state = loop {
            if let Some(state) = capture.try_snapshot() {
                if state.lost_frames > 0 {
                    break state;
                }
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(state.lost_frames, (CAPACITY + 10) as u64);
        assert!(matches!(state.state, InputState::SignalLost));
        assert_eq!(state.epoch, 2);
        drop(capture);
        release_tx.send(()).unwrap();
    }
    #[test]
    fn play_failure_precedes_guard_cleanup_and_factory_panic_publishes_stop() {
        let (closing_tx, closing_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let capture = InputCapture::spawn(
            48000,
            Record::SeratoSecondEditionA,
            false,
            move |producer| {
                Ok(Guard {
                    _producer: producer,
                    closing: closing_tx,
                    gate: release_rx,
                    start_error: true,
                })
            },
        )
        .unwrap();
        closing_rx.recv_timeout(Duration::from_secs(3)).unwrap();
        let state = capture.try_snapshot().unwrap();
        drop(capture);
        release_tx.send(()).unwrap();
        assert!(
            matches!(&state.state, InputState::Failed(error) if error == "injected play error")
        );
        let capture = InputCapture::spawn(
            48000,
            Record::SeratoSecondEditionA,
            false,
            |_| -> Result<Guard, String> { panic!("injected backend factory panic") },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(state) = capture.try_snapshot() {
                if matches!(state.state, InputState::Stopped) {
                    break;
                }
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(1));
        }
    }
}
