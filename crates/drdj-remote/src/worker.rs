use crate::{render, Frame, Snapshot};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
const PERIOD: Duration = Duration::from_nanos(33_333_333);
struct Shared {
    input: Mutex<Snapshot>,
    output: Mutex<Option<Arc<Frame>>>,
    stop: AtomicBool,
}
/// Dedicated 30Hz latest-state renderer. UI operations never wait on its mutexes.
/// No audio callback receives this object. Drop requests stop without joining UI.
pub struct DisplayWorker {
    shared: Arc<Shared>,
    thread: JoinHandle<()>,
}
impl DisplayWorker {
    pub fn start() -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            input: Mutex::new(Snapshot::default()),
            output: Mutex::new(None),
            stop: AtomicBool::new(false),
        });
        let state = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("remote-display".into())
            .spawn(move || {
                while !state.stop.load(Ordering::Acquire) {
                    let deadline = Instant::now() + PERIOD;
                    let snapshot = state
                        .input
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .clone();
                    let frame = Arc::new(render(&snapshot));
                    *state.output.lock().unwrap_or_else(|e| e.into_inner()) = Some(frame);
                    while !state.stop.load(Ordering::Acquire) && Instant::now() < deadline {
                        thread::park_timeout(deadline.saturating_duration_since(Instant::now()));
                    }
                }
            })?;
        Ok(Self { shared, thread })
    }
    /// False means contention: the caller can publish the newest state next tick.
    pub fn try_submit(&self, mut snapshot: Snapshot) -> bool {
        snapshot.bound_text();
        let Ok(mut input) = self.shared.input.try_lock() else {
            return false;
        };
        *input = snapshot;
        true
    }
    pub fn latest(&self) -> Option<Arc<Frame>> {
        self.shared.output.try_lock().ok()?.clone()
    }
}
impl Drop for DisplayWorker {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        self.thread.thread().unpark();
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worker_publishes_latest_and_never_blocks_on_contended_input() {
        let w = DisplayWorker::start().unwrap();
        let guard = w.shared.input.lock().unwrap();
        assert!(!w.try_submit(Snapshot::default()));
        drop(guard);
        let s = Snapshot {
            title: "LATEST".into(),
            loaded: true,
            ..Default::default()
        };
        let submit_until = Instant::now() + Duration::from_secs(3);
        while !w.try_submit(s.clone()) {
            assert!(Instant::now() < submit_until);
            thread::sleep(Duration::from_millis(1));
        }
        let expected = render(&s);
        let until = Instant::now() + Duration::from_secs(3);
        loop {
            if w.latest().is_some_and(|f| f.pixels == expected.pixels) {
                break;
            }
            assert!(Instant::now() < until);
            thread::sleep(Duration::from_millis(2));
        }
    }
    #[test]
    fn renderer_budget_probe() {
        let s = Snapshot {
            loaded: true,
            title: "LONG TRACK NAME HERE".into(),
            artist: "ARTIST".into(),
            bpm: "128.5".into(),
            key: "8A".into(),
            ..Default::default()
        };
        let mut times = Vec::with_capacity(500);
        for _ in 0..500 {
            let now = Instant::now();
            std::hint::black_box(render(&s));
            times.push(now.elapsed());
        }
        times.sort();
        eprintln!(
            "remote render 128x64 p50={:?} p99={:?}",
            times[250], times[495]
        );
        assert!(
            times[495] < PERIOD,
            "isolated render budget exceeded; not a native refresh qualification"
        );
    }
}
