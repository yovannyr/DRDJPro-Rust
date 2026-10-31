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
    Monitor(usize, u8),
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
        (0xb0, 15, 12..=13, value) => Some(Command::Monitor(usize::from(bytes[1] - 12), value)),
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
                ALL_LEDS_OFF
            },
        ])
    }
}
const FEEDBACK_MESSAGES: usize = 20;
#[derive(Default)]
struct Feedback {
    last: Option<[[u8; 3]; FEEDBACK_MESSAGES]>,
}
impl Feedback {
    fn send(
        &mut self,
        messages: [[u8; 3]; FEEDBACK_MESSAGES],
        mut send: impl FnMut(&[u8; 3]) -> Result<(), String>,
    ) -> Result<(), String> {
        for (index, message) in messages.iter().enumerate() {
            if self
                .last
                .as_ref()
                .is_none_or(|last| last[index] != *message)
            {
                send(message)?;
            }
        }
        self.last = Some(messages);
        Ok(())
    }
}
fn meter_value(level: f32, clipped: bool, warning: bool) -> u8 {
    if clipped || warning || !level.is_finite() {
        return 127;
    }
    // Manufacturer table: seven segments, ranges 1..18, 19..36, ...109..127.
    let segments = (level.clamp(0., 1.) * 7.).ceil() as u8;
    if segments == 0 {
        0
    } else {
        1 + (segments - 1) * 18
    }
}
fn feedback_messages(ui: &crate::AppWindow) -> [[u8; 3]; FEEDBACK_MESSAGES] {
    use slint::Model;
    let decks = [
        (
            ui.get_left_playing(),
            ui.get_left_loaded(),
            ui.get_left_keylock(),
            ui.get_monitor_cue_a(),
            ui.get_fx_active_a(),
            ui.get_left_hot_cues(),
        ),
        (
            ui.get_right_playing(),
            ui.get_right_loaded(),
            ui.get_right_keylock(),
            ui.get_monitor_cue_b(),
            ui.get_fx_active_b(),
            ui.get_right_hot_cues(),
        ),
    ];
    let mut messages = [[0; 3]; FEEDBACK_MESSAGES];
    for (deck, (playing, loaded, keylock, monitor, fx, cues)) in decks.into_iter().enumerate() {
        let channel = deck as u8;
        for (index, (status, note, active)) in [
            (0x90 | channel, 0, playing),
            (0x90 | channel, 1, loaded && !playing),
            (0x90 | channel, 13, keylock),
            (0x90 | channel, 27, monitor),
            (0x98 | channel, 0, fx),
        ]
        .into_iter()
        .enumerate()
        {
            messages[deck * 9 + index] = [status, note, if active { 127 } else { 0 }];
        }
        for slot in 0..4 {
            let occupied = cues.row_data(slot).is_some_and(|cue| cue.occupied);
            messages[deck * 9 + 5 + slot] = [0x94 | channel, 0x14 + slot as u8, u8::from(occupied)];
        }
    }
    messages[18] = [
        0xbf,
        0x44,
        meter_value(
            ui.get_channel_a_peak_level(),
            ui.get_channel_a_clipped(),
            ui.get_channel_a_peak_warning(),
        ),
    ];
    messages[19] = [
        0xbf,
        0x45,
        meter_value(
            ui.get_channel_b_peak_level(),
            ui.get_channel_b_clipped(),
            ui.get_channel_b_peak_warning(),
        ),
    ];
    messages
}
pub fn present() -> Result<bool, String> {
    let midi = midir::MidiInput::new("DrDJ discovery").map_err(|e| e.to_string())?;
    let names = midi
        .ports()
        .iter()
        .map(|port| midi.port_name(port).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(unique_port(&names)?.is_some())
}
// Keine Auswahl nach Listenreihenfolge: mehrere gleichnamige Controller sind mehrdeutig.
fn unique_port(names: &[String]) -> Result<Option<usize>, String> {
    let mut matching = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.trim().eq_ignore_ascii_case("MC4000"));
    let found = matching.next().map(|(index, _)| index);
    if matching.next().is_some() {
        return Err("Multiple MC4000 ports; connection is ambiguous".into());
    }
    Ok(found)
}
type TraceRecord = (u64, [u8; 3], usize);
// Auch bei unverändertem Portnamen alte Handles und Eingabequeues gezielt freigeben.
pub fn release_for_reconnect<T>(connection: &mut Option<T>, requested: bool) {
    if requested {
        *connection = None;
    }
}
struct InputTrace {
    receiver: Receiver<TraceRecord>,
    overflow: Arc<AtomicBool>,
}
pub struct Input {
    _connection: midir::MidiInputConnection<()>,
    output: Option<midir::MidiOutputConnection>,
    greeting: Greeting,
    feedback: Feedback,
    buttons: Receiver<(std::time::Instant, Command)>,
    accept_buttons_after: std::time::Instant,
    levels: Arc<[AtomicU8; 5]>,
    fx_levels: Arc<[AtomicU8; 6]>,
    overflow: Arc<AtomicBool>,
    trace: Option<InputTrace>,
}
impl Input {
    pub fn connect() -> Result<Option<Self>, String> {
        let mut midi = midir::MidiInput::new("DrDJ MC4000").map_err(|e| e.to_string())?;
        midi.ignore(midir::Ignore::All);
        let ports = midi.ports();
        let names = ports
            .iter()
            .map(|port| midi.port_name(port).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(index) = unique_port(&names)? else {
            return Ok(None);
        };
        let port = &ports[index];
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
                port,
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
                            Command::Monitor(index, value) => {
                                values[3 + index].store(value, Ordering::Release)
                            }
                            command => {
                                if sender
                                    .try_send((std::time::Instant::now(), command))
                                    .is_err()
                                {
                                    failed.store(true, Ordering::Release);
                                }
                            }
                        }
                    }
                },
                (),
            )
            .map_err(|e| e.to_string())?;
        // Ein fehlender LED-Port darf den bereits geöffneten Eingang nicht verwerfen.
        let output = match open_output() {
            Ok(output) => output,
            Err(error) => {
                eprintln!("MC4000 LED output unavailable: {error}; retrying");
                None
            }
        };
        Ok(Some(Self {
            output,
            feedback: Feedback::default(),
            greeting: Greeting {
                started: std::time::Instant::now(),
                last: None,
            },
            _connection: connection,
            buttons,
            accept_buttons_after: std::time::Instant::now(),
            levels,
            fx_levels,
            overflow,
            trace: trace_enabled.then_some(InputTrace {
                receiver: trace_receiver,
                overflow: trace_overflow,
            }),
        }))
    }
    pub fn discard_buttons_before(&mut self, instant: std::time::Instant) {
        self.accept_buttons_after = self.accept_buttons_after.max(instant);
    }
    pub fn refresh_output(&mut self) -> Result<(), String> {
        let available = (|| {
            let midi = midir::MidiOutput::new("DrDJ LED discovery").map_err(|e| e.to_string())?;
            let names = midi
                .ports()
                .iter()
                .map(|port| midi.port_name(port).map_err(|e| e.to_string()))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(unique_port(&names)?.is_some())
        })();
        if reconcile_output(&mut self.output, available, open_output)? {
            self.feedback = Feedback::default();
            self.greeting = Greeting {
                started: std::time::Instant::now(),
                last: None,
            };
        }
        Ok(())
    }
    pub fn leds_connected(&self) -> bool {
        self.output.is_some()
    }
    pub fn sync_feedback(&mut self, ui: &crate::AppWindow) -> Result<(), String> {
        let Some(output) = &mut self.output else {
            return Ok(());
        };
        let now = std::time::Instant::now();
        let result = (|| {
            if let Some(messages) = self.greeting.messages(now) {
                for message in messages {
                    output.send(&message).map_err(|error| error.to_string())?;
                }
            }
            if self.greeting.last == Some(GREETING_STEPS) {
                self.feedback.send(feedback_messages(ui), |message| {
                    output.send(message).map_err(|error| error.to_string())
                })?;
            }
            Ok(())
        })();
        if result.is_err() {
            self.output = None;
            self.feedback = Feedback::default();
        }
        result
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
        // Coalesced levels are applied before transport in this UI tick.
        for i in 0..5 {
            let value = self.levels[i].swap(128, Ordering::AcqRel);
            if value < 128 {
                apply(if i >= 3 {
                    Command::Monitor(i - 3, value)
                } else if i == 2 {
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
        dispatch_buttons(&self.buttons, self.accept_buttons_after, &mut apply);
        self.overflow.swap(false, Ordering::AcqRel)
    }
}
fn reconcile_output<T>(
    output: &mut Option<T>,
    available: Result<bool, String>,
    open: impl FnOnce() -> Result<Option<T>, String>,
) -> Result<bool, String> {
    match available {
        Err(error) => {
            *output = None;
            Err(error)
        }
        Ok(false) => {
            *output = None;
            Ok(false)
        }
        Ok(true) if output.is_some() => Ok(false),
        Ok(true) => {
            *output = open()?;
            Ok(output.is_some())
        }
    }
}
fn open_output() -> Result<Option<midir::MidiOutputConnection>, String> {
    let midi = midir::MidiOutput::new("DrDJ MC4000 LEDs").map_err(|e| e.to_string())?;
    let ports = midi.ports();
    let names = ports
        .iter()
        .map(|port| midi.port_name(port).map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let Some(index) = unique_port(&names)? else {
        return Ok(None);
    };
    midi.connect(&ports[index], "DrDJ MC4000 LEDs")
        .map(Some)
        .map_err(|e| e.to_string())
}
fn dispatch_buttons(
    buttons: &Receiver<(std::time::Instant, Command)>,
    accept_after: std::time::Instant,
    mut apply: impl FnMut(Command),
) {
    for _ in 0..128 {
        match buttons.try_recv() {
            Ok((received, command)) if received >= accept_after => apply(command),
            Ok(_) => (),
            Err(_) => break,
        }
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
        Command::Monitor(index, value) => {
            match index {
                0 => ui.set_monitor_gain(f32::from(value) * 100. / 127.),
                1 => ui.set_monitor_mix(f32::from(value) * 100. / 127.),
                _ => return,
            }
            ui.invoke_monitor_changed();
        }
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
    fn explicit_reconnect_releases_old_handles_without_observed_absence() {
        let old = std::rc::Rc::new(());
        let weak = std::rc::Rc::downgrade(&old);
        let mut connection = Some(old);
        release_for_reconnect(&mut connection, false);
        assert!(weak.upgrade().is_some());
        release_for_reconnect(&mut connection, true);
        assert!(weak.upgrade().is_none());
        assert!(connection.is_none());
        assert!(
            reconcile_output(&mut connection, Ok(true), || Ok(Some(std::rc::Rc::new(())))).unwrap()
        );
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn hotplug_output_retries_failed_open_and_restarts_only_on_recovery() {
        let mut output = Some(1);
        assert!(!reconcile_output(&mut output, Ok(true), || panic!(
            "existing output must be retained"
        ))
        .unwrap());
        assert!(!reconcile_output(&mut output, Ok(false), || panic!(
            "missing output must not open"
        ))
        .unwrap());
        assert_eq!(output, None);
        assert!(reconcile_output(&mut output, Ok(true), || Err("busy".into())).is_err());
        assert_eq!(output, None);
        assert!(reconcile_output(&mut output, Ok(true), || Ok(Some(2))).unwrap());
        assert_eq!(output, Some(2));
        assert!(
            reconcile_output(&mut output, Err("ambiguous".into()), || panic!(
                "ambiguous output must not open"
            ))
            .is_err()
        );
        assert_eq!(output, None);
        assert!(reconcile_output(&mut output, Ok(true), || Ok(Some(3))).unwrap());
    }
    #[test]
    fn port_selection_rejects_duplicates_instead_of_selecting_by_order() {
        assert_eq!(unique_port(&["Other".into()]).unwrap(), None);
        assert_eq!(
            unique_port(&["Other".into(), " mc4000 ".into()]).unwrap(),
            Some(1)
        );
        assert!(unique_port(&["MC4000".into(), "mc4000".into()]).is_err());
    }
    #[test]
    fn commands_received_during_output_change_do_not_replay_after_commit() {
        let (tx, rx) = mpsc::sync_channel(128);
        let end = std::time::Instant::now();
        tx.send((
            end - std::time::Duration::from_millis(1),
            Command::Toggle(0),
        ))
        .unwrap();
        tx.send((end, Command::Toggle(1))).unwrap();
        let mut applied = Vec::new();
        dispatch_buttons(&rx, end, |command| applied.push(command));
        assert_eq!(applied, vec![Command::Toggle(1)]);
    }
    #[test]
    fn headphone_cue_notes_are_deck_scoped_and_releases_do_not_toggle() {
        assert_eq!(decode(&[0xbf, 12, 0]), Some(Command::Monitor(0, 0)));
        assert_eq!(decode(&[0xbf, 13, 127]), Some(Command::Monitor(1, 127)));
        assert_eq!(decode(&[0xb0, 12, 127]), None);
        assert_eq!(decode(&[0x9f, 12, 127]), None);
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
    fn greeting_runs_five_circuits_then_releases_leds_to_feedback() {
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
        assert_eq!(messages[1], ALL_LEDS_OFF);
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
            Some([ALL_LEDS_OFF, ALL_LEDS_OFF])
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
    fn meter_values_match_all_seven_manufacturer_ranges_and_gui_clip_hold() {
        assert_eq!(meter_value(0., false, false), 0);
        assert_eq!(meter_value(-1., false, false), 0);
        for segments in 1..=7 {
            let level = (segments as f32 - 0.5) / 7.;
            assert_eq!(meter_value(level, false, false), 1 + (segments - 1) * 18);
        }
        assert_eq!(meter_value(1., false, false), 109);
        assert_eq!(meter_value(2., false, false), 109);
        assert_eq!(meter_value(0., true, false), 127);
        assert_eq!(meter_value(0., false, true), 127);
        assert_eq!(meter_value(f32::NAN, false, false), 127);
        assert_eq!(meter_value(f32::INFINITY, false, false), 127);
    }
    #[test]
    fn feedback_uses_main_view_states_and_coalesces_unchanged_ticks() {
        use slint::ModelRc;
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        ui.set_left_loaded(true);
        ui.set_left_playing(true);
        ui.set_right_loaded(true);
        ui.set_left_keylock(true);
        ui.set_monitor_cue_b(true);
        ui.set_fx_active_a(true);
        ui.set_channel_a_peak_level(0.5);
        ui.set_channel_b_peak_level(0.1);
        ui.set_left_hot_cues(ModelRc::new(slint::VecModel::from(vec![
            crate::HotCueRow {
                occupied: true,
                ..Default::default()
            },
            crate::HotCueRow::default(),
            crate::HotCueRow {
                occupied: true,
                ..Default::default()
            },
            crate::HotCueRow::default(),
        ])));
        let messages = feedback_messages(&ui);
        assert_eq!(
            &messages[..5],
            &[
                [0x90, 0, 127],
                [0x90, 1, 0],
                [0x90, 13, 127],
                [0x90, 27, 0],
                [0x98, 0, 127],
            ]
        );
        assert_eq!(
            &messages[5..9],
            &[
                [0x94, 0x14, 1],
                [0x94, 0x15, 0],
                [0x94, 0x16, 1],
                [0x94, 0x17, 0],
            ]
        );
        assert_eq!(
            &messages[9..14],
            &[
                [0x91, 0, 0],
                [0x91, 1, 127],
                [0x91, 13, 0],
                [0x91, 27, 127],
                [0x99, 0, 0],
            ]
        );
        assert_eq!(messages[18], [0xbf, 0x44, 55]);
        assert_eq!(messages[19], [0xbf, 0x45, 1]);
        let mut feedback = Feedback::default();
        let mut sent = Vec::new();
        feedback
            .send(messages, |message| {
                sent.push(*message);
                Ok(())
            })
            .unwrap();
        assert_eq!(sent.len(), FEEDBACK_MESSAGES);
        feedback
            .send(messages, |_| panic!("unchanged tick must not send"))
            .unwrap();

        ui.set_left_playing(false);
        ui.set_channel_a_peak_level(0.);
        ui.set_channel_b_clipped(true);
        let mut changed = Vec::new();
        feedback
            .send(feedback_messages(&ui), |message| {
                changed.push(*message);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            changed,
            vec![
                [0x90, 0, 0],
                [0x90, 1, 127],
                [0xbf, 0x44, 0],
                [0xbf, 0x45, 127],
            ]
        );
        ui.set_channel_b_clipped(false);
        let mut cleared = Vec::new();
        feedback
            .send(feedback_messages(&ui), |message| {
                cleared.push(*message);
                Ok(())
            })
            .unwrap();
        assert_eq!(cleared, vec![[0xbf, 0x45, 1]]);
    }
    #[test]
    fn feedback_failure_does_not_acknowledge_frame_and_reconnect_replays_state() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        let messages = feedback_messages(&ui);
        let mut feedback = Feedback::default();
        let mut count = 0;
        assert!(feedback
            .send(messages, |_| {
                count += 1;
                if count == 3 {
                    Err("disconnected".into())
                } else {
                    Ok(())
                }
            })
            .is_err());
        assert!(feedback.last.is_none());
        let mut count = 0;
        feedback
            .send(messages, |_| {
                count += 1;
                Ok(())
            })
            .unwrap();
        assert_eq!(count, FEEDBACK_MESSAGES);
        feedback = Feedback::default();
        let start = std::time::Instant::now();
        let mut greeting = Greeting {
            started: start,
            last: None,
        };
        assert_eq!(greeting.messages(start).unwrap()[1], LED_PATH[0]);
        assert_ne!(greeting.last, Some(GREETING_STEPS));
        assert_eq!(
            greeting
                .messages(start + std::time::Duration::from_millis(5200))
                .unwrap()[1],
            ALL_LEDS_OFF
        );
        ui.set_right_playing(true);
        let messages = feedback_messages(&ui);
        let mut sent = Vec::new();
        feedback
            .send(messages, |message| {
                sent.push(*message);
                Ok(())
            })
            .unwrap();
        assert_eq!(sent.len(), FEEDBACK_MESSAGES);
        assert!(sent.contains(&[0x91, 0, 127]));
    }
    #[test]
    fn engine_tick_feedback_matches_main_view_after_gui_and_midi_transport() {
        use std::rc::Rc;
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        let (mixer, mut output) = rodio::mixer::mixer(1, 48_000);
        let engine = Rc::new(crate::playback::DualDeckEngine::from_mixer(
            mixer, 1, 48_000,
        ));
        let track = crate::test_support::wav_file(8192, 48_000);
        engine.load_a(track.path()).unwrap();
        let controller = crate::controller::AudioController::wire(
            &ui,
            engine.clone(),
            Arc::new(|| None),
            Arc::new(|| None),
        );
        ui.invoke_toggle_left();
        output.by_ref().take(4096).for_each(drop);
        controller.tick();
        let messages = feedback_messages(&ui);
        assert_eq!(
            messages[0],
            [0x90, 0, if ui.get_left_playing() { 127 } else { 0 }]
        );
        assert_eq!(
            messages[18][2],
            meter_value(
                ui.get_channel_a_peak_level(),
                ui.get_channel_a_clipped(),
                ui.get_channel_a_peak_warning(),
            )
        );
        assert!(ui.get_channel_a_peak_level() > 0.);
        apply(&ui, Command::Toggle(0));
        controller.tick();
        assert!(!ui.get_left_playing());
        assert_eq!(feedback_messages(&ui)[0], [0x90, 0, 0]);
        controller.close_library().unwrap();
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
