//! MC4000 input: no engine/UI access from the MIDI callback.
use std::sync::{
    atomic::{AtomicBool, AtomicU8, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
#[derive(Debug, PartialEq)]
pub enum Command {
    Toggle(usize),
    Keylock(usize),
    Cue(usize),
    HeadphoneCue(usize),
    Fader(usize, u8),
    Crossfader(u8),
    FxToggle(usize),
    FxKnob(usize, usize, u8),
    FxTimeStep(usize, i8),
}
pub fn decode(bytes: &[u8]) -> Option<Command> {
    if bytes.len() != 3 || bytes[1] > 127 || bytes[2] > 127 {
        return None;
    }
    let channel = bytes[0] & 15;
    match (bytes[0] & 0xf0, channel, bytes[1], bytes[2]) {
        (0x90, 8..=9, 0, 1..=127) => Some(Command::FxToggle((channel - 8) as usize)),
        (0xb0, 8..=9, 0..=2, value) => Some(Command::FxKnob(
            (channel - 8) as usize,
            bytes[1] as usize,
            value,
        )),
        (0xb0, 8..=9, 3, 1..=127) => Some(Command::FxTimeStep(
            (channel - 8) as usize,
            if bytes[2] < 64 {
                bytes[2] as i8
            } else {
                (i16::from(bytes[2]) - 128) as i8
            },
        )),
        (0x90, 0..=1, 13, 1..=127) => Some(Command::Keylock(channel as usize)),
        (0x90, 0..=1, 0, 1..=127) => Some(Command::Toggle(channel as usize)),
        (0x90, 0..=1, 1, 1..=127) => Some(Command::Cue(channel as usize)),
        (0x90, 0..=1, 27, 1..=127) => Some(Command::HeadphoneCue(channel as usize)),
        (0xb0, 0..=1, 28, value) => Some(Command::Fader(channel as usize, value)),
        (0xb0, 15, 8, value) => Some(Command::Crossfader(value)),
        _ => None,
    }
}
// Manufacturer MIDI table: coloured hot cues use value 1; ordinary LEDs use 127.
const LED_PATH: [[u8; 3]; 16] = [
    [0x94, 0x14, 1],
    [0x94, 0x15, 1],
    [0x94, 0x16, 1],
    [0x94, 0x17, 1],
    [0x9f, 0x21, 127],
    [0x9f, 0x22, 127],
    [0x9f, 0x23, 127],
    [0x9f, 0x24, 127],
    [0x95, 0x17, 1],
    [0x95, 0x16, 1],
    [0x95, 0x15, 1],
    [0x95, 0x14, 1],
    [0x91, 0, 127],
    [0x91, 1, 127],
    [0x90, 1, 127],
    [0x90, 0, 127],
];
const GREETING_STEPS: usize = LED_PATH.len() * 5;
const ALL_LEDS_OFF: [u8; 3] = [0xb0, 0x75, 0];
const ALL_LEDS_ON: [u8; 3] = [0xb0, 0x75, 127];
/// Five circuits in 5.2 seconds. Each step switches the previous LED off first.
struct Greeting {
    started: std::time::Instant,
    last: Option<usize>,
}
impl Greeting {
    fn messages(&mut self, now: std::time::Instant) -> Option<[[u8; 3]; 2]> {
        let step = (now.saturating_duration_since(self.started).as_millis() / 65)
            .min(GREETING_STEPS as u128) as usize;
        if self.last == Some(step) {
            return None;
        }
        let off = self
            .last
            .filter(|&last| last < GREETING_STEPS)
            .map_or(ALL_LEDS_OFF, |last| {
                let mut message = LED_PATH[last % LED_PATH.len()];
                message[2] = 0;
                message
            });
        self.last = Some(step);
        Some([
            off,
            if step < GREETING_STEPS {
                LED_PATH[step % LED_PATH.len()]
            } else {
                ALL_LEDS_ON
            },
        ])
    }
}
pub fn present() -> Result<bool, String> {
    let midi = midir::MidiInput::new("DrDJ discovery").map_err(|e| e.to_string())?;
    Ok(midi.ports().iter().any(|port| {
        midi.port_name(port)
            .is_ok_and(|name| name.trim().eq_ignore_ascii_case("MC4000"))
    }))
}
type TraceRecord = (u64, [u8; 3], usize);
struct InputTrace {
    receiver: Receiver<TraceRecord>,
    overflow: Arc<AtomicBool>,
}
pub struct Input {
    _connection: midir::MidiInputConnection<()>,
    output: Option<midir::MidiOutputConnection>,
    greeting: Greeting,
    buttons: Receiver<Command>,
    levels: Arc<[AtomicU8; 3]>,
    fx_levels: Arc<[AtomicU8; 6]>,
    overflow: Arc<AtomicBool>,
    trace: Option<InputTrace>,
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
        let fx_levels = Arc::new(std::array::from_fn(|_| AtomicU8::new(128)));
        let fx_values = Arc::clone(&fx_levels);
        let overflow = Arc::new(AtomicBool::new(false));
        let failed = overflow.clone();
        let trace_enabled = std::env::var_os("DRDJ_MIDI_TRACE").is_some_and(|v| v == "1");
        let (trace_sender, trace_receiver) = mpsc::sync_channel(128);
        let trace_overflow = Arc::new(AtomicBool::new(false));
        let trace_failed = trace_overflow.clone();
        let connection = midi
            .connect(
                &port,
                "DrDJ MC4000 input",
                move |timestamp, data, _| {
                    if trace_enabled {
                        let mut bytes = [0; 3];
                        let count = data.len().min(3);
                        bytes[..count].copy_from_slice(&data[..count]);
                        if trace_sender
                            .try_send((timestamp, bytes, data.len()))
                            .is_err()
                        {
                            trace_failed.store(true, Ordering::Release);
                        }
                    }
                    if let Some(command) = decode(data) {
                        match command {
                            Command::FxKnob(deck, knob, value) => {
                                fx_values[deck * 3 + knob].store(value, Ordering::Release)
                            }
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
            fx_levels,
            overflow,
            trace: trace_enabled.then_some(InputTrace {
                receiver: trace_receiver,
                overflow: trace_overflow,
            }),
        }))
    }
    pub fn poll(&mut self, mut apply: impl FnMut(Command)) -> bool {
        if let Some(InputTrace { receiver, overflow }) = &self.trace {
            for _ in 0..128 {
                let Ok((timestamp, bytes, len)) = receiver.try_recv() else {
                    break;
                };
                eprintln!(
                    "MC4000 raw us={timestamp} len={len} prefix={bytes:02x?} decoded={:?}",
                    if len == 3 { decode(&bytes) } else { None }
                );
            }
            if overflow.swap(false, Ordering::AcqRel) {
                eprintln!("MC4000 diagnostic queue overflow: raw messages omitted");
            }
        }
        if let Some(messages) = self.greeting.messages(std::time::Instant::now()) {
            if let Some(output) = &mut self.output {
                for message in messages {
                    if output.send(&message).is_err() {
                        self.output = None;
                        break;
                    }
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
        for index in 0..6 {
            let value = self.fx_levels[index].swap(128, Ordering::AcqRel);
            if value < 128 {
                apply(Command::FxKnob(index / 3, index % 3, value));
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
            let _ = output.send(&ALL_LEDS_OFF);
        }
    }
}
pub fn apply(ui: &crate::AppWindow, command: Command) {
    if matches!(
        &command,
        Command::FxToggle(_) | Command::FxKnob(_, _, _) | Command::FxTimeStep(_, _)
    ) && !ui.get_quantize_preferences_ready()
    {
        return;
    }
    match command {
        Command::HeadphoneCue(0) => {
            ui.set_monitor_cue_a(!ui.get_monitor_cue_a());
            ui.invoke_monitor_cue_changed(0);
        }
        Command::HeadphoneCue(1) => {
            ui.set_monitor_cue_b(!ui.get_monitor_cue_b());
            ui.invoke_monitor_cue_changed(1);
        }
        Command::HeadphoneCue(_) => (),
        Command::FxToggle(deck) => {
            if deck == 0 {
                ui.set_fx_active_a(!ui.get_fx_active_a());
            } else {
                ui.set_fx_active_b(!ui.get_fx_active_b());
            }
            ui.invoke_fx_changed(deck as i32);
        }
        Command::FxKnob(deck, knob, value) => {
            let value = f32::from(value) / 127.;
            match (deck, knob) {
                (0, 0) => ui.set_fx_wet_a(value * 100.),
                (_, 0) => ui.set_fx_wet_b(value * 100.),
                (0, 1) => ui.set_fx_feedback_a(value * 85.),
                (_, 1) => ui.set_fx_feedback_b(value * 85.),
                (0, 2) => ui.set_fx_time_a(20. + value * 980.),
                (_, 2) => ui.set_fx_time_b(20. + value * 980.),
                _ => return,
            }
            ui.invoke_fx_changed(deck as i32);
        }
        Command::FxTimeStep(deck, step) => {
            if deck == 0 {
                ui.set_fx_time_a((ui.get_fx_time_a() + f32::from(step) * 10.).clamp(20., 1000.));
            } else {
                ui.set_fx_time_b((ui.get_fx_time_b() + f32::from(step) * 10.).clamp(20., 1000.));
            }
            ui.invoke_fx_changed(deck as i32);
        }
        Command::Keylock(0) => {
            ui.set_left_keylock(!ui.get_left_keylock());
            ui.invoke_keylock_left();
        }
        Command::Keylock(_) => {
            ui.set_right_keylock(!ui.get_right_keylock());
            ui.invoke_keylock_right();
        }
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
    fn headphone_cue_notes_are_deck_scoped_and_releases_do_not_toggle() {
        for deck in 0..=1 {
            assert_eq!(
                decode(&[0x90 | deck, 27, 127]),
                Some(Command::HeadphoneCue(usize::from(deck)))
            );
            assert_eq!(decode(&[0x90 | deck, 27, 0]), None);
            assert_eq!(decode(&[0x80 | deck, 27, 64]), None);
            assert_eq!(decode(&[0xb0 | deck, 27, 127]), None);
            assert_eq!(
                decode(&[0x90 | deck, 1, 127]),
                Some(Command::Cue(usize::from(deck)))
            );
        }
        // Channel 16, note 27 is Load Prep, not either headphone selector.
        assert_eq!(decode(&[0x9f, 27, 127]), None);
        assert_eq!(decode(&[0x92, 27, 127]), None);
    }
    #[test]
    fn transport_press_release_sequences_dispatch_once_per_deck() {
        use std::{cell::Cell, rc::Rc};
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        let counts = Rc::new([Cell::new(0), Cell::new(0)]);
        let a = counts.clone();
        ui.on_toggle_left(move || a[0].set(a[0].get() + 1));
        let b = counts.clone();
        ui.on_toggle_right(move || b[1].set(b[1].get() + 1));
        for channel in 0..=1 {
            for message in [
                [0x90 | channel, 0, 127],
                [0x90 | channel, 0, 0],
                [0x80 | channel, 0, 64],
                [0x90 | channel, 0x32, 127], // unrelated note must not toggle transport
            ] {
                if let Some(command) = decode(&message) {
                    apply(&ui, command);
                }
            }
        }
        assert_eq!([counts[0].get(), counts[1].get()], [1, 1]);
    }
    #[test]
    fn fx_banks_decode_and_reach_separate_gui_parameters() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        ui.set_quantize_preferences_ready(true);
        assert_eq!(decode(&[0x98, 0, 127]), Some(Command::FxToggle(0)));
        assert_eq!(decode(&[0xb9, 3, 127]), Some(Command::FxTimeStep(1, -1)));
        assert_eq!(decode(&[0xb8, 3, 2]), Some(Command::FxTimeStep(0, 2)));
        assert_eq!(decode(&[0x98, 0, 0]), None);
        assert_eq!(decode(&[0x98, 11, 127]), None);
        apply(&ui, decode(&[0x98, 0, 127]).unwrap());
        assert!(ui.get_fx_active_a());
        assert!(!ui.get_fx_active_b());
        apply(&ui, decode(&[0xb9, 0, 127]).unwrap());
        assert_eq!(ui.get_fx_wet_b(), 100.);
        apply(&ui, decode(&[0xb8, 1, 127]).unwrap());
        assert_eq!(ui.get_fx_feedback_a(), 85.);
        apply(&ui, decode(&[0xb9, 2, 127]).unwrap());
        assert_eq!(ui.get_fx_time_b(), 1000.);
        apply(&ui, decode(&[0xb9, 3, 127]).unwrap());
        assert_eq!(ui.get_fx_time_b(), 990.);
        ui.set_quantize_preferences_ready(false);
        apply(&ui, Command::FxToggle(1));
        assert!(!ui.get_fx_active_b());
    }
    #[test]
    fn greeting_runs_five_circuits_then_keeps_all_leds_on() {
        assert_eq!(LED_PATH[0], [0x94, 0x14, 1]);
        assert_eq!(LED_PATH[4], [0x9f, 0x21, 127]);
        assert_eq!(LED_PATH[8], [0x95, 0x17, 1]);
        let start = std::time::Instant::now();
        let mut greeting = Greeting {
            started: start,
            last: None,
        };
        let mut active = std::collections::HashSet::new();
        for step in 0..GREETING_STEPS {
            let time = start + std::time::Duration::from_millis(step as u64 * 65);
            let messages = greeting.messages(time).unwrap();
            for message in messages {
                if message[0] == 0xb0 {
                    active.clear();
                } else if message[2] == 0 {
                    active.remove(&(message[0], message[1]));
                } else {
                    active.insert((message[0], message[1]));
                }
            }
            assert_eq!(active.len(), 1);
            assert_eq!(messages[1], LED_PATH[step % LED_PATH.len()]);
            assert_eq!(greeting.messages(time), None);
        }
        let messages = greeting
            .messages(start + std::time::Duration::from_millis(5200))
            .unwrap();
        assert_eq!(messages[0][2], 0);
        assert_eq!(messages[1], ALL_LEDS_ON);
        assert_eq!(
            greeting.messages(start + std::time::Duration::from_secs(10)),
            None
        );
        let mut late = Greeting {
            started: start,
            last: None,
        };
        assert_eq!(
            late.messages(start + std::time::Duration::from_secs(10)),
            Some([ALL_LEDS_OFF, ALL_LEDS_ON])
        );
        for time in [11, 60, 3600, 86400] {
            assert_eq!(
                greeting.messages(start + std::time::Duration::from_secs(time)),
                None
            );
            assert_eq!(
                late.messages(start + std::time::Duration::from_secs(time)),
                None
            );
        }
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
