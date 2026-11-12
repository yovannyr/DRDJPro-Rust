//! Explicit standard-MIDI input for the generic remote display. Native handles
//! and their possibly blocking destruction remain on the worker.
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct Status {
    pub ports: Vec<String>,
    pub epoch: u64,
    pub connected: bool,
    pub message: String,
}
pub struct Packet {
    pub epoch: u64,
    pub received: Instant,
    pub bytes: [u8; 3],
    pub len: usize,
}
enum Command {
    Refresh,
    Open(u64, String),
    Stop(u64),
}
pub struct MidiPads {
    commands: SyncSender<Command>,
    pub events: Arc<crossbeam::queue::ArrayQueue<Packet>>,
    status: Arc<Mutex<Status>>,
    pub overflow: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    epoch: u64,
}
fn names(input: &midir::MidiInput) -> Result<Vec<String>, String> {
    names_for(input, &input.ports())
}
fn names_for(
    input: &midir::MidiInput,
    ports: &[midir::MidiInputPort],
) -> Result<Vec<String>, String> {
    ports
        .iter()
        .map(|p| input.port_name(p).map_err(|e| e.to_string()))
        .collect()
}
fn unique_port(names: &[String], selected: &str) -> Result<usize, String> {
    let mut matches = names
        .iter()
        .enumerate()
        .filter(|(_, n)| n.as_str() == selected);
    let Some((index, _)) = matches.next() else {
        return Err("Selected MIDI input is no longer available".into());
    };
    if matches.next().is_some() {
        return Err("Duplicate MIDI input names; selection is ambiguous".into());
    }
    Ok(index)
}
// Called only after the previous native connection/callback has been destroyed.
// A new connection cannot inherit a full queue or the old overflow latch.
fn reset_queue(queue: &crossbeam::queue::ArrayQueue<Packet>, lost: &AtomicBool) {
    while queue.pop().is_some() {}
    lost.store(false, Ordering::Release);
}
impl MidiPads {
    #[cfg(test)]
    pub(crate) fn fixture() -> Self {
        let (commands, _requests) = mpsc::sync_channel(4);
        Self {
            commands,
            events: Arc::new(crossbeam::queue::ArrayQueue::new(256)),
            status: Arc::new(Mutex::new(Status {
                epoch: 1,
                connected: true,
                ..Default::default()
            })),
            overflow: Arc::new(AtomicBool::new(false)),
            stop: Arc::new(AtomicBool::new(false)),
            epoch: 1,
        }
    }
    #[cfg(test)]
    pub(crate) fn fixture_status(&self, value: Status) {
        *self.status.lock().unwrap() = value;
    }
    pub fn start() -> std::io::Result<Self> {
        let (commands, requests) = mpsc::sync_channel(4);
        let events = Arc::new(crossbeam::queue::ArrayQueue::new(256));
        let packets = Arc::clone(&events);
        let status = Arc::new(Mutex::new(Status::default()));
        let output = Arc::clone(&status);
        let overflow = Arc::new(AtomicBool::new(false));
        let lost = Arc::clone(&overflow);
        let stop = Arc::new(AtomicBool::new(false));
        let done = Arc::clone(&stop);
        thread::Builder::new()
            .name("remote-midi".into())
            .spawn(move || {
                let publish = |value: Status| {
                    *output.lock().unwrap_or_else(|e| e.into_inner()) = value;
                };
                let mut state = Status::default();
                let mut connection = None;
                let mut selected = String::new();
                let mut next_scan = Instant::now();
                while !done.load(Ordering::Acquire) {
                    let request = requests.recv_timeout(Duration::from_millis(10));
                    match request {
                        Ok(Command::Stop(epoch)) => {
                            state.epoch = epoch;
                            state.connected = false;
                            state.message = "MIDI pads stopped".into();
                            publish(state.clone());
                            connection = None;
                            selected.clear();
                            reset_queue(&packets, &lost);
                        }
                        Ok(Command::Open(epoch, name)) => {
                            state.epoch = epoch;
                            state.connected = false;
                            state.message = "Opening MIDI input".into();
                            publish(state.clone());
                            connection = None;
                            selected.clear();
                            reset_queue(&packets, &lost);
                            let result = (|| {
                                let mut input = midir::MidiInput::new("DrDJ generic pads")
                                    .map_err(|e| e.to_string())?;
                                input.ignore(midir::Ignore::All);
                                let ports = input.ports();
                                let available = names_for(&input, &ports)?;
                                let index = unique_port(&available, &name)?;
                                state.ports = available;
                                let tx = Arc::clone(&packets);
                                let lost = Arc::clone(&lost);
                                input
                                    .connect(
                                        &ports[index],
                                        "DrDJ generic pad input",
                                        move |_, bytes, _| {
                                            if !(2..=3).contains(&bytes.len()) {
                                                return;
                                            }
                                            let mut data = [0; 3];
                                            data[..bytes.len()].copy_from_slice(bytes);
                                            if tx
                                                .push(Packet {
                                                    epoch,
                                                    received: Instant::now(),
                                                    bytes: data,
                                                    len: bytes.len(),
                                                })
                                                .is_err()
                                            {
                                                lost.store(true, Ordering::Release);
                                            }
                                        },
                                        (),
                                    )
                                    .map_err(|e| e.to_string())
                            })();
                            match result {
                                Ok(value) => {
                                    connection = Some(value);
                                    selected = name;
                                    state.connected = true;
                                    state.message =
                                        "MIDI pads connected; notes 1-8 A, 9-16 B".into();
                                }
                                Err(e) => state.message = e,
                            }
                            publish(state.clone());
                        }
                        Ok(Command::Refresh) => {
                            next_scan = Instant::now();
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if lost.load(Ordering::Acquire) && state.connected {
                        state.connected = false;
                        state.message = "MIDI queue overflow; stopped, reopen explicitly".into();
                        publish(state.clone());
                        connection = None;
                        selected.clear();
                    }
                    if Instant::now() >= next_scan {
                        next_scan = Instant::now() + Duration::from_secs(1);
                        let found = midir::MidiInput::new("DrDJ pad discovery")
                            .map_err(|e| e.to_string())
                            .and_then(|input| names(&input));
                        match found {
                            Ok(ports) => {
                                if state.connected && unique_port(&ports, &selected).is_err() {
                                    state.connected = false;
                                    state.message =
                                        "MIDI input disconnected; reopen explicitly".into();
                                    publish(state.clone());
                                    connection = None;
                                    selected.clear();
                                }
                                state.ports = ports;
                            }
                            Err(e) => {
                                state.connected = false;
                                state.message = format!("MIDI discovery failed: {e}");
                                publish(state.clone());
                                connection = None;
                                selected.clear();
                            }
                        }
                        publish(state.clone());
                    }
                }
                state.connected = false;
                state.message = "MIDI worker stopped".into();
                publish(state);
                drop(connection);
            })?;
        Ok(Self {
            commands,
            events,
            status,
            overflow,
            stop,
            epoch: 0,
        })
    }
    pub fn snapshot(&self) -> Option<Status> {
        self.status.try_lock().ok().map(|s| s.clone())
    }
    pub fn refresh(&self) -> Result<(), String> {
        self.commands
            .try_send(Command::Refresh)
            .map_err(|_| "MIDI worker busy".into())
    }
    pub fn open(&mut self, name: String) -> Result<u64, String> {
        let epoch = self.epoch.checked_add(1).ok_or("MIDI epoch exhausted")?;
        self.commands
            .try_send(Command::Open(epoch, name))
            .map_err(|_| "MIDI worker busy")?;
        self.epoch = epoch;
        Ok(epoch)
    }
    pub fn stop(&mut self) -> Result<u64, String> {
        let epoch = self.epoch.checked_add(1).ok_or("MIDI epoch exhausted")?;
        self.commands
            .try_send(Command::Stop(epoch))
            .map_err(|_| "MIDI worker busy")?;
        self.epoch = epoch;
        Ok(epoch)
    }
}
impl Drop for MidiPads {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_is_exact_and_rejects_duplicate_or_missing_ports() {
        let ports = vec!["A".into(), "B".into(), "A".into()];
        assert_eq!(unique_port(&ports, "B").unwrap(), 1);
        assert!(unique_port(&ports, "A").is_err());
        assert!(unique_port(&ports, "C").is_err());
    }
    #[test]
    fn full_previous_queue_is_cleared_before_new_callback_can_publish() {
        let queue = crossbeam::queue::ArrayQueue::new(256);
        let lost = AtomicBool::new(true);
        for _ in 0..256 {
            assert!(queue
                .push(Packet {
                    epoch: 1,
                    received: Instant::now(),
                    bytes: [0x90, 36, 127],
                    len: 3
                })
                .is_ok());
        }
        reset_queue(&queue, &lost);
        assert!(!lost.load(Ordering::Acquire));
        assert!(queue.is_empty());
        assert!(queue
            .push(Packet {
                epoch: 2,
                received: Instant::now(),
                bytes: [0x90, 36, 127],
                len: 3
            })
            .is_ok());
        assert_eq!(queue.pop().unwrap().epoch, 2);
    }
}
