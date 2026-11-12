//! Hardware-free PCM producer -> native decoder -> snapshot tests.
use drdj_dvs::*;
use std::{
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
    let mut send_pcm = |pcm: &[i16]| {
        commands.send(Command::Pcm(pcm.to_vec())).unwrap();
        let state = wait(&capture, |state| state.revision > revision);
        revision = state.revision;
        state
    };
    let pcm = support::signal(Record::SeratoSecondEditionA, 48000, 3, 1.0, 1.0);
    let mut last = None;
    for block in pcm.chunks(512) {
        last = Some(send_pcm(block));
    }
    let tracking = last.take().unwrap();
    assert!(matches!(tracking.state, InputState::Tracking(_)));
    assert_eq!(tracking.lost_frames, 0);
    for _ in 0..8 {
        send_pcm(&[0; 512]);
    }
    let lost = send_pcm(&[0; 512]);
    assert!(matches!(lost.state, InputState::SignalLost));
    assert!(lost.epoch > tracking.epoch);
    for block in pcm.chunks(512) {
        last = Some(send_pcm(block));
    }
    let reacquired = last.unwrap();
    assert!(matches!(reacquired.state, InputState::Tracking(_)));
    assert_eq!(
        reacquired.epoch, lost.epoch,
        "slow consumers still observe loss through epoch"
    );
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
