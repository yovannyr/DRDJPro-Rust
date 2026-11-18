use crate::{
    cache, InferenceBackend, InferenceError, ModelId, ModelRegistry, INPUT_LEN, OUTPUT_LEN,
};
use std::{
    collections::VecDeque,
    fmt, io,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        mpsc, Arc, Condvar, Mutex, TryLockError,
    },
    thread,
};

/// Maximum number of fixed input tensors in one job.
pub const MAX_CHUNKS: usize = 256;
/// Number of waiting jobs, excluding the job owned by the worker.
pub const MAX_PENDING: usize = 8;
const ACTIVE: u8 = 0;
const CANCELLED: u8 = 1;
const COMMITTING: u8 = 2;

/// High priority is selected first. Equal priorities execute in submission order.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub enum Priority {
    Low,
    Normal,
    High,
}

/// Caller-prepared input for the fixed A0 tensor contract.
pub struct InferenceJob {
    pub model: ModelId,
    /// Fingerprint of the source audio supplied by the preparation worker.
    pub audio_sha256: [u8; 32],
    /// Chunk contents encode the preprocessing parameters of this fixed graph.
    pub chunks: Vec<[f32; INPUT_LEN]>,
    pub priority: Priority,
}

/// Submission, worker and result-publication errors.
#[derive(Debug)]
pub enum JobError {
    InvalidModel,
    ModelNotLoaded,
    ModelCorrupt,
    InsufficientRam,
    InvalidInput,
    QueueFull,
    QueueBusy,
    Cancelled,
    WorkerStopped,
    WorkerSetup(io::Error),
    Cache(io::Error),
    Backend(InferenceError),
}

impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for JobError {}

/// Complete output in input order.
#[derive(Debug)]
pub struct JobOutput {
    pub chunks: Vec<[f32; OUTPUT_LEN]>,
    pub from_cache: bool,
}

type Outcome = Result<JobOutput, JobError>;

struct Status {
    phase: AtomicU8,
    completed: AtomicUsize,
}

/// Poll or cancel without waiting for a model call to finish.
pub struct Ticket {
    status: Arc<Status>,
    total: usize,
    rx: Option<mpsc::Receiver<Outcome>>,
}

impl Ticket {
    /// Wins only before final publication starts. The current chunk may finish.
    pub fn cancel(&self) -> bool {
        self.status
            .phase
            .compare_exchange(ACTIVE, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Completed chunks and total. The terminal count is visible only on success.
    pub fn progress(&self) -> (usize, usize) {
        (self.status.completed.load(Ordering::Acquire), self.total)
    }

    /// Read a result once. A disconnected worker is reported explicitly.
    pub fn try_result(&mut self) -> Option<Outcome> {
        let outcome = match self.rx.as_ref()?.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(Err(JobError::WorkerStopped)),
        };
        if outcome.is_some() {
            self.rx = None;
        }
        outcome
    }
}

impl Drop for Ticket {
    fn drop(&mut self) {
        self.cancel();
    }
}

struct Pending {
    job: InferenceJob,
    status: Arc<Status>,
    tx: mpsc::Sender<Outcome>,
}

struct Queue {
    pending: Mutex<VecDeque<Pending>>,
    wake: Condvar,
    stopped: AtomicBool,
    playback_active: AtomicBool,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            pending: Mutex::new(VecDeque::new()),
            wake: Condvar::new(),
            stopped: AtomicBool::new(false),
            playback_active: AtomicBool::new(true),
        }
    }
}

/// One background worker. Reserve a dedicated cache directory per service.
/// Submission uses try_lock and never waits for inference or disk access.
pub struct InferenceService(Arc<Queue>);

impl InferenceService {
    /// Update pacing from the controller or a background owner. New services
    /// start in the protective mode. The worker checks this between chunks.
    pub fn set_playback_active(&self, active: bool) {
        self.0.playback_active.store(active, Ordering::Release);
    }

    /// Start an explicit CPU ORT service with a single intra-op thread.
    #[cfg(feature = "ort")]
    pub fn start(
        registry: ModelRegistry,
        cache: PathBuf,
        runtime: PathBuf,
        ram_budget: u64,
    ) -> Result<Self, JobError> {
        Self::spawn(registry, cache, ram_budget, move |bytes| {
            crate::ort_backend::load_bytes(bytes, crate::Threads(1), Some(&runtime))
        })
    }

    fn spawn(
        registry: ModelRegistry,
        cache: PathBuf,
        ram_budget: u64,
        factory: impl Fn(&[u8]) -> Result<Box<dyn InferenceBackend>, InferenceError> + Send + 'static,
    ) -> Result<Self, JobError> {
        let queue = Arc::new(Queue::default());
        let shared = Arc::clone(&queue);
        thread::Builder::new()
            .name("drdj-inference".into())
            .spawn(move || {
                let _stop_on_exit = StopOnExit(Arc::clone(&shared));
                let priority = lower_worker_priority();
                while let Some(pending) = next(&shared) {
                    let outcome = match &priority {
                        Ok(()) => run(&shared, &registry, &cache, ram_budget, &pending, &factory),
                        Err(error) => Err(JobError::WorkerSetup(io::Error::new(
                            error.kind(),
                            error.to_string(),
                        ))),
                    };
                    // Prevent late cancellation from claiming it changed an already terminal result.
                    let outcome = if pending.status.phase.compare_exchange(
                        ACTIVE,
                        COMMITTING,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    ) == Err(CANCELLED)
                    {
                        Err(JobError::Cancelled)
                    } else {
                        outcome
                    };
                    let _ = pending.tx.send(outcome);
                }
            })
            .map_err(JobError::WorkerSetup)?;
        Ok(Self(queue))
    }

    /// Enqueue or return immediate backpressure. Validation of tensor contents is worker-side.
    pub fn submit(&self, job: InferenceJob) -> Result<Ticket, JobError> {
        let total = job.chunks.len();
        // Bound retained capacity as well as length before accepting caller-owned memory.
        if total == 0
            || total > MAX_CHUNKS
            || job.chunks.capacity() > MAX_CHUNKS
            || job.model.name.len() > 128
            || job.model.name.capacity() > 128
        {
            return Err(JobError::InvalidInput);
        }
        let mut queue = self.0.pending.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => JobError::QueueBusy,
            TryLockError::Poisoned(_) => JobError::WorkerStopped,
        })?;
        if self.0.stopped.load(Ordering::Acquire) {
            return Err(JobError::WorkerStopped);
        }
        if queue.len() >= MAX_PENDING {
            return Err(JobError::QueueFull);
        }
        let status = Arc::new(Status {
            phase: AtomicU8::new(ACTIVE),
            completed: AtomicUsize::new(0),
        });
        let (tx, rx) = mpsc::channel();
        let index = queue
            .iter()
            .position(|pending| pending.job.priority < job.priority)
            .unwrap_or(queue.len());
        queue.insert(
            index,
            Pending {
                job,
                status: Arc::clone(&status),
                tx,
            },
        );
        self.0.wake.notify_one();
        Ok(Ticket {
            status,
            total,
            rx: Some(rx),
        })
    }
}

impl Drop for InferenceService {
    fn drop(&mut self) {
        self.0.stop();
    }
}

impl Queue {
    fn stop(&self) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        self.stopped.store(true, Ordering::Release);
        for job in pending.drain(..) {
            let reason = if job.status.phase.compare_exchange(
                ACTIVE,
                COMMITTING,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) == Err(CANCELLED)
            {
                JobError::Cancelled
            } else {
                JobError::WorkerStopped
            };
            let _ = job.tx.send(Err(reason));
        }
        self.wake.notify_one();
    }
}

struct StopOnExit(Arc<Queue>);
impl Drop for StopOnExit {
    fn drop(&mut self) {
        self.0.stop();
    }
}

fn next(queue: &Queue) -> Option<Pending> {
    let mut pending = queue.pending.lock().ok()?;
    loop {
        if queue.stopped.load(Ordering::Acquire) {
            return None;
        }
        if let Some(job) = pending.pop_front() {
            return Some(job);
        }
        pending = queue.wake.wait(pending).ok()?;
    }
}

fn lower_worker_priority() -> io::Result<()> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::{
            GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL,
        };
        // The pseudo-handle identifies this live worker. No native handle is retained.
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) }
            .map_err(|error| io::Error::other(error.to_string()))?;
    }
    Ok(())
}

fn run(
    queue: &Queue,
    registry: &ModelRegistry,
    root: &std::path::Path,
    ram_budget: u64,
    pending: &Pending,
    factory: &impl Fn(&[u8]) -> Result<Box<dyn InferenceBackend>, InferenceError>,
) -> Outcome {
    let job = &pending.job;
    let status = &pending.status;
    let check = || {
        if status.phase.load(Ordering::Acquire) == CANCELLED
            || queue.stopped.load(Ordering::Acquire)
        {
            Err(JobError::Cancelled)
        } else {
            Ok(())
        }
    };
    check()?;
    let model = registry.get(&job.model)?;
    if job.chunks.iter().flatten().any(|value| !value.is_finite()) {
        return Err(JobError::InvalidInput);
    }
    let key = cache::key(model, &job.audio_sha256, &job.chunks);
    let cached = cache::read(root, &key, job.chunks.len());
    let from_cache = cached.is_some();
    let output = if let Some(cached) = cached {
        cached
    } else {
        if model.minimum_ram_bytes > ram_budget {
            return Err(JobError::InsufficientRam);
        }
        let bytes = model.verified_bytes()?;
        check()?;
        let mut backend = factory(&bytes).map_err(JobError::Backend)?;
        let mut output = Vec::with_capacity(job.chunks.len());
        for (index, chunk) in job.chunks.iter().enumerate() {
            check()?;
            let values = backend.infer(chunk).map_err(JobError::Backend)?;
            check()?;
            let values: [f32; OUTPUT_LEN] = values
                .try_into()
                .map_err(|_| JobError::Backend(InferenceError::InvalidOutput))?;
            if values.iter().any(|value| !value.is_finite()) {
                return Err(JobError::Backend(InferenceError::InvalidOutput));
            }
            output.push(values);
            if index + 1 < job.chunks.len() {
                status.completed.store(index + 1, Ordering::Release);
            }
            if queue.playback_active.load(Ordering::Acquire) {
                // Include the last chunk so consecutive one-chunk jobs are paced.
                thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        output
    };
    {
        // Serialize the publication boundary with worker shutdown.
        let _queue_guard = queue.pending.lock().map_err(|_| JobError::WorkerStopped)?;
        check()?;
        status
            .phase
            .compare_exchange(ACTIVE, COMMITTING, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| JobError::Cancelled)?;
    }
    if !from_cache {
        cache::write(root, &key, &output)?;
    }
    status.completed.store(job.chunks.len(), Ordering::Release);
    Ok(JobOutput {
        chunks: output,
        from_cache,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelSpec, ModelUse};
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        time::{Duration, Instant},
    };

    fn setup(dir: &std::path::Path) -> (ModelRegistry, ModelSpec) {
        let model = ModelSpec {
            id: ModelId {
                name: "test".into(),
                version: 1,
            },
            path: dir.join("test.onnx"),
            sha256: Sha256::digest(b"test model").into(),
            license: "Synthetic fixture".into(),
            allowed_use: ModelUse::SyntheticTest,
            minimum_ram_bytes: 1,
        };
        fs::write(&model.path, b"test model").unwrap();
        let mut registry = ModelRegistry::default();
        registry.register(model.clone()).unwrap();
        (registry, model)
    }

    fn job(model: &ModelSpec, index: u8, priority: Priority) -> InferenceJob {
        InferenceJob {
            model: model.id.clone(),
            audio_sha256: [index; 32],
            chunks: vec![[f32::from(index); INPUT_LEN]; 3],
            priority,
        }
    }

    fn submit(
        service: &InferenceService,
        mut make: impl FnMut() -> InferenceJob,
    ) -> Result<Ticket, JobError> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            match service.submit(make()) {
                Err(JobError::QueueBusy) if Instant::now() < deadline => thread::yield_now(),
                result => return result,
            }
        }
    }

    fn finish(ticket: &mut Ticket) -> Outcome {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut previous = 0;
        loop {
            let (done, total) = ticket.progress();
            assert!((previous..=total).contains(&done));
            previous = done;
            if let Some(outcome) = ticket.try_result() {
                assert_eq!(ticket.progress().0 == total, outcome.is_ok());
                assert!(ticket.try_result().is_none());
                return outcome;
            }
            assert!(Instant::now() < deadline, "worker did not answer");
            thread::sleep(Duration::from_millis(1));
        }
    }

    struct Echo;
    impl InferenceBackend for Echo {
        fn infer(&mut self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
            Ok(vec![input[0]; OUTPUT_LEN])
        }
    }

    #[test]
    fn registry_rejects_duplicates_missing_versions_and_changed_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let (mut registry, mut model) = setup(dir.path());
        assert!(matches!(
            registry.register(model.clone()),
            Err(JobError::InvalidModel)
        ));
        model.id.version += 1;
        assert!(matches!(
            registry.get(&model.id),
            Err(JobError::ModelNotLoaded)
        ));
        fs::write(&model.path, b"changed").unwrap();
        assert!(matches!(
            model.verified_bytes(),
            Err(JobError::ModelCorrupt)
        ));
        fs::remove_file(&model.path).unwrap();
        assert!(matches!(
            model.verified_bytes(),
            Err(JobError::ModelNotLoaded)
        ));
    }

    #[test]
    fn persistent_cache_bypasses_model_and_keys_cover_audio_version_and_parameters() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        let (registry, model) = setup(dir.path());
        let service =
            InferenceService::spawn(registry, root.clone(), 1, |_| Ok(Box::new(Echo))).unwrap();
        let make = || job(&model, 2, Priority::Normal);
        assert!(
            !finish(&mut submit(&service, make).unwrap())
                .unwrap()
                .from_cache
        );
        let original = make();
        let key = cache::key(&model, &original.audio_sha256, &original.chunks);
        let mut versioned = model.clone();
        versioned.id.version += 1;
        assert_ne!(
            key,
            cache::key(&versioned, &original.audio_sha256, &original.chunks)
        );
        assert_ne!(key, cache::key(&model, &[3; 32], &original.chunks));
        let mut changed = original.chunks;
        changed[1][2] = -4.0;
        assert_ne!(key, cache::key(&model, &original.audio_sha256, &changed));

        drop(service);
        let mut registry = ModelRegistry::default();
        registry.register(model.clone()).unwrap();
        fs::remove_file(&model.path).unwrap();
        let service = InferenceService::spawn(registry, root.clone(), 1, |_| {
            panic!("cache hit loaded a model")
        })
        .unwrap();
        let output = finish(&mut submit(&service, make).unwrap()).unwrap();
        assert!(output.from_cache);
        assert_eq!(output.chunks, vec![[2.0; OUTPUT_LEN]; 3]);
        let path = fs::read_dir(root).unwrap().next().unwrap().unwrap().path();
        let mut bytes = fs::read(&path).unwrap();
        bytes[64] ^= 1;
        fs::write(path, bytes).unwrap();
        assert!(matches!(
            finish(&mut submit(&service, make).unwrap()),
            Err(JobError::ModelNotLoaded)
        ));
    }

    struct PausedSecondChunk {
        calls: usize,
        entered: mpsc::Sender<()>,
        resume: mpsc::Receiver<()>,
    }
    impl InferenceBackend for PausedSecondChunk {
        fn infer(&mut self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
            self.calls += 1;
            if self.calls == 2 {
                self.entered.send(()).unwrap();
                self.resume.recv_timeout(Duration::from_secs(5)).unwrap();
            }
            Echo.infer(input)
        }
    }

    #[test]
    fn cancellation_during_chunk_leaves_no_result_and_priority_is_stable_under_backpressure() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        let (registry, model) = setup(dir.path());
        let (entered, ready) = mpsc::channel();
        let (resume, paused) = mpsc::channel();
        let pause = Mutex::new(Some(paused));
        let (ordered, execution_order) = mpsc::channel();

        struct Record(mpsc::Sender<u8>, bool);
        impl InferenceBackend for Record {
            fn infer(&mut self, input: &[f32]) -> Result<Vec<f32>, InferenceError> {
                if !self.1 {
                    self.0.send(input[0] as u8).unwrap();
                    self.1 = true;
                }
                Echo.infer(input)
            }
        }

        let service = InferenceService::spawn(registry, root.clone(), 1, move |_| {
            if let Some(resume) = pause.lock().unwrap().take() {
                Ok(Box::new(PausedSecondChunk {
                    calls: 0,
                    entered: entered.clone(),
                    resume,
                }))
            } else {
                Ok(Box::new(Record(ordered.clone(), false)))
            }
        })
        .unwrap();
        let mut current = submit(&service, || job(&model, 0, Priority::Normal)).unwrap();
        ready.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(current.progress(), (1, 3));
        let specs = [
            Priority::Low,
            Priority::High,
            Priority::Normal,
            Priority::High,
            Priority::Low,
            Priority::Low,
            Priority::Low,
            Priority::Low,
        ];
        let mut queued: Vec<_> = specs
            .into_iter()
            .enumerate()
            .map(|(i, priority)| submit(&service, || job(&model, i as u8 + 1, priority)).unwrap())
            .collect();
        assert!(matches!(
            submit(&service, || job(&model, 9, Priority::High)),
            Err(JobError::QueueFull)
        ));
        assert!(current.cancel());
        resume.send(()).unwrap();
        assert!(matches!(finish(&mut current), Err(JobError::Cancelled)));
        assert_eq!(current.progress(), (1, 3));
        for ticket in &mut queued {
            finish(ticket).unwrap();
        }
        let order: Vec<_> = execution_order.try_iter().collect();
        assert_eq!(order, [2, 4, 3, 1, 5, 6, 7, 8]);
        let cancelled = job(&model, 0, Priority::Normal);
        assert!(cache::read(
            &root,
            &cache::key(&model, &cancelled.audio_sha256, &cancelled.chunks),
            3
        )
        .is_none());
    }

    #[test]
    fn failed_cache_publication_never_reports_completion() {
        let dir = tempfile::tempdir().unwrap();
        let (registry, model) = setup(dir.path());
        let service =
            InferenceService::spawn(registry, model.path.clone(), 1, |_| Ok(Box::new(Echo)))
                .unwrap();
        let mut ticket = submit(&service, || job(&model, 1, Priority::Normal)).unwrap();
        assert!(matches!(finish(&mut ticket), Err(JobError::Cache(_))));
        assert_eq!(ticket.progress(), (2, 3));
    }

    #[test]
    fn submission_reports_contention_and_ram_admission_does_not_load_model() {
        let dir = tempfile::tempdir().unwrap();
        let (registry, model) = setup(dir.path());
        let service = InferenceService::spawn(registry, dir.path().join("cache"), 0, |_| {
            panic!("must reject insufficient RAM before loading")
        })
        .unwrap();
        let make = || job(&model, 1, Priority::Normal);
        {
            let _guard = service.0.pending.lock().unwrap();
            assert!(matches!(service.submit(make()), Err(JobError::QueueBusy)));
        }
        let mut ticket = submit(&service, make).unwrap();
        assert!(matches!(
            finish(&mut ticket),
            Err(JobError::InsufficientRam)
        ));
        assert_eq!(ticket.progress(), (0, 3));
    }
}
