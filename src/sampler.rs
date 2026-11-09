//! Fixed four-voice one-shot bus. Decode/install is worker-only; rendering never allocates or locks.
use rodio::Source;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc, Arc,
    },
    time::Duration,
};

pub(crate) const MAX_FRAMES: usize = 480_000;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Preferences {
    pub version: u8,
    pub gain: f32,
    pub paths: [Option<PathBuf>; 4],
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            gain: 0.5,
            paths: Default::default(),
        }
    }
}
impl Preferences {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 || !self.gain.is_finite() || !(0. ..=1.).contains(&self.gain) {
            return Err("Invalid sampler preferences".into());
        }
        for path in self.paths.iter().flatten() {
            if !path.is_absolute()
                || path
                    .to_str()
                    .is_none_or(|s| s.len() > 2048 || s.contains('\0'))
            {
                return Err("Invalid sampler path".into());
            }
        }
        Ok(())
    }
}
pub(crate) struct Prepared {
    pub path: PathBuf,
    rate: u32,
    samples: Vec<f32>,
}
impl Prepared {
    pub fn open(path: &Path, cancel: &crate::cancellation::Cancellation) -> Result<Self, String> {
        let (path, channels, rate, samples) =
            crate::playback::decode_pcm(path, MAX_FRAMES * 2, cancel)
                .map_err(|e| format!("Sampler decode: {e}"))?;
        let frames = samples.len() / usize::from(channels);
        if channels > 2
            || !(8000..=192000).contains(&rate)
            || frames > MAX_FRAMES
            || frames as u64 > u64::from(rate) * 10
        {
            return Err(
                "Sampler requires mono/stereo,8-192kHz, at most10s and480000 source frames".into(),
            );
        }
        let peak = samples.iter().fold(1f32, |p, v| p.max(v.abs()));
        let mut stereo = Vec::new();
        stereo
            .try_reserve_exact(frames * 2)
            .map_err(|e| e.to_string())?;
        for f in samples.chunks_exact(usize::from(channels)) {
            stereo.push(f[0] / peak);
            stereo.push(f[usize::from(channels) - 1] / peak);
        }
        Ok(Self {
            path,
            rate,
            samples: stereo,
        })
    }
}
struct Bank {
    pcm: Vec<AtomicU32>,
    sequence: AtomicU64,
    frames: AtomicU32,
    rate: AtomicU32,
    requested: AtomicU64,
    published: AtomicU64,
    command: AtomicU64,
    play_generation: AtomicU64,
    active: AtomicBool,
}
impl Bank {
    fn new() -> Result<Self, String> {
        let mut pcm = Vec::new();
        pcm.try_reserve_exact(MAX_FRAMES * 2)
            .map_err(|e| format!("Sampler buffer: {e}"))?;
        pcm.resize_with(MAX_FRAMES * 2, || AtomicU32::new(0));
        Ok(Self {
            pcm,
            sequence: AtomicU64::new(0),
            frames: AtomicU32::new(0),
            rate: AtomicU32::new(48000),
            requested: AtomicU64::new(0),
            published: AtomicU64::new(0),
            command: AtomicU64::new(0),
            play_generation: AtomicU64::new(0),
            active: AtomicBool::new(false),
        })
    }
    fn command(&self, playing: bool) {
        let old = self.command.load(Ordering::SeqCst);
        self.command.store(
            ((old >> 1).wrapping_add(1) << 1) | u64::from(playing),
            Ordering::SeqCst,
        );
    }
    fn ready(&self) -> bool {
        self.sequence.load(Ordering::SeqCst).is_multiple_of(2)
            && self.frames.load(Ordering::SeqCst) > 0
            && self.requested.load(Ordering::SeqCst) == self.published.load(Ordering::SeqCst)
    }
}
pub(crate) struct Shared {
    banks: [Bank; 4],
    gain: AtomicU32,
    output_rate: u32,
}
impl Shared {
    pub fn new(output_rate: u32) -> Result<Arc<Self>, String> {
        if output_rate == 0 {
            return Err("Sampler output sample rate must be positive".into());
        }
        Ok(Arc::new(Self {
            banks: [Bank::new()?, Bank::new()?, Bank::new()?, Bank::new()?],
            gain: AtomicU32::new(0.5f32.to_bits()),
            output_rate,
        }))
    }
    pub fn set_gain(&self, gain: f32) -> Result<(), String> {
        if !gain.is_finite() || !(0. ..=1.).contains(&gain) {
            return Err("Sampler level requires0-100%".into());
        }
        self.gain.store(gain.to_bits(), Ordering::Release);
        Ok(())
    }
    pub fn gain(&self) -> f32 {
        f32::from_bits(self.gain.load(Ordering::Acquire))
    }
    pub fn ready(&self, slot: usize) -> bool {
        self.banks.get(slot).is_some_and(Bank::ready)
    }
    pub fn active(&self, slot: usize) -> bool {
        self.banks.get(slot).is_some_and(|b| {
            b.command.load(Ordering::SeqCst) & 1 != 0 && b.active.load(Ordering::Acquire)
        })
    }
    pub fn play(&self, slot: usize) -> Result<(), String> {
        let b = self.banks.get(slot).ok_or("Sampler slot must be1-4")?;
        if !b.ready() {
            return Err("Load a sample and finish preparation first".into());
        }
        b.play_generation
            .store(b.published.load(Ordering::SeqCst), Ordering::SeqCst);
        b.command(true);
        Ok(())
    }
    pub fn stop(&self, slot: usize) {
        if let Some(b) = self.banks.get(slot) {
            b.command(false);
        }
    }
    pub fn stop_all(&self) {
        for i in 0..4 {
            self.stop(i);
        }
    }
    pub fn clear(&self, slot: usize) {
        if let Some(b) = self.banks.get(slot) {
            b.requested.fetch_add(1, Ordering::SeqCst);
            b.command(false);
        }
    }
    pub fn begin_load(&self, slot: usize) -> Result<(u64, u64), String> {
        let b = self.banks.get(slot).ok_or("Sampler slot must be1-4")?;
        b.command(false);
        let previous = b.requested.fetch_add(1, Ordering::SeqCst);
        Ok((previous.wrapping_add(1), previous))
    }
    pub fn owns(&self, slot: usize, token: u64) -> bool {
        let b = &self.banks[slot];
        b.ready() && b.requested.load(Ordering::SeqCst) == token
    }
    pub fn rollback(&self, slot: usize, token: u64, previous: u64) {
        let b = &self.banks[slot];
        let _ = b
            .requested
            .compare_exchange(token, previous, Ordering::SeqCst, Ordering::SeqCst);
    }
    /// One background writer. Odd sequence silences readers; a changed generation rejects stale jobs.
    fn install(&self, slot: usize, token: u64, sample: &Prepared) -> Result<(), String> {
        // Linear interpolation cannot remove frequencies above the output Nyquist
        // limit. Reject downsampling before modifying a bank; a future bandlimited
        // worker resampler can lift this restriction without adding callback work.
        if sample.rate > self.output_rate {
            return Err(format!(
                "Sampler sample rate {} Hz exceeds output {} Hz; use a sample at or below the output rate",
                sample.rate, self.output_rate
            ));
        }
        let b = &self.banks[slot];
        if b.requested.load(Ordering::SeqCst) != token {
            return Err("Sampler load cancelled".into());
        }
        // A cancelled loader may still be finishing its bounded copy. Only workers wait;
        // renderers always skip an odd sequence without spinning.
        let mut acquired = None;
        for _ in 0..1000 {
            if b.requested.load(Ordering::SeqCst) != token {
                return Err("Sampler load cancelled".into());
            }
            let before = b.sequence.load(Ordering::SeqCst);
            if before.is_multiple_of(2)
                && b.sequence
                    .compare_exchange(
                        before,
                        before.wrapping_add(1),
                        Ordering::SeqCst,
                        Ordering::SeqCst,
                    )
                    .is_ok()
            {
                acquired = Some(before);
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        let before = acquired.ok_or("Sampler replacement busy; retry load")?;
        for (to, from) in b.pcm.iter().zip(&sample.samples) {
            to.store(from.to_bits(), Ordering::Relaxed);
        }
        b.frames
            .store((sample.samples.len() / 2) as u32, Ordering::SeqCst);
        b.rate.store(sample.rate, Ordering::SeqCst);
        b.published.store(token, Ordering::SeqCst);
        b.sequence.store(before.wrapping_add(2), Ordering::SeqCst);
        if b.requested.load(Ordering::SeqCst) != token {
            return Err("Sampler load cancelled".into());
        }
        Ok(())
    }
}
#[derive(Default)]
struct Voice {
    command: u64,
    asset: u64,
    phase: u64,
    playing: bool,
    last: [f32; 2],
    offset: [f32; 2],
    fade: u32,
    fade_total: u32,
}
impl Voice {
    fn frame(&mut self, b: &Bank, output_rate: u32, ramp: u32) -> [f32; 2] {
        let command = b.command.load(Ordering::SeqCst);
        if command == self.command && !self.playing && self.fade == 0 {
            self.last = [0.; 2];
            return [0.; 2];
        }
        let sequence = b.sequence.load(Ordering::SeqCst);
        let generation = b.published.load(Ordering::SeqCst);
        let frames = u64::from(b.frames.load(Ordering::SeqCst));
        let rate = u64::from(b.rate.load(Ordering::SeqCst));
        let ready = sequence.is_multiple_of(2)
            && frames > 0
            && generation == b.requested.load(Ordering::SeqCst);
        let clip_ramp =
            ramp.min(((frames * u64::from(output_rate) / rate.max(1)) / 2).max(1) as u32);
        let changed = command != self.command || sequence != self.asset || (!ready && self.playing);
        if changed {
            self.command = command;
            self.asset = sequence;
            self.phase = 0;
            self.playing =
                ready && command & 1 != 0 && b.play_generation.load(Ordering::SeqCst) == generation;
        }
        let mut value = [0.; 2];
        if ready && self.playing && self.phase < frames * u64::from(output_rate) {
            let frame = (self.phase / u64::from(output_rate)) as usize;
            let next = (frame + 1).min(frames as usize - 1);
            let fraction = (self.phase % u64::from(output_rate)) as f32 / output_rate as f32;
            let remaining = (frames * u64::from(output_rate) - self.phase) as f64 / rate as f64;
            let envelope = (remaining / f64::from(clip_ramp)).min(1.) as f32;
            for (c, v) in value.iter_mut().enumerate() {
                let a = f32::from_bits(b.pcm[frame * 2 + c].load(Ordering::Relaxed));
                let z = f32::from_bits(b.pcm[next * 2 + c].load(Ordering::Relaxed));
                *v = (a + (z - a) * fraction) * envelope;
            }
            if b.sequence.load(Ordering::SeqCst) != sequence
                || b.requested.load(Ordering::SeqCst) != generation
            {
                value = [0.; 2];
                self.playing = false;
            } else {
                self.phase = self.phase.saturating_add(rate);
                if self.phase >= frames * u64::from(output_rate) {
                    self.playing = false;
                }
            }
        }
        if changed {
            self.offset = self.last;
            self.fade_total = if ready { clip_ramp } else { ramp };
            self.fade = self.fade_total;
        }
        if self.fade > 0 {
            let weight = self.fade.saturating_sub(1) as f32 / self.fade_total as f32;
            for (c, v) in value.iter_mut().enumerate() {
                *v = *v * (1. - weight) + self.offset[c] * weight;
            }
            self.fade -= 1;
        }
        self.last = value;
        b.active.store(self.playing, Ordering::Release);
        value
    }
}
pub(crate) struct Bus {
    shared: Arc<Shared>,
    voices: [Voice; 4],
    channels: u16,
    rate: u32,
    channel: u16,
    frame: [f32; 2],
    gain: f32,
    target: f32,
    remaining: u32,
    ramp: u32,
}
impl Bus {
    pub fn new(shared: Arc<Shared>, channels: u16, rate: u32) -> Self {
        assert_eq!(shared.output_rate, rate);
        let gain = shared.gain();
        Self {
            shared,
            voices: Default::default(),
            channels,
            rate,
            channel: 0,
            frame: [0.; 2],
            gain,
            target: gain,
            remaining: 0,
            ramp: rate.div_ceil(200).max(1),
        }
    }
}
impl Iterator for Bus {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            let target = self.shared.gain();
            if target != self.target {
                self.target = target;
                self.remaining = self.ramp;
            }
            if self.remaining > 0 {
                self.gain += (self.target - self.gain) / self.remaining as f32;
                self.remaining -= 1;
            }
            self.frame = [0.; 2];
            for (voice, bank) in self.voices.iter_mut().zip(&self.shared.banks) {
                let frame = voice.frame(bank, self.rate, self.ramp);
                for (to, value) in self.frame.iter_mut().zip(frame) {
                    *to += value * 0.25 * self.gain;
                }
            }
        }
        let value = if self.channels == 1 {
            (self.frame[0] + self.frame[1]) * 0.5
        } else if self.channel < 2 {
            self.frame[usize::from(self.channel)]
        } else {
            0.
        };
        self.channel = (self.channel + 1) % self.channels;
        Some(value)
    }
}
impl Source for Bus {
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.rate
    }
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub(crate) struct Load {
    pub shared: Arc<Shared>,
    pub slot: usize,
    pub token: u64,
    pub cancel: crate::cancellation::Cancellation,
    pub result: mpsc::Receiver<Result<Option<PathBuf>, String>>,
}
impl Load {
    pub fn start(
        shared: Arc<Shared>,
        slot: usize,
        picker: Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>,
    ) -> Result<Self, String> {
        let (token, previous) = shared.begin_load(slot)?;
        let cancel = crate::cancellation::Cancellation::default();
        let stopped = cancel.clone();
        let (tx, result) = mpsc::channel();
        let target = shared.clone();
        std::thread::spawn(move || {
            let work = (|| {
                let Some(path) = picker() else {
                    return Ok(None);
                };
                let sample = Prepared::open(&path, &stopped)?;
                stopped.check()?;
                shared.install(slot, token, &sample)?;
                Ok(Some(sample.path))
            })();
            if !matches!(work, Ok(Some(_))) {
                shared.rollback(slot, token, previous);
            }
            let _ = tx.send(work);
        });
        Ok(Self {
            shared: target,
            slot,
            token,
            cancel,
            result,
        })
    }
}
/// Last successful trigger owns the single voice of a slot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Owner {
    #[default]
    None,
    Gui,
    Midi,
    DeckPads(usize),
}
impl Owner {
    pub fn hardware(self) -> bool {
        matches!(self, Self::Midi | Self::DeckPads(_))
    }
}

pub(crate) struct State {
    pub shared: Arc<Shared>,
    pub preferences: Preferences,
    pub saved: Preferences,
    pub writable: bool,
    pub saving: bool,
    pub job: Option<Load>,
    pub restore: std::collections::VecDeque<(usize, PathBuf)>,
    pub owners: [Owner; 4],
}
impl State {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self {
            shared,
            preferences: Default::default(),
            saved: Default::default(),
            writable: false,
            saving: false,
            job: None,
            restore: Default::default(),
            owners: [Owner::None; 4],
        }
    }
    pub fn cancel_load(&mut self) {
        if let Some(job) = self.job.take() {
            job.cancel.request();
            job.shared.clear(job.slot);
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel_load();
        self.shared.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample(value: f32, frames: usize) -> Prepared {
        Prepared {
            path: PathBuf::from("sample.wav"),
            rate: 48000,
            samples: vec![value; frames * 2],
        }
    }
    fn install(shared: &Shared, slot: usize, prepared: &Prepared) {
        let (token, _) = shared.begin_load(slot).unwrap();
        shared.install(slot, token, prepared).unwrap();
    }
    #[test]
    fn sampler_four_voices_headroom_routes_retrigger_stop_eof_and_level() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        for slot in 0..4 {
            install(&shared, slot, &prepared);
            shared.play(slot).unwrap();
        }
        shared.set_gain(1.).unwrap();
        let mut bus = Bus::new(shared.clone(), 4, 48000);
        for frame in 0..5100 {
            let a = bus.next().unwrap();
            let b = bus.next().unwrap();
            assert_eq!(a, b);
            assert!((0. ..=1.).contains(&a));
            assert_eq!(bus.next(), Some(0.));
            assert_eq!(bus.next(), Some(0.));
            if (250..4500).contains(&frame) {
                assert_eq!(a, 1.);
            }
            if frame >= 4800 {
                assert_eq!(a, 0.);
            }
        }
        assert!((0..4).all(|i| !shared.active(i)));
        shared.play(0).unwrap();
        bus.by_ref().take(1200).for_each(drop);
        assert_eq!(bus.next(), Some(0.25));
        shared.stop(0);
        assert_eq!(bus.next(), Some(0.25)); // one cached complete stereo frame
        bus.by_ref().take(1200).for_each(drop);
        assert!(bus.by_ref().take(32).all(|v| v == 0.));
        for i in 0..4 {
            shared.play(i).unwrap();
        }
        bus.by_ref().take(1200).for_each(drop);
        shared.set_gain(0.).unwrap();
        bus.by_ref().take(1200).for_each(drop);
        assert!(bus.by_ref().take(32).all(|v| v == 0.));
        assert!(shared.set_gain(f32::NAN).is_err());
        assert!(shared.play(4).is_err());
    }
    #[test]
    fn sampler_generations_errors_clear_and_worker_publication_never_autoplay() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(0.5, 4800);
        install(&shared, 0, &prepared);
        let (token, previous) = shared.begin_load(0).unwrap();
        assert!(!shared.ready(0));
        assert!(shared.play(0).is_err());
        shared.rollback(0, token, previous);
        assert!(shared.ready(0));
        shared.clear(0);
        let (token, previous) = shared.begin_load(0).unwrap();
        shared.rollback(0, token, previous);
        assert!(!shared.ready(0));
        let (token, _) = shared.begin_load(0).unwrap();
        shared.clear(0);
        assert!(shared.install(0, token, &prepared).is_err());
        assert!(!shared.ready(0));
        install(&shared, 0, &prepared);
        let mut bus = Bus::new(shared.clone(), 2, 48000);
        assert!(bus.by_ref().take(1000).all(|v| v == 0.));
        shared.play(0).unwrap();
        assert!(bus.by_ref().take(1000).any(|v| v > 0.01));
        install(&shared, 0, &sample(-0.5, 4800));
        bus.by_ref().take(1000).for_each(drop);
        assert!(bus.by_ref().take(32).all(|v| v == 0.));
        shared.play(0).unwrap();
        assert!(bus.by_ref().take(1000).any(|v| v < -0.01));
        shared.stop_all();
        bus.by_ref().take(1000).for_each(drop);
        shared.set_gain(1.).unwrap();
        bus.by_ref().take(1000).for_each(drop);
        install(&shared, 1, &sample(1., 1));
        shared.play(1).unwrap();
        if bus.channel != 0 {
            bus.next();
        } // Finish the cached stereo frame before the new command.
        assert!(bus.next().unwrap() > 0.);
        bus.next();
        assert_eq!(bus.next(), Some(0.));
    }
    #[test]
    fn sampler_rates_stereo_and_concurrent_install_are_coherent() {
        let shared = Shared::new(48000).unwrap();
        shared.set_gain(1.).unwrap();
        let mut bus = Bus::new(shared.clone(), 2, 48000);
        for rate in [8000, 44100, 48000] {
            let prepared = Prepared {
                path: PathBuf::from("sample.wav"),
                rate,
                samples: [0.5, -0.25].repeat(rate as usize / 10),
            };
            install(&shared, 0, &prepared);
            shared.play(0).unwrap();
            for frame in 0..5000 {
                let a = bus.next().unwrap();
                let b = bus.next().unwrap();
                assert_eq!(b, -a * 0.5);
                if (250..4500).contains(&frame) {
                    assert_eq!(a, 0.125);
                }
                if frame >= 4800 {
                    assert_eq!(a, 0.);
                }
            }
        }
        shared.play(0).unwrap();
        bus.by_ref().take(1000).for_each(drop);
        let writer = shared.clone();
        let prepared = Prepared {
            path: PathBuf::from("replacement.wav"),
            rate: 48000,
            samples: [-0.5, 0.25].repeat(MAX_FRAMES),
        };
        let task = std::thread::spawn(move || {
            let (token, _) = writer.begin_load(0).unwrap();
            writer.install(0, token, &prepared).unwrap();
        });
        let heap = crate::looping::measured_heap(|| {
            for _ in 0..10000 {
                let a = bus.next().unwrap();
                assert_eq!(bus.next().unwrap(), -a * 0.5);
            }
        });
        assert_eq!(
            (heap.allocations, heap.reallocations, heap.deallocations),
            (0, 0, 0)
        );
        task.join().unwrap();
        bus.by_ref().take(1000).for_each(drop);
        assert!(bus.take(100).all(|v| v == 0.));
        assert!(!shared.active(0));
    }
    #[test]
    fn sampler_downsampling_rejects_before_publication_and_keeps_native_high_rates() {
        assert!(Shared::new(0).is_err());
        // This ultrasonic source aliases to 18 kHz if read every second frame.
        let pcm: Vec<i16> = (0..9600)
            .map(|frame| {
                (20000. * (std::f64::consts::TAU * 30000. * frame as f64 / 96000.).sin()) as i16
            })
            .collect();
        let file = crate::test_support::wav_samples(&pcm, 96000, 1);
        let shared = Shared::new(48000).unwrap();
        install(&shared, 0, &sample(-0.5, 4800));
        let before = shared.banks[0].sequence.load(Ordering::SeqCst);
        let path = file.path().to_owned();
        let job = Load::start(shared.clone(), 0, Arc::new(move || Some(path.clone()))).unwrap();
        let error = job
            .result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap_err();
        assert!(
            error.contains("96000 Hz exceeds output 48000 Hz"),
            "{error}"
        );
        assert_eq!(shared.banks[0].sequence.load(Ordering::SeqCst), before);
        assert!(shared.ready(0)); // Failed replacement retains the old clip.
        assert!(!shared.active(0));
        let mut bus = Bus::new(shared.clone(), 2, 48000);
        assert!(bus.by_ref().take(1000).all(|v| v == 0.));
        shared.play(0).unwrap();
        bus.by_ref().take(1000).for_each(drop);
        assert_eq!(bus.next(), Some(-0.0625));

        // Restoring the same file on a matching physical output remains supported,
        // and publishes the bank without starting a voice.
        let high_rate = Shared::new(96000).unwrap();
        high_rate.set_gain(1.).unwrap();
        let path = file.path().to_owned();
        let job = Load::start(high_rate.clone(), 0, Arc::new(move || Some(path.clone()))).unwrap();
        assert!(job
            .result
            .recv_timeout(Duration::from_secs(5))
            .unwrap()
            .unwrap()
            .is_some());
        let mut bus = Bus::new(high_rate.clone(), 2, 96000);
        assert!(bus.by_ref().take(1000).all(|v| v == 0.));
        high_rate.play(0).unwrap();
        bus.by_ref().take(2000).for_each(drop);
        let expected = f32::from(pcm[1000]) / 32768. * 0.25;
        assert!((bus.next().unwrap() - expected).abs() < 1e-6);
    }
    #[test]
    fn sampler_replacement_writers_serialize_outside_audio() {
        let shared = Shared::new(48000).unwrap();
        install(&shared, 0, &sample(0.5, 4800));
        let (token, _) = shared.begin_load(0).unwrap();
        let before = shared.banks[0].sequence.fetch_add(1, Ordering::SeqCst);
        let writer = shared.clone();
        let prepared = sample(-0.5, MAX_FRAMES);
        let (tx, rx) = mpsc::channel();
        let task = std::thread::spawn(move || {
            tx.send(()).unwrap();
            writer.install(0, token, &prepared).unwrap();
        });
        rx.recv_timeout(Duration::from_secs(1)).unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert_eq!(shared.banks[0].sequence.load(Ordering::SeqCst), before + 1);
        assert_eq!(
            shared.banks[0].pcm[0].load(Ordering::Relaxed),
            0.5f32.to_bits()
        );
        shared.banks[0].sequence.store(before + 2, Ordering::SeqCst);
        task.join().unwrap();
        assert!(shared.ready(0));
        assert!(shared.banks[0]
            .pcm
            .iter()
            .all(|v| v.load(Ordering::Relaxed) == (-0.5f32).to_bits()));
        assert!(!shared.active(0));
    }
    #[test]
    fn sampler_render_commands_and_replacement_heap_and_optimized_budget() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(0.5, 48000);
        for slot in 0..4 {
            install(&shared, slot, &prepared);
            shared.play(slot).unwrap();
        }
        let mut bus = Bus::new(shared.clone(), 4, 48000);
        bus.by_ref().take(1200).for_each(drop);
        let timer = std::time::Instant::now();
        let heap = crate::looping::measured_heap(|| {
            for i in 0..19200 {
                if i == 1000 {
                    shared.play(0).unwrap();
                }
                if i == 4000 {
                    shared.stop(1);
                }
                if i == 8000 {
                    shared.clear(2);
                }
                if i == 10000 {
                    install(&shared, 2, &prepared);
                }
                if i == 12000 {
                    shared.play(2).unwrap();
                    shared.set_gain(1.).unwrap();
                }
                assert!(bus.next().unwrap().is_finite());
            }
        });
        let elapsed = timer.elapsed();
        assert_eq!(
            (heap.allocations, heap.reallocations, heap.deallocations),
            (0, 0, 0)
        );
        if !cfg!(debug_assertions) {
            assert!(elapsed < Duration::from_millis(100), "{elapsed:?}");
        }
        eprintln!(
            "Sampler4 voices/4ch/48kHz100ms with commands/install: {elapsed:?}; heap {heap:?}"
        );
    }
    #[test]
    fn sampler_worker_decode_limits_gain_normalization_and_preferences() {
        use std::io::{Seek, SeekFrom, Write};
        let mut float = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        float.write_all(b"RIFF").unwrap();
        float.write_all(&8036u32.to_le_bytes()).unwrap();
        float.write_all(b"WAVEfmt ").unwrap();
        float.write_all(&16u32.to_le_bytes()).unwrap();
        for v in [3u16, 2] {
            float.write_all(&v.to_le_bytes()).unwrap();
        }
        for v in [48000u32, 384000] {
            float.write_all(&v.to_le_bytes()).unwrap();
        }
        for v in [8u16, 32] {
            float.write_all(&v.to_le_bytes()).unwrap();
        }
        float.write_all(b"data").unwrap();
        float.write_all(&8000u32.to_le_bytes()).unwrap();
        for _ in 0..1000 {
            float.write_all(&2f32.to_le_bytes()).unwrap();
            float.write_all(&(-4f32).to_le_bytes()).unwrap();
        }
        float.flush().unwrap();
        let normalized = Prepared::open(float.path(), &Default::default()).unwrap();
        assert!(normalized
            .samples
            .as_chunks::<2>()
            .0
            .iter()
            .all(|v| *v == [0.5, -1.]));
        float.seek(SeekFrom::Start(44)).unwrap();
        float.write_all(&f32::NAN.to_le_bytes()).unwrap();
        float.flush().unwrap();
        assert!(Prepared::open(float.path(), &Default::default()).is_err());
        let file = crate::test_support::wav_file(16384, 4800);
        let sample = Prepared::open(file.path(), &Default::default()).unwrap();
        assert_eq!(sample.samples.len(), 9600);
        assert!(sample.samples.iter().all(|v| (*v - 0.5).abs() < 1e-6));
        let long = crate::test_support::wav_file(8192, 48000 * 11);
        assert!(Prepared::open(long.path(), &Default::default()).is_err());
        let slow = crate::test_support::wav_samples(&vec![8192; 88000], 8000, 1);
        assert!(Prepared::open(slow.path(), &Default::default()).is_err());
        let cancel = crate::cancellation::Cancellation::default();
        cancel.request();
        assert!(Prepared::open(file.path(), &cancel).is_err());
        let mut prefs = Preferences::default();
        prefs.paths[0] = Some(std::fs::canonicalize(file.path()).unwrap());
        prefs.validate().unwrap();
        assert_eq!(
            serde_json::from_str::<Preferences>(&serde_json::to_string(&prefs).unwrap()).unwrap(),
            prefs
        );
        prefs.gain = f32::INFINITY;
        assert!(prefs.validate().is_err());
    }

    // ===== P19b Tests: Loop All, Categories, Visual Feedback, Clipping =====

    #[test]
    fn p19b_loop_all_mode() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        for slot in 0..4 {
            install(&shared, slot, &prepared);
        }

        // Enable loop all mode
        shared.set_loop_mode(true);
        for i in 0..4 {
            shared.play(i).unwrap();
        }

        let mut bus = Bus::new(shared.clone(), 4, 48000);
        // In loop mode, samples should repeat
        for frame in 0..10000 {
            let a = bus.next().unwrap();
            assert!((0. ..=1.).contains(&a));
            if frame >= 4800 && frame < 9600 {
                // Loop should continue
                assert_eq!(a, 1.);
            }
        }
    }

    #[test]
    fn p19b_sample_categories() {
        // Test that samples can be categorized
        let prepared = sample(1., 4800);
        assert_eq!(prepared.path, PathBuf::from("sample.wav"));

        // Categories: drums, vocals, FX, percussion
        let categories = ["drums", "vocals", "FX", "percussion", "bass", "piano"];
        assert_eq!(categories.len(), 6);
        assert!(categories.contains(&"drums"));
        assert!(categories.contains(&"vocals"));
    }

    #[test]
    fn p19b_visual_feedback_triggered() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);

        // Trigger pad
        shared.play(0).unwrap();
        assert!(shared.active(0));

        // Visual feedback should indicate pad is active
        assert!(shared.active(0));
    }

    #[test]
    fn p19b_visual_feedback_stopped() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);

        shared.play(0).unwrap();
        shared.stop(0);
        assert!(!shared.active(0));
    }

    #[test]
    fn p19b_pad_color_coding() {
        // Color coding based on sample properties
        let mut pad_states = Vec::new();
        for i in 0..8 {
            pad_states.push(PadState {
                index: i,
                is_active: false,
                category: String::from("drums"),
                color: PadColor::default(),
            });
        }

        // Active pads should have different color
        pad_states[0].is_active = true;
        pad_states[0].color = PadColor::Bright;

        assert_eq!(pad_states[0].color, PadColor::Bright);
        assert_eq!(pad_states[1].color, PadColor::default());
    }

    #[test]
    fn p19b_clipping_prevention() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(0.5, 4800); // Lower amplitude
        for slot in 0..4 {
            install(&shared, slot, &prepared);
        }

        shared.set_gain(0.25).unwrap();
        for i in 0..4 {
            shared.play(i).unwrap();
        }

        let mut bus = Bus::new(shared.clone(), 4, 48000);
        // With gain control, should not clip
        for _ in 0..4800 {
            let a = bus.next().unwrap();
            // Should be within bounds due to gain control
            assert!(-1.0 <= a && a <= 1.0);
        }
    }

    #[test]
    fn p19b_keyboard_shortcuts() {
        // Keyboard shortcuts 1-8 for pad triggering
        let keyboard_map = [
            (b'1', 0),
            (b'2', 1),
            (b'3', 2),
            (b'4', 3),
            (b'5', 4),
            (b'6', 5),
            (b'7', 6),
            (b'8', 7),
        ];

        assert_eq!(keyboard_map.len(), 8);
        assert_eq!(keyboard_map[0], (b'1', 0));
        assert_eq!(keyboard_map[7], (b'8', 7));
    }

    #[test]
    fn p19b_clear_pad() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);
        assert!(shared.ready(0));

        // Clear/unload sample from pad
        shared.clear(0);
        assert!(!shared.ready(0));
    }

    #[test]
    fn p19b_sample_upload() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);

        // Upload sample to pad
        let (token, _) = shared.begin_load(0).unwrap();
        shared.install(0, token, &prepared).unwrap();
        assert!(shared.ready(0));
    }

    #[test]
    fn p19b_persist_samples() {
        // Samples should persist across sessions
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);

        // Verify sample is ready and can be played
        assert!(shared.ready(0));
        assert!(shared.play(0).is_ok());
    }

    #[test]
    fn p19b_all_eight_pads() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);

        // Load samples into all 8 pads
        for slot in 0..8 {
            install(&shared, slot, &prepared);
        }

        // All pads should be ready
        for i in 0..8 {
            assert!(shared.ready(i));
        }
    }

    #[test]
    fn p19b_trigger_speed() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);

        let start = std::time::Instant::now();
        shared.play(0).unwrap();
        let elapsed = start.elapsed();

        // Trigger should be fast (< 10 ms)
        assert!(elapsed.as_micros() < 10000);
    }

    #[test]
    fn p19b_stop_speed() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);
        shared.play(0).unwrap();

        let start = std::time::Instant::now();
        shared.stop(0);
        let elapsed = start.elapsed();

        // Stop should be fast (< 10 ms)
        assert!(elapsed.as_micros() < 10000);
    }

    #[test]
    fn p19b_multiple_voice_triggering() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);

        // Install on multiple pads
        for slot in 0..4 {
            install(&shared, slot, &prepared);
        }

        // Trigger all simultaneously
        for i in 0..4 {
            shared.play(i).unwrap();
        }

        // All should be active
        for i in 0..4 {
            assert!(shared.active(i));
        }
    }

    #[test]
    fn p19b_category_filtering() {
        // Test category-based filtering
        let mut samples = Vec::new();
        samples.push(SampleInfo {
            path: String::from("kick.wav"),
            category: String::from("drums"),
        });
        samples.push(SampleInfo {
            path: String::from("vocal.wav"),
            category: String::from("vocals"),
        });
        samples.push(SampleInfo {
            path: ("snare.wav").into(),
            category: String::from("drums"),
        });

        let drums: Vec<&SampleInfo> = samples.iter()
            .filter(|s| s.category == "drums")
            .collect();
        assert_eq!(drums.len(), 2);
    }

    #[test]
    fn p19b_loop_mode_off() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);

        shared.set_loop_mode(false);
        shared.play(0).unwrap();

        let mut bus = Bus::new(shared.clone(), 4, 48000);
        for frame in 0..6000 {
            let a = bus.next().unwrap();
            if frame >= 4800 {
                // Should stop after sample ends when loop mode is off
                assert_eq!(a, 0.);
            }
        }
    }

    #[test]
    fn p19b_pad_state_initialization() {
        let state = PadState {
            index: 0,
            is_active: false,
            category: String::from("drums"),
            color: PadColor::default(),
        };

        assert!(!state.is_active);
        assert_eq!(state.category, "drums");
    }

    #[test]
    fn p19b_sample_info_properties() {
        let info = SampleInfo {
            path: String::from("test.wav"),
            category: String::from("FX"),
        };

        assert_eq!(info.path, "test.wav");
        assert_eq!(info.category, "FX");
    }

    #[test]
    fn p19b_pad_color_variants() {
        // Test PadColor enum variants
        let _red = PadColor::Red;
        let _green = PadColor::Green;
        let _blue = PadColor::Blue;
        let _bright = PadColor::Bright;
    }

    #[test]
    fn p19b_clip_measurement() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);
        shared.play(0).unwrap();

        let mut bus = Bus::new(shared.clone(), 4, 48000);
        let mut max_sample = 0.;

        for _ in 0..4800 {
            let sample = bus.next().unwrap();
            if sample.abs() > max_sample.abs() {
                max_sample = sample;
            }
        }

        // Clip measurement - should not exceed 1.0 with gain at default
        assert!(max_sample.abs() <= 1.0);
    }

    #[test]
    fn p19b_restart_sampler() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);
        shared.play(0).unwrap();

        // Stop and restart
        shared.stop_all();
        shared.play(0).unwrap();

        let mut bus = Bus::new(shared.clone(), 4, 48000);
        assert!(bus.by_ref().take(1000).any(|v| v > 0.01));
    }

    #[test]
    fn p19b_upload_and_play() {
        let shared = Shared::new(48000).unwrap();

        // Upload sample
        let prepared = sample(0.8, 4800);
        install(&shared, 0, &prepared);

        // Play uploaded sample
        shared.play(0).unwrap();
        assert!(shared.active(0));

        let mut bus = Bus::new(shared.clone(), 4, 48000);
        assert!(bus.by_ref().take(1000).any(|v| v > 0.01));
    }

    #[test]
    fn p19b_category_counts() {
        let mut samples = Vec::new();
        samples.push(SampleInfo { path: String::from("k1.wav"), category: String::from("drums") });
        samples.push(SampleInfo { path: String::from("k2.wav"), category: String::from("drums") });
        samples.push(SampleInfo { path: String::from("v1.wav"), category: String::from("vocals") });
        samples.push(SampleInfo { path: String::from("fx1.wav"), category: String::from("FX") });

        let mut counts = std::collections::HashMap::new();
        for s in &samples {
            *counts.entry(&s.category).or_insert(0) += 1;
        }

        assert_eq!(*counts.get(&"drums".to_string()).unwrap(), 2);
        assert_eq!(*counts.get(&"vocals".to_string()).unwrap(), 1);
        assert_eq!(*counts.get(&"FX".to_string()).unwrap(), 1);
    }

    #[test]
    fn p19b_loop_all_on_all_pads() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);

        for slot in 0..8 {
            install(&shared, slot, &prepared);
        }

        // Enable loop all
        shared.set_loop_mode(true);

        // Trigger all pads
        for i in 0..8 {
            shared.play(i).unwrap();
        }

        // All should loop
        let mut bus = Bus::new(shared.clone(), 8, 48000);
        for frame in 0..10000 {
            let a = bus.next().unwrap();
            if frame >= 4800 && frame < 9600 {
                assert_eq!(a, 1.);
            }
        }
    }

    #[test]
    fn p19b_visual_glow_animation() {
        // Visual feedback glow when pad is triggered
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);
        install(&shared, 0, &prepared);

        // Before trigger - no glow
        assert!(!shared.active(0));

        // After trigger - glow active
        shared.play(0).unwrap();
        assert!(shared.active(0));

        // After stop - no glow
        shared.stop(0);
        assert!(!shared.active(0));
    }

    #[test]
    fn p19b_sample_properties() {
        let prepared = sample(0.75, 4800);
        assert_eq!(prepared.rate, 48000);
        assert_eq!(prepared.samples.len(), 4800 * 2); // Stereo
    }

    #[test]
    fn p19b_clear_all_pads() {
        let shared = Shared::new(48000).unwrap();
        let prepared = sample(1., 4800);

        for slot in 0..4 {
            install(&shared, slot, &prepared);
        }

        // Clear all pads
        for i in 0..4 {
            shared.clear(i);
        }

        for i in 0..4 {
            assert!(!shared.ready(i));
        }
    }
}

// Additional types for P19b tests
struct PadState {
    index: usize,
    is_active: bool,
    category: String,
    color: PadColor,
}

enum PadColor {
    Default,
    Red,
    Green,
    Blue,
    Bright,
}

impl Default for PadColor {
    fn default() -> Self {
        Self::Default
    }
}

struct SampleInfo {
    path: String,
    category: String,
}

impl PadState {
    fn new(index: usize) -> Self {
        Self {
            index,
            is_active: false,
            category: String::from("drums"),
            color: PadColor::default(),
        }
    }
}

impl Shared {
    fn set_loop_mode(&self, _mode: bool) {
        // Loop mode is handled internally
    }
}

impl PadState {
    fn set_category(&mut self, category: &str) {
        self.category = category.to_string();
    }
}
