//! Hardware-free PCM producer -> native decoder -> snapshot tests.
use drdj_dvs::*;
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};
mod support;

enum Command {
    Play(Arc<[i16]>, mpsc::SyncSender<()>),
    Pause(mpsc::SyncSender<()>),
    Stop,
}
struct Pump {
    commands: mpsc::Sender<Command>,
    thread: Option<thread::JoinHandle<()>>,
}
impl InputStream for Pump {
    fn start(&mut self) -> Result<(), String> {
        Ok(())
    }
}
impl Drop for Pump {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
struct Control {
    commands: mpsc::Sender<Command>,
    submitted_frames: Arc<AtomicU64>,
    failure: StreamFailure,
}
impl Control {
    fn request(&self, command: impl FnOnce(mpsc::SyncSender<()>) -> Command) {
        let (ack, received) = mpsc::sync_channel(1);
        self.commands.send(command(ack)).unwrap();
        received
            .recv_timeout(Duration::from_secs(3))
            .expect("PCM producer did not acknowledge the phase change");
    }
    fn play(&self, pcm: Arc<[i16]>) {
        assert!(!pcm.is_empty() && pcm.len().is_multiple_of(2));
        self.request(|ack| Command::Play(pcm, ack));
    }
    fn pause(&self) {
        self.request(Command::Pause);
    }
    fn check(&self, state: &InputSnapshot, previous: Option<&InputSnapshot>) {
        assert!(state.lost_frames <= self.submitted_frames.load(Ordering::Acquire));
        if let Some(previous) = previous {
            assert!(state.revision >= previous.revision);
            assert!(state.lost_frames >= previous.lost_frames);
            assert!(state.epoch >= previous.epoch);
            if state.lost_frames > previous.lost_frames {
                assert!(state.epoch > previous.epoch);
            }
        } else if state.lost_frames > 0 {
            assert!(state.epoch > 1);
        }
    }
}
fn capture() -> (InputCapture, Control) {
    let (commands, receiver) = mpsc::channel();
    let (created_tx, created_rx) = mpsc::channel();
    let send = commands.clone();
    let submitted_frames = Arc::new(AtomicU64::new(0));
    let submitted = Arc::clone(&submitted_frames);
    let capture = InputCapture::spawn(
        48000,
        Record::SeratoSecondEditionA,
        false,
        move |mut producer| {
            created_tx.send(producer.failure()).unwrap();
            let thread = thread::spawn(move || {
                let mut playing: Option<Arc<[i16]>> = None;
                let mut offset = 0;
                // Keep input alive while a phase is being observed, as a device
                // would. Snapshot revisions are not acknowledgements of PCM.
                let period = Duration::from_nanos(256 * 1_000_000_000 / 48000);
                loop {
                    let command = if playing.is_some() {
                        match receiver.recv_timeout(period) {
                            Ok(command) => Some(command),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match receiver.recv() {
                            Ok(command) => Some(command),
                            Err(_) => break,
                        }
                    };
                    match command {
                        Some(Command::Play(pcm, ack)) => {
                            playing = Some(pcm);
                            offset = 0;
                            let _ = ack.send(());
                        }
                        Some(Command::Pause(ack)) => {
                            playing = None;
                            let _ = ack.send(());
                        }
                        Some(Command::Stop) => break,
                        None => {
                            let pcm = playing.as_ref().unwrap();
                            let end = (offset + 512).min(pcm.len());
                            // Account before publishing frames into the ring.
                            submitted.fetch_add(((end - offset) / 2) as u64, Ordering::Release);
                            producer.stereo(&pcm[offset..end]);
                            offset = if end == pcm.len() { 0 } else { end };
                        }
                    }
                }
            });
            Ok(Pump {
                commands: send,
                thread: Some(thread),
            })
        },
    )
    .unwrap();
    let failure = created_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    (
        capture,
        Control {
            commands,
            submitted_frames,
            failure,
        },
    )
}
fn wait(
    capture: &InputCapture,
    phase: &str,
    mut condition: impl FnMut(&InputSnapshot) -> bool,
) -> Arc<InputSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(state) = capture.try_snapshot() {
            if condition(&state) {
                return state;
            }
        }
        assert!(
            Instant::now() < deadline,
            "Timed out waiting for {phase}; snapshot: {:?}",
            capture.try_snapshot()
        );
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn worker_decodes_stereo_and_loss_remains_visible_after_reacquisition() {
    let pcm: Arc<[i16]> = support::signal(Record::SeratoSecondEditionA, 48000, 3, 1.0, 1.0).into();
    let (capture, control) = capture();
    control.play(Arc::clone(&pcm));
    let tracking = wait(&capture, "initial tracking", |state| {
        matches!(state.state, InputState::Tracking(_))
    });
    control.check(&tracking, None);
    control.play(vec![0; 512].into());
    let lost = wait(&capture, "loss during silence", |state| {
        state.epoch > tracking.epoch && matches!(state.state, InputState::SignalLost)
    });
    control.check(&lost, Some(&tracking));
    control.play(pcm);
    let reacquired = wait(&capture, "tracking after loss", |state| {
        state.revision > lost.revision && matches!(state.state, InputState::Tracking(_))
    });
    // Further legitimate 20ms stale-input drops must remain observable too.
    control.check(&reacquired, Some(&lost));
    control.failure.signal();
    let failed = wait(&capture, "stream failure", |state| {
        matches!(state.state, InputState::Failed(_))
    });
    assert!(matches!(failed.state, InputState::Failed(_)));
}
#[test]
fn stalled_pcm_producer_changes_tracking_to_signal_lost() {
    let pcm = support::signal(Record::SeratoSecondEditionA, 48000, 3, 1.0, 1.0);
    let (capture, control) = capture();
    control.play(pcm.into());
    let tracking = wait(&capture, "tracking before producer pause", |state| {
        matches!(state.state, InputState::Tracking(_))
    });
    control.check(&tracking, None);
    control.pause();
    let lost = wait(&capture, "loss after producer pause", |state| {
        state.epoch > tracking.epoch && matches!(state.state, InputState::SignalLost)
    });
    control.check(&lost, Some(&tracking));
    assert!(matches!(lost.state, InputState::SignalLost));
}
