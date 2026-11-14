//! Hardware-free PCM producer -> native decoder -> snapshot tests.
use drdj_dvs::*;
use std::{
    cell::Cell,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
mod support;

enum Command {
    Pcm(Vec<i16>),
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
fn wait(
    capture: &InputCapture,
    mut condition: impl FnMut(&InputSnapshot) -> bool,
) -> std::sync::Arc<InputSnapshot> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        if let Some(state) = capture.try_snapshot() {
            if condition(&state) {
                return state;
            }
        }
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn worker_decodes_stereo_and_loss_remains_visible_after_reacquisition() {
    // Prepare data before starting capture so fixture generation cannot be
    // mistaken for a stalled live producer.
    let pcm = support::signal(Record::SeratoSecondEditionA, 48000, 3, 1.0, 1.0);
    let (commands, receiver) = mpsc::channel();
    let (created_tx, created_rx) = mpsc::channel();
    let send = commands.clone();
    let capture = InputCapture::spawn(
        48000,
        Record::SeratoSecondEditionA,
        false,
        move |mut producer| {
            created_tx.send(producer.failure()).unwrap();
            let thread = thread::spawn(move || {
                while let Ok(command) = receiver.recv() {
                    match command {
                        Command::Pcm(pcm) => producer.stereo(&pcm),
                        Command::Stop => break,
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
    let mut revision = 0;
    let submitted_frames = Cell::new(0u64);
    let mut observed_loss = 0;
    let mut observed_epoch = 1;
    let mut send_pcm = |pcm: &[i16]| {
        submitted_frames.set(submitted_frames.get() + (pcm.len() / 2) as u64);
        commands.send(Command::Pcm(pcm.to_vec())).unwrap();
        // Progress can be an idle transition or a partial drain, not an ack for
        // this command. Each phase below waits for its observable state too.
        let state = wait(&capture, |state| state.revision > revision);
        revision = state.revision;
        assert!(state.lost_frames >= observed_loss);
        assert!(state.lost_frames <= submitted_frames.get());
        assert!(state.epoch >= observed_epoch);
        if state.lost_frames > observed_loss {
            assert!(state.epoch > observed_epoch);
        }
        observed_loss = state.lost_frames;
        observed_epoch = state.epoch;
        state
    };
    for block in pcm.chunks(512) {
        send_pcm(block);
    }
    let tracking = wait(&capture, |state| {
        matches!(state.state, InputState::Tracking(_))
    });
    assert!(tracking.lost_frames <= submitted_frames.get());
    if tracking.lost_frames > 0 {
        assert!(tracking.epoch > 1);
    }
    for _ in 0..9 {
        send_pcm(&[0; 512]);
    }
    let lost = wait(&capture, |state| {
        state.epoch > tracking.epoch && matches!(state.state, InputState::SignalLost)
    });
    assert!(lost.lost_frames >= tracking.lost_frames);
    assert!(lost.lost_frames <= submitted_frames.get());
    for block in pcm.chunks(512) {
        send_pcm(block);
    }
    let reacquired = wait(&capture, |state| {
        state.revision > lost.revision && matches!(state.state, InputState::Tracking(_))
    });
    // A loaded host may legitimately cause further 20ms stale-input drops.
    // Such loss must remain reported rather than disappearing on reacquisition.
    assert!(
        reacquired.epoch >= lost.epoch,
        "slow consumers still observe loss through epoch"
    );
    assert!(reacquired.lost_frames >= lost.lost_frames);
    assert!(reacquired.lost_frames <= submitted_frames.get());
    failure.signal();
    assert!(matches!(
        wait(&capture, |state| matches!(
            state.state,
            InputState::Failed(_)
        ))
        .state,
        InputState::Failed(_)
    ));
}
#[test]
fn stalled_pcm_producer_changes_tracking_to_signal_lost() {
    let (commands, receiver) = mpsc::channel();
    let send = commands.clone();
    let capture = InputCapture::spawn(
        48000,
        Record::SeratoSecondEditionA,
        false,
        move |mut producer| {
            let thread = thread::spawn(move || {
                while let Ok(command) = receiver.recv() {
                    match command {
                        Command::Pcm(pcm) => producer.stereo(&pcm),
                        Command::Stop => break,
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
    let mut revision = 0;
    for block in support::signal(Record::SeratoSecondEditionA, 48000, 3, 1.0, 1.0).chunks(512) {
        commands.send(Command::Pcm(block.to_vec())).unwrap();
        revision = wait(&capture, |state| state.revision > revision).revision;
    }
    let tracking = wait(&capture, |state| {
        matches!(state.state, InputState::Tracking(_))
    });
    let lost = wait(&capture, |state| state.epoch > tracking.epoch);
    assert!(matches!(lost.state, InputState::SignalLost));
}
