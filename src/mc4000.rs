//! MC4000 input: no engine/UI access from the MIDI callback.
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
#[derive(Debug, PartialEq)]
pub enum Command {
    Toggle(usize),
    Cue(usize),
    Fader(usize, u8),
    Crossfader(u8),
}
pub fn decode(bytes: &[u8]) -> Option<Command> {
    if bytes.len() != 3 || bytes[1] > 127 || bytes[2] > 127 {
        return None;
    }
    let channel = bytes[0] & 15;
    match (bytes[0] & 0xf0, channel, bytes[1], bytes[2]) {
        (0x90, 0..=1, 0, 1..=127) => Some(Command::Toggle(channel as usize)),
        (0x90, 0..=1, 1, 1..=127) => Some(Command::Cue(channel as usize)),
        (0xb0, 0..=1, 28, value) => Some(Command::Fader(channel as usize, value)),
        (0xb0, 15, 8, value) => Some(Command::Crossfader(value)),
        _ => None,
    }
}
/// Three brief flashes, ending off. Late UI ticks skip expired steps.
struct Greeting {
    started: std::time::Instant,
    last: Option<u128>,
}
impl Greeting {
    fn value(&mut self, now: std::time::Instant) -> Option<u8> {
        let step = now
            .saturating_duration_since(self.started)
            .as_millis()
            .min(750)
            / 150;
        if self.last == Some(step) {
            return None;
        }
        self.last = Some(step);
        Some(if step.is_multiple_of(2) { 127 } else { 0 })
    }
}
pub fn present() -> Result<bool, String> {
    let midi = midir::MidiInput::new("DrDJ discovery").map_err(|e| e.to_string())?;
    Ok(midi.ports().iter().any(|port| {
        midi.port_name(port)
            .is_ok_and(|name| name.trim().eq_ignore_ascii_case("MC4000"))
    }))
}
pub struct Input {
    _connection: midir::MidiInputConnection<()>,
    output: Option<midir::MidiOutputConnection>,
    greeting: Greeting,
    buttons: Receiver<Command>,
    levels: Arc<[AtomicU8; 3]>,
    overflow: Arc<AtomicBool>,
}
impl Input {
    pub fn connect() -> Result<Option<Self>, String> {
        let mut midi = midir::MidiInput::new("DrDJ MC4000").map_err(|e| e.to_string())?;
        midi.ignore(midir::Ignore::All);
        let port = midi.ports().into_iter().find(|p| {
            midi.port_name(p)
                .is_ok_and(|name| name.trim().eq_ignore_ascii_case("MC4000"))
        });
        let Some(port) = port else {
            return Ok(None);
        };
        let (sender, buttons) = mpsc::sync_channel(128);
        let levels = Arc::new(std::array::from_fn(|_| AtomicU8::new(128)));
        let values = levels.clone();
        let overflow = Arc::new(AtomicBool::new(false));
        let failed = overflow.clone();
        let connection = midi
            .connect(
                &port,
                "DrDJ MC4000 input",
                move |_, data, _| {
                    if let Some(command) = decode(data) {
                        match command {
                            Command::Fader(deck, value) => {
                                values[deck].store(value, Ordering::Release)
                            }
                            Command::Crossfader(value) => values[2].store(value, Ordering::Release),
                            command => {
                                if sender.try_send(command).is_err() {
                                    failed.store(true, Ordering::Release);
                                }
                            }
                        }
                    }
                },
                (),
            )
            .map_err(|e| e.to_string())?;
        let midi_out = midir::MidiOutput::new("DrDJ MC4000 LEDs").map_err(|e| e.to_string())?;
        let port = midi_out.ports().into_iter().find(|port| {
            midi_out
                .port_name(port)
                .is_ok_and(|name| name.trim().eq_ignore_ascii_case("MC4000"))
        });
        let output = port.and_then(|port| midi_out.connect(&port, "DrDJ MC4000 LEDs").ok());
        Ok(Some(Self {
            output,
            greeting: Greeting {
                started: std::time::Instant::now(),
                last: None,
            },
            _connection: connection,
            buttons,
            levels,
            overflow,
        }))
    }
    pub fn poll(&mut self, mut apply: impl FnMut(Command)) -> bool {
        if let Some(value) = self.greeting.value(std::time::Instant::now()) {
            if let Some(output) = &mut self.output {
                if output.send(&[0xb0, 0x75, value]).is_err() {
                    self.output = None;
                }
            }
        }
        // Coalesced levels are applied before transport in this UI tick.
        for i in 0..3 {
            let value = self.levels[i].swap(128, Ordering::AcqRel);
            if value < 128 {
                apply(if i == 2 {
                    Command::Crossfader(value)
                } else {
                    Command::Fader(i, value)
                });
            }
        }
        for _ in 0..128 {
            match self.buttons.try_recv() {
                Ok(command) => apply(command),
                Err(_) => break,
            }
        }
        self.overflow.swap(false, Ordering::AcqRel)
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        if let Some(output) = &mut self.output {
            let _ = output.send(&[0xb0, 0x75, 0]);
        }
    }
}
pub fn apply(ui: &crate::AppWindow, command: Command) {
    match command {
        Command::Toggle(0) => ui.invoke_toggle_left(),
        Command::Toggle(_) => ui.invoke_toggle_right(),
        Command::Cue(0) => ui.invoke_reset_left(),
        Command::Cue(_) => ui.invoke_reset_right(),
        Command::Fader(0, v) => {
            ui.set_channel_a(f32::from(v) / 127.0);
            ui.invoke_mixer_changed();
        }
        Command::Fader(_, v) => {
            ui.set_channel_b(f32::from(v) / 127.0);
            ui.invoke_mixer_changed();
        }
        Command::Crossfader(v) => {
            ui.set_crossfader(f32::from(v) / 127.0);
            ui.invoke_mixer_changed();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn greeting_has_three_flashes_finishes_off_and_skips_late_steps() {
        let start = std::time::Instant::now();
        let mut greeting = Greeting {
            started: start,
            last: None,
        };
        for (ms, value) in [
            (0, 127),
            (150, 0),
            (300, 127),
            (450, 0),
            (600, 127),
            (750, 0),
        ] {
            assert_eq!(
                greeting.value(start + std::time::Duration::from_millis(ms)),
                Some(value)
            );
            assert_eq!(
                greeting.value(start + std::time::Duration::from_millis(ms)),
                None
            );
        }
        assert_eq!(
            greeting.value(start + std::time::Duration::from_secs(10)),
            None
        );
        let mut late = Greeting {
            started: start,
            last: None,
        };
        assert_eq!(
            late.value(start + std::time::Duration::from_secs(2)),
            Some(0)
        );
    }
    #[test]
    fn mapped_commands_reach_existing_gui_actions_and_mixer_values() {
        use std::{cell::Cell, rc::Rc};
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        let toggles = Rc::new(Cell::new(0));
        let count = toggles.clone();
        ui.on_toggle_left(move || count.set(count.get() + 1));
        let cues = Rc::new(Cell::new(0));
        let count = cues.clone();
        ui.on_reset_right(move || count.set(count.get() + 1));
        apply(&ui, decode(&[0x90, 0, 127]).unwrap());
        apply(&ui, decode(&[0x91, 1, 127]).unwrap());
        assert_eq!(toggles.get(), 1);
        assert_eq!(cues.get(), 1);
        apply(&ui, Command::Fader(0, 0));
        apply(&ui, Command::Fader(1, 127));
        apply(&ui, Command::Crossfader(127));
        assert_eq!(ui.get_channel_a(), 0.0);
        assert_eq!(ui.get_channel_b(), 1.0);
        assert_eq!(ui.get_crossfader(), 1.0);
    }
    #[test]
    #[ignore = "requires attached MC4000"]
    fn attached_mc4000_input_opens_and_closes() {
        let input = Input::connect().unwrap();
        assert!(input.is_some(), "MC4000 input missing");
        drop(input);
    }
    #[test]
    fn manufacturer_messages_ignore_release_shift_and_other_channels() {
        assert_eq!(decode(&[0x90, 0, 127]), Some(Command::Toggle(0)));
        assert_eq!(decode(&[0x91, 0, 127]), Some(Command::Toggle(1)));
        assert_eq!(decode(&[0x91, 1, 127]), Some(Command::Cue(1)));
        assert_eq!(decode(&[0xb1, 28, 63]), Some(Command::Fader(1, 63)));
        assert_eq!(decode(&[0xbf, 8, 127]), Some(Command::Crossfader(127)));
        for data in [
            &[0x91, 0, 0][..],
            &[0x81, 0, 127],
            &[0x92, 0, 127],
            &[0x91, 4, 127],
            &[0xb1, 6, 1],
            &[0x91, 0],
            &[0xbf, 8, 128],
        ] {
            assert_eq!(decode(data), None);
        }
    }
}
