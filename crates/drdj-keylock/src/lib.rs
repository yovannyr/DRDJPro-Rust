//! Worker-prepared tempo processing with bounded allocation-free render corrections.
pub mod micro;
extern "C" {
    fn drdj_stretch_clip(
        input: *mut f32,
        frames: usize,
        output: *mut f32,
        out_frames: usize,
        channels: i32,
        rate: f32,
    ) -> bool;
}
/// Preserve pitch while changing duration. The caller retains original source coordinates.
pub fn stretch(
    input: &[f32],
    channels: u16,
    rate: u32,
    tempo: f64,
) -> Result<Vec<f32>, &'static str> {
    if !(1..=2).contains(&channels)
        || !(8000..=192000).contains(&rate)
        || !tempo.is_finite()
        || !(0.5..=2.0).contains(&tempo)
        || input.is_empty()
        || !input.len().is_multiple_of(usize::from(channels))
        || input.iter().any(|s| !s.is_finite())
    {
        return Err("Invalid keylock input");
    }
    let frames = input.len() / usize::from(channels);
    let output_frames = (frames as f64 / tempo).round() as usize;
    let samples = output_frames
        .checked_mul(usize::from(channels))
        .ok_or("Keylock size overflow")?;
    if input.len() > 67_108_864 || samples > 67_108_864 {
        return Err("Keylock clip exceeds worker memory budget");
    }
    let mut copy = Vec::new();
    copy.try_reserve_exact(input.len())
        .map_err(|_| "Keylock input allocation failed")?;
    copy.extend_from_slice(input);
    let mut output = Vec::new();
    output
        .try_reserve_exact(samples)
        .map_err(|_| "Keylock output allocation failed")?;
    output.resize(samples, 0.0);
    // SAFETY: validated channel/frame counts match the live, disjoint mutable buffers.
    let ok = unsafe {
        drdj_stretch_clip(
            copy.as_mut_ptr(),
            frames,
            output.as_mut_ptr(),
            output_frames,
            i32::from(channels),
            rate as f32,
        )
    };
    if !ok || output.iter().any(|s| !s.is_finite()) {
        return Err("Keylock processing failed");
    }
    Ok(output)
}
/// Worker-prepared immutable audio with explicit original-source coordinates.
pub struct PreparedKeylock {
    samples: Vec<f32>,
    channels: u16,
    source_frames: usize,
}
impl PreparedKeylock {
    pub fn prepare(
        input: &[f32],
        channels: u16,
        rate: u32,
        tempo: f64,
    ) -> Result<Self, &'static str> {
        let samples = stretch(input, channels, rate, tempo)?;
        Ok(Self {
            samples,
            channels,
            source_frames: input.len() / usize::from(channels),
        })
    }
    pub fn source_frames(&self) -> usize {
        self.source_frames
    }
    pub fn output_frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }
    /// Map source coordinates to the processed buffer, accounting for rounded output length.
    pub fn sample_at_source(&self, frame: f64, channel: u16) -> f32 {
        if !frame.is_finite()
            || frame < 0.0
            || frame >= self.source_frames as f64
            || channel >= self.channels
        {
            return 0.0;
        }
        let position = frame * self.output_frames() as f64 / self.source_frames as f64;
        let first = (position.floor() as usize).min(self.output_frames() - 1);
        let next = (first + 1).min(self.output_frames() - 1);
        let channels = usize::from(self.channels);
        let a = self.samples[first * channels + usize::from(channel)];
        let b = self.samples[next * channels + usize::from(channel)];
        a + (b - a) * (position - first as f64) as f32
    }
}

/// A generation identifies the exact loaded source and requested tempo.
pub struct PreparedResult {
    pub generation: u64,
    pub tempo: f64,
    pub clip: PreparedKeylock,
}
static ACTIVE_JOBS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct WorkerSlot;
impl Drop for WorkerSlot {
    fn drop(&mut self) {
        ACTIVE_JOBS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
    }
}
pub struct Job {
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    finished: std::sync::Arc<std::sync::atomic::AtomicBool>,
    receiver: std::sync::mpsc::Receiver<Result<PreparedResult, &'static str>>,
}
impl Job {
    pub fn start(
        generation: u64,
        input: std::sync::Arc<Vec<f32>>,
        channels: u16,
        rate: u32,
        tempo: f64,
    ) -> Result<Self, std::io::Error> {
        if ACTIVE_JOBS.fetch_add(1, std::sync::atomic::Ordering::AcqRel) >= 2 {
            ACTIVE_JOBS.fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "Two Keylock workers are already active",
            ));
        }
        let slot = WorkerSlot;
        let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let token = cancel.clone();
        let finished = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let done = finished.clone();
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        std::thread::Builder::new()
            .name("keylock-prepare".into())
            .spawn(move || {
                let _slot = slot;
                let result = if token.load(std::sync::atomic::Ordering::Acquire) {
                    Err("Keylock cancelled")
                } else {
                    PreparedKeylock::prepare(&input, channels, rate, tempo).and_then(|clip| {
                        if token.load(std::sync::atomic::Ordering::Acquire) {
                            Err("Keylock cancelled")
                        } else {
                            Ok(PreparedResult {
                                generation,
                                tempo,
                                clip,
                            })
                        }
                    })
                };
                drop(_slot);
                let _ = sender.send(result);
                done.store(true, std::sync::atomic::Ordering::Release);
            })?;
        Ok(Self {
            cancel,
            finished,
            receiver,
        })
    }
    /// True after processing and result publication, without consuming the queued result.
    pub fn is_finished(&self) -> bool {
        self.finished.load(std::sync::atomic::Ordering::Acquire)
    }
    pub fn cancel(&self) {
        self.cancel
            .store(true, std::sync::atomic::Ordering::Release);
    }
    pub fn poll(&self) -> Result<Option<Result<PreparedResult, &'static str>>, &'static str> {
        match self.receiver.try_recv() {
            Ok(result) => Ok(Some(result)),
            Err(std::sync::mpsc::TryRecvError::Empty) => Ok(None),
            Err(std::sync::mpsc::TryRecvError::Disconnected) => Err("Keylock worker disconnected"),
        }
    }
}
impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn worker_preserves_source_generation_and_requested_tempo() {
        let input = std::sync::Arc::new(vec![0.0; 48000]);
        let job = super::Job::start(42, input, 1, 48000, 1.25).unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !job.is_finished() {
            assert!(std::time::Instant::now() < deadline);
            std::thread::yield_now();
        }
        let result = job
            .poll()
            .unwrap()
            .expect("Completion must leave the result queued")
            .unwrap();
        assert_eq!(result.generation, 42);
        assert_eq!(result.tempo, 1.25);
        assert_eq!(result.clip.source_frames(), 48000);
        assert_eq!(result.clip.output_frames(), 38400);
    }
    #[test]
    fn prepared_coordinates_cover_source_end_and_reject_invalid_seeks() {
        let prepared = super::PreparedKeylock {
            samples: vec![0.0, 10.0, 20.0, 30.0],
            channels: 1,
            source_frames: 8,
        };
        assert_eq!(prepared.source_frames(), 8);
        assert_eq!(prepared.output_frames(), 4);
        assert_eq!(prepared.sample_at_source(2.0, 0), 10.0);
        assert_eq!(prepared.sample_at_source(3.0, 0), 15.0);
        assert_eq!(prepared.sample_at_source(7.0, 0), 30.0);
        for position in [8.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(prepared.sample_at_source(position, 0), 0.0);
        }
        assert_eq!(prepared.sample_at_source(2.0, 1), 0.0);
    }
    #[test]
    fn duration_and_pitch_are_independent_in_stereo() {
        let rate = 48000;
        let input: Vec<_> = (0..rate * 2)
            .flat_map(|i| {
                let x = (std::f32::consts::TAU * 440.0 * i as f32 / rate as f32).sin() * 0.2;
                [x, -x]
            })
            .collect();
        for tempo in [0.8, 1.25] {
            let out = super::stretch(&input, 2, rate, tempo).unwrap();
            assert_eq!(
                out.len() / 2,
                (f64::from(rate) * 2.0 / tempo).round() as usize
            );
            let central: Vec<_> = out
                .as_chunks::<2>()
                .0
                .iter()
                .skip(24000)
                .take(24000)
                .map(|s| {
                    assert!((s[0] + s[1]).abs() < 0.01);
                    s[0]
                })
                .collect();
            let hz = central
                .windows(2)
                .filter(|s| s[0] <= 0.0 && s[1] > 0.0)
                .count() as f64
                * 2.0;
            assert!((hz - 440.0).abs() < 3.0, "{hz}");
        }
    }
}
