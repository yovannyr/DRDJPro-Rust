//! MC4000 input: no engine/UI access from the MIDI callback.
use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicU8, Ordering},
    mpsc::{self, Receiver},
    Arc,
};
#[derive(Debug, PartialEq)]
pub enum Command {
    Toggle(usize),
    Keylock(usize),
    Cue(usize, bool),
    TrackStart(usize),
    Stutter(usize),
    HeadphoneCue(usize),
    Monitor(usize, u8),
    Fader(usize, u8),
    Crossfader(u8),
    FxToggle(usize),
    FxKnob(usize, usize, u8),
    FxTimeStep(usize, i8),
    Jog(usize, i8),
    PlatterTouch(usize, bool),
    Vinyl(usize),
    EqKnob(usize, usize, u8),
    Shift(bool),
    PitchRange(usize),
    PitchBend(usize, i8, bool),
    PitchMsb(usize, u8),
    PitchLsb(usize, u8),
    PitchFader(usize, u16),
    Sync(usize, bool),
    Loop(usize, u8),
    HotCue(usize, usize, bool, bool),
    Browse(i32),
    BrowserAction(u8),
    Load(usize),
}
pub fn decode(bytes: &[u8]) -> Option<Command> {
    if bytes.len() != 3 || bytes[1] > 127 || bytes[2] > 127 {
        return None;
    }
    let channel = bytes[0] & 15;
    match (bytes[0] & 0xf0, channel, bytes[1], bytes[2]) {
        (0xb0, 0..=1, 22..=26, value) => Some(Command::EqKnob(
            usize::from(channel),
            usize::from(bytes[1] - 22),
            value,
        )),
        (0xb0, 0..=1, 6, 1..=127) => Some(Command::Jog(
            channel as usize,
            if bytes[2] < 64 {
                bytes[2] as i8
            } else {
                (i16::from(bytes[2]) - 128) as i8
            },
        )),
        (0x90, 0..=1, 6, value) => Some(Command::PlatterTouch(channel as usize, value != 0)),
        (0x80, 0..=1, 6, _) => Some(Command::PlatterTouch(channel as usize, false)),
        (0x90, 0..=1, 7, 1..=127) => Some(Command::Vinyl(channel as usize)),
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
        (0x90, 0..=1, 1, value) => Some(Command::Cue(channel as usize, value != 0)),
        (0x80, 0..=1, 1, _) => Some(Command::Cue(channel as usize, false)),
        (0x90, 0..=1, 5, 1..=127) => Some(Command::TrackStart(channel as usize)),
        (0x90, 0..=1, 4, 1..=127) => Some(Command::Stutter(channel as usize)),
        (0x90, 0..=1, 27, 1..=127) => Some(Command::HeadphoneCue(channel as usize)),
        (0xb0, 0..=1, 28, value) => Some(Command::Fader(channel as usize, value)),
        (0xb0, 15, 8, value) => Some(Command::Crossfader(value)),
        (0xb0, 15, 12..=13, value) => Some(Command::Monitor(usize::from(bytes[1] - 12), value)),
        // S2 modifier and pitch: SHIFT on channel16 (nibble15), range/bend on deck channels.
        (0x90, 15, 50, value) => Some(Command::Shift(value != 0)),
        (0x80, 15, 50, _) => Some(Command::Shift(false)),
        (0x90, 0..=1, 14, 1..=127) => Some(Command::PitchRange(channel as usize)),
        (0x90, 0..=1, 11..=12, value) => Some(Command::PitchBend(
            channel as usize,
            if bytes[1] == 11 { 1 } else { -1 },
            value != 0,
        )),
        (0x80, 0..=1, 11..=12, _) => Some(Command::PitchBend(
            channel as usize,
            if bytes[1] == 11 { 1 } else { -1 },
            false,
        )),
        (0xb0, 0..=1, 41, value) => Some(Command::PitchMsb(channel as usize, value)),
        (0xb0, 0..=1, 9, value) => Some(Command::PitchLsb(channel as usize, value)),
        (0xb0, 15, note @ 0..=1, value @ 1..=127) => {
            let step = if value < 64 {
                i32::from(value)
            } else {
                i32::from(value) - 128
            };
            Some(Command::Browse(step * if note == 1 { 10 } else { 1 }))
        }
        (0x90, 15, note @ (7 | 17 | 31), 1..=127) => Some(Command::BrowserAction(note)),
        (0x90, 15, note @ 2..=3, 1..=127) => Some(Command::Load(usize::from(note - 2))),
        (0x90, 0..=1, 2..=3, 1..=127) => Some(Command::Sync(channel as usize, bytes[1] == 2)),
        (0x90, 4..=5, note @ (50..=53 | 56..=57 | 63), 1..=127) => {
            Some(Command::Loop((channel - 4) as usize, note))
        }
        (status @ (0x90 | 0x80), 4..=5, note @ (20..=23 | 28..=31), value) => {
            let delete = note >= 28;
            Some(Command::HotCue(
                (channel - 4) as usize,
                usize::from(note - if delete { 28 } else { 20 }),
                status == 0x90 && value != 0,
                delete,
            ))
        }
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
const FEEDBACK_MESSAGES: usize = 26;
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
fn pad_color(tint: slint::Color) -> u8 {
    [(1, [0i32, 0, 255]), (2, [255, 0, 0]), (3, [128, 0, 128])]
        .into_iter()
        .min_by_key(|(_, rgb)| {
            [tint.red(), tint.green(), tint.blue()]
                .into_iter()
                .zip(rgb)
                .map(|(component, target)| (i32::from(component) - target).pow(2))
                .sum::<i32>()
        })
        .unwrap()
        .0
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
            let color = cues
                .row_data(slot)
                .filter(|cue| cue.occupied)
                .map_or(0, |cue| pad_color(cue.tint));
            messages[deck * 9 + 5 + slot] = [0x94 | channel, 0x14 + slot as u8, color];
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
    messages[20] = [0x90, 7, if ui.get_left_vinyl() { 127 } else { 0 }];
    messages[21] = [0x91, 7, if ui.get_right_vinyl() { 127 } else { 0 }];
    messages[22] = [0x90, 2, if ui.get_left_sync_active() { 127 } else { 0 }];
    messages[23] = [0x91, 2, if ui.get_right_sync_active() { 127 } else { 0 }];
    messages[24] = [0x94, 50, if ui.get_left_loop_active() { 127 } else { 0 }];
    messages[25] = [0x95, 50, if ui.get_right_loop_active() { 127 } else { 0 }];
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
    eq_levels: Arc<[AtomicU8; 10]>,
    pitch_faders: Arc<[AtomicU16; 2]>,
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
        let eq_levels = Arc::new(std::array::from_fn(|_| AtomicU8::new(128)));
        let eq_values = Arc::clone(&eq_levels);
        let pitch_partial_msb = Arc::new(std::array::from_fn(|_| AtomicU8::new(0)));
        let pitch_partial_msb_values = Arc::clone(&pitch_partial_msb);
        let pitch_partial_lsb = Arc::new(std::array::from_fn(|_| AtomicU8::new(0)));
        let pitch_partial_lsb_values = Arc::clone(&pitch_partial_lsb);
        let pitch_fresh = Arc::new(std::array::from_fn(|_| AtomicU8::new(0)));
        let pitch_fresh_values = Arc::clone(&pitch_fresh);
        let pitch_faders = Arc::new(std::array::from_fn(|_| AtomicU16::new(PITCH_FADER_NONE)));
        let pitch_fader_values = Arc::clone(&pitch_faders);
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
                            Command::EqKnob(deck, knob, value) => {
                                eq_values[deck * 5 + knob].store(value, Ordering::Release)
                            }
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
                            Command::PitchMsb(deck, value) => accumulate_pitch(
                                deck,
                                Half::Msb,
                                value,
                                &pitch_partial_msb_values,
                                &pitch_partial_lsb_values,
                                &pitch_fresh_values,
                                &pitch_fader_values,
                            ),
                            Command::PitchLsb(deck, value) => accumulate_pitch(
                                deck,
                                Half::Lsb,
                                value,
                                &pitch_partial_msb_values,
                                &pitch_partial_lsb_values,
                                &pitch_fresh_values,
                                &pitch_fader_values,
                            ),
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
            eq_levels,
            pitch_faders,
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
        poll_eq_knobs(&self.eq_levels, &mut apply);
        poll_pitch_faders(&self.pitch_faders, &mut apply);
        dispatch_buttons(
            &self.buttons,
            &mut self.accept_buttons_after,
            &self.overflow,
            &mut apply,
        )
    }
}
fn poll_eq_knobs(values: &[AtomicU8; 10], mut apply: impl FnMut(Command)) {
    for (index, value) in values.iter().enumerate() {
        let value = value.swap(128, Ordering::AcqRel);
        if value < 128 {
            apply(Command::EqKnob(index / 5, index % 5, value));
        }
    }
}
/// Sentinel for a pitch-fader slot with no complete 14-bit value yet.
const PITCH_FADER_NONE: u16 = u16::MAX;

/// Which 7-bit half of a 14-bit pitch-fader value a CC message carries.
#[derive(Clone, Copy)]
enum Half {
    Msb,
    Lsb,
}

// Freshness bits for a pitched deck: set when that half arrived, cleared when a
// complete pair is published. A pair is only published when both halves are fresh,
// so a poll landing between the halves of a new fader position never mixes them
// with a stale half of an earlier generation.
const HALF_MSB_FRESH: u8 = 0b01;
const HALF_LSB_FRESH: u8 = 0b10;

/// Accumulate one pitch-fader nibble in the MIDI callback and atomically publish a
/// complete 14-bit value only once both halves of the same fader generation are
/// fresh. A lone half is preserved (not dropped) until its pair arrives, so a poll
/// never sees a half tempo step or a value mixing different generations.
#[allow(clippy::too_many_arguments)]
fn accumulate_pitch(
    deck: usize,
    half: Half,
    value: u8,
    partial_msb: &[AtomicU8; 2],
    partial_lsb: &[AtomicU8; 2],
    fresh: &[AtomicU8; 2],
    published: &[AtomicU16; 2],
) {
    let stored = value + 1; // 1-based; 0 means none so literal 0..=127 all map >= 1.
    match half {
        Half::Msb => partial_msb[deck].store(stored, Ordering::Release),
        Half::Lsb => partial_lsb[deck].store(stored, Ordering::Release),
    }
    let fresh_bit = match half {
        Half::Msb => HALF_MSB_FRESH,
        Half::Lsb => HALF_LSB_FRESH,
    };
    let other_bit = match half {
        Half::Msb => HALF_LSB_FRESH,
        Half::Lsb => HALF_MSB_FRESH,
    };
    let mut state = fresh[deck].load(Ordering::Acquire);
    loop {
        if state & fresh_bit != 0 {
            // This half was already fresh (duplicate message); keep the stored value
            // but do not re-publish, so transient pairs never repeat.
            return;
        }
        let next = state | fresh_bit;
        match fresh[deck].compare_exchange_weak(state, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                if next & other_bit != 0 {
                    // Both halves are fresh: publish the complete value and clear them.
                    let msb = partial_msb[deck].load(Ordering::Acquire);
                    let lsb = partial_lsb[deck].load(Ordering::Acquire);
                    published[deck].store(
                        (u16::from(msb - 1) << 7) | u16::from(lsb - 1),
                        Ordering::Release,
                    );
                    fresh[deck].store(0, Ordering::Release);
                }
                return;
            }
            Err(actual) => state = actual,
        }
    }
}

fn poll_pitch_faders(values: &[AtomicU16; 2], mut apply: impl FnMut(Command)) {
    for (deck, slot) in values.iter().enumerate() {
        let value = slot.swap(PITCH_FADER_NONE, Ordering::AcqRel);
        if value != PITCH_FADER_NONE {
            apply(Command::PitchFader(deck, value));
        }
    }
}

/// Map a 14-bit fader value to a pitch percent across a range. Provisional pending a
/// native MIDI recording: 0 maps to min, 16383 to max, the midpoint to the centre.
pub fn pitch_fader_percent(value: u16, min: f32, max: f32) -> f32 {
    min + (max - min) * f32::from(value) / 16383.
}

fn centered_knob(value: u8, minimum: f32, maximum: f32) -> f32 {
    // Both central MIDI values are neutral; physical detents vary between 63 and 64.
    if value < 63 {
        minimum * f32::from(63 - value) / 63.
    } else if value > 64 {
        maximum * f32::from(value - 64) / 63.
    } else {
        0.
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
    accept_after: &mut std::time::Instant,
    overflow: &AtomicBool,
    mut apply: impl FnMut(Command),
) -> bool {
    for _ in 0..128 {
        match buttons.try_recv() {
            Ok((received, command)) if received >= *accept_after => apply(command),
            Ok(_) => (),
            Err(_) => break,
        }
    }
    let lost = overflow.swap(false, Ordering::AcqRel);
    if lost {
        // The matching release may be lost: never replay a queued old touch after recovery.
        *accept_after = (*accept_after).max(std::time::Instant::now());
    }
    lost
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
        Command::EqKnob(deck, knob, value) => {
            if deck > 1 || knob > 4 || value > 127 {
                ui.set_status("Invalid MC4000 EQ control".into());
                return;
            }
            let parameter = match knob {
                0 => centered_knob(value, -12., 12.),
                4 => centered_knob(value, -1., 1.),
                _ => centered_knob(value, -24., 12.),
            };
            match (deck, knob) {
                (0, 0) => ui.set_eq_trim_a(parameter),
                (0, 1) => ui.set_eq_high_a(parameter),
                (0, 2) => ui.set_eq_mid_a(parameter),
                (0, 3) => ui.set_eq_low_a(parameter),
                (0, 4) => ui.set_eq_filter_a(parameter),
                (1, 0) => ui.set_eq_trim_b(parameter),
                (1, 1) => ui.set_eq_high_b(parameter),
                (1, 2) => ui.set_eq_mid_b(parameter),
                (1, 3) => ui.set_eq_low_b(parameter),
                (1, 4) => ui.set_eq_filter_b(parameter),
                _ => unreachable!(),
            }
            ui.invoke_eq_changed(deck as i32);
        }
        Command::Jog(deck, steps) => ui.invoke_controller_jog(deck as i32, i32::from(steps)),
        Command::PlatterTouch(deck, held) => ui.invoke_controller_platter_touch(deck as i32, held),
        Command::Vinyl(deck) => ui.invoke_controller_vinyl(deck as i32),
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
        Command::Cue(deck, held) => ui.invoke_temporary_cue(deck as i32, held, true),
        Command::TrackStart(0) => ui.invoke_reset_left(),
        Command::TrackStart(_) => ui.invoke_reset_right(),
        Command::Stutter(deck) => ui.invoke_stutter(deck as i32),
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

        Command::Shift(held) => ui.set_controller_shift(held),
        Command::PitchRange(0) => ui.invoke_controller_pitch_range_left(),
        Command::PitchRange(_) => ui.invoke_controller_pitch_range_right(),
        Command::PitchBend(0, direction, held) => {
            ui.invoke_controller_bend_left(i32::from(direction), held)
        }
        Command::PitchBend(_, direction, held) => {
            ui.invoke_controller_bend_right(i32::from(direction), held)
        }
        Command::PitchFader(deck, value) => {
            ui.invoke_controller_pitch_fader(deck as i32, i32::from(value))
        }
        Command::PitchMsb(_, _) | Command::PitchLsb(_, _) => (),
        Command::Sync(deck, enabled) => ui.invoke_controller_sync(deck as i32, enabled),
        Command::Browse(delta) => ui.invoke_controller_browse(delta),
        Command::BrowserAction(action) => ui.invoke_controller_browser_action(i32::from(action)),
        Command::Load(deck) => ui.invoke_controller_browser_action(100 + deck as i32),
        Command::HotCue(deck, slot, held, delete) => {
            ui.invoke_controller_hot_cue(deck as i32, slot as i32, held, delete)
        }
        Command::Loop(deck, note) => match (deck, note) {
            (0, 50) if ui.get_left_loop_active() => ui.invoke_loop_exit_left(),
            (1, 50) if ui.get_right_loop_active() => ui.invoke_loop_exit_right(),
            (0, 50) => ui.invoke_auto_loop_left(),
            (1, 50) => ui.invoke_auto_loop_right(),
            (0, 51) => ui.invoke_reloop_left(),
            (1, 51) => ui.invoke_reloop_right(),
            (0, 52) => ui.invoke_half_loop_left(),
            (1, 52) => ui.invoke_half_loop_right(),
            (0, 53) => ui.invoke_double_loop_left(),
            (1, 53) => ui.invoke_double_loop_right(),
            (0, 56) => ui.invoke_loop_in_left(),
            (1, 56) => ui.invoke_loop_in_right(),
            (0, 57) => ui.invoke_loop_out_left(),
            (1, 57) => ui.invoke_loop_out_right(),
            (_, 63) => ui.invoke_controller_four_bar_loop(deck as i32),
            _ => ui.set_status("Invalid MC4000 loop command".into()),
        },
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pad_decode_release_shift_and_palette() {
        for deck in 0..2 {
            for slot in 0..4 {
                assert_eq!(
                    decode(&[0x94 + deck, 20 + slot, 127]),
                    Some(Command::HotCue(deck as usize, slot as usize, true, false))
                );
                assert_eq!(
                    decode(&[0x84 + deck, 20 + slot, 127]),
                    Some(Command::HotCue(deck as usize, slot as usize, false, false))
                );
                assert_eq!(
                    decode(&[0x94 + deck, 28 + slot, 0]),
                    Some(Command::HotCue(deck as usize, slot as usize, false, true))
                );
            }
        }
        assert_eq!(pad_color(slint::Color::from_rgb_u8(255, 0, 0)), 2);
        assert_eq!(pad_color(slint::Color::from_rgb_u8(0, 0, 255)), 1);
        assert_eq!(pad_color(slint::Color::from_rgb_u8(128, 0, 128)), 3);
    }
    #[test]
    fn pitch_fader_percent_maps_endpoints_and_midpoint() {
        for (min, max) in [(-8., 8.), (-50., 100.)] {
            assert_eq!(pitch_fader_percent(0, min, max), min);
            assert_eq!(pitch_fader_percent(16383, min, max), max);
            let mid = (min + max) / 2.;
            assert!((pitch_fader_percent(8191, min, max) - mid).abs() < 0.01);
            assert!((pitch_fader_percent(8192, min, max) - mid).abs() < 0.01);
        }
    }

    #[test]
    fn pitch_fader_pairs_order_lone_halves_and_abi_isolation() {
        let msb = std::array::from_fn(|_| AtomicU8::new(0));
        let lsb = std::array::from_fn(|_| AtomicU8::new(0));
        let published = std::array::from_fn(|_| AtomicU16::new(PITCH_FADER_NONE));
        let fresh = std::array::from_fn(|_| AtomicU8::new(0));
        let mut emitted = Vec::new();
        // A lone MSB is preserved, never published as a half tempo step.
        accumulate_pitch(0, Half::Msb, 41, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert!(emitted.is_empty());
        // The poll between halves must not drop the preserved MSB: the matching LSB
        // later completes a single coherent pair and yields the 14-bit value.
        accumulate_pitch(0, Half::Lsb, 9, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert_eq!(emitted, vec![Command::PitchFader(0, (41 << 7) | 9)]);
        emitted.clear();
        // No repeated emission without fresh MIDI after the pair is consumed.
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert!(emitted.is_empty());
        // Deck 1 pairs independently (A/B isolation) regardless of arrival order.
        accumulate_pitch(1, Half::Lsb, 1, &msb, &lsb, &fresh, &published);
        accumulate_pitch(1, Half::Msb, 1, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert_eq!(emitted, vec![Command::PitchFader(1, (1 << 7) | 1)]);
        emitted.clear();
        // A lone half published while the other deck's pair is unconsumed stays isolated.
        accumulate_pitch(0, Half::Msb, 100, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert!(emitted.is_empty());
        // Completing deck 0 emits only deck 0; deck 1 is not republished.
        accumulate_pitch(0, Half::Lsb, 3, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert_eq!(emitted, vec![Command::PitchFader(0, (100 << 7) | 3)]);
    }

    #[test]
    fn pitch_fader_poll_between_halves_never_mixes_generations() {
        let msb = std::array::from_fn(|_| AtomicU8::new(0));
        let lsb = std::array::from_fn(|_| AtomicU8::new(0));
        let published = std::array::from_fn(|_| AtomicU16::new(PITCH_FADER_NONE));
        let fresh = std::array::from_fn(|_| AtomicU8::new(0));
        let mut emitted = Vec::new();
        // Generation 1 completes and is consumed by the poll.
        accumulate_pitch(0, Half::Msb, 41, &msb, &lsb, &fresh, &published);
        accumulate_pitch(0, Half::Lsb, 9, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert_eq!(emitted, vec![Command::PitchFader(0, (41 << 7) | 9)]);
        emitted.clear();
        // Generation 2 sends only its MSB now; a poll in between must not surface
        // a partially-updated value (the stale LSB from gen 1 is preserved, not published).
        accumulate_pitch(0, Half::Msb, 80, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert!(emitted.is_empty());
        // Gen 2's LSB completes the pair; only the new coherent value is published.
        accumulate_pitch(0, Half::Lsb, 1, &msb, &lsb, &fresh, &published);
        poll_pitch_faders(&published, |c| emitted.push(c));
        assert_eq!(emitted, vec![Command::PitchFader(0, (80 << 7) | 1)]);
    }
    #[test]
    fn modifier_and_pitch_command_decode() {
        // SHIFT lives only on channel16 (nibble15): press (value>0), release by
        // NoteOn0 or NoteOff.
        assert_eq!(decode(&[0x9f, 50, 127]), Some(Command::Shift(true)));
        assert_eq!(decode(&[0x9f, 50, 0]), Some(Command::Shift(false)));
        assert_eq!(decode(&[0x8f, 50, 0]), Some(Command::Shift(false)));
        assert_ne!(decode(&[0x90, 50, 127]), Some(Command::Shift(true)));
        // Pitch range (Note14) and bend (Note11/12) on the two deck channels.
        assert_eq!(decode(&[0x90, 14, 1]), Some(Command::PitchRange(0)));
        assert_eq!(decode(&[0x91, 14, 127]), Some(Command::PitchRange(1)));
        assert_eq!(
            decode(&[0x90, 11, 127]),
            Some(Command::PitchBend(0, 1, true))
        );
        assert_eq!(
            decode(&[0x91, 12, 127]),
            Some(Command::PitchBend(1, -1, true))
        );
        assert_eq!(
            decode(&[0x80, 11, 0]),
            Some(Command::PitchBend(0, 1, false))
        );
        assert_eq!(
            decode(&[0x81, 12, 0]),
            Some(Command::PitchBend(1, -1, false))
        );
        assert_eq!(
            decode(&[0x80, 12, 0]),
            Some(Command::PitchBend(0, -1, false))
        );
        // 14-bit pitch: CC41 MSB and CC9 LSB on deck channels.
        assert_eq!(decode(&[0xb0, 41, 100]), Some(Command::PitchMsb(0, 100)));
        assert_eq!(decode(&[0xb1, 9, 50]), Some(Command::PitchLsb(1, 50)));
    }

    #[test]
    fn shift_state_applies_to_ui_property() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        assert!(!ui.get_controller_shift());
        apply(&ui, Command::Shift(true));
        assert!(ui.get_controller_shift());
        apply(&ui, Command::Shift(false));
        assert!(!ui.get_controller_shift());
        apply(&ui, Command::Shift(true));
        assert!(ui.get_controller_shift());
        // Lifecycle/overflow resets explicit state rather than leaving a stale hold.
        ui.set_controller_shift(false);
        assert!(!ui.get_controller_shift());
    }
    #[test]
    fn eq_knobs_decode_coalesce_and_map_neutral_and_endpoints() {
        i_slint_backend_testing::init_no_event_loop();
        let ui = crate::AppWindow::new().unwrap();
        for deck in 0..=1 {
            for knob in 0..5 {
                assert_eq!(
                    decode(&[0xb0 | deck, 22 + knob, 64]),
                    Some(Command::EqKnob(deck as usize, knob as usize, 64))
                );
            }
        }
        assert_eq!(decode(&[0xb2, 22, 64]), None);
        assert_eq!(decode(&[0xbf, 26, 64]), None);
        assert_eq!(decode(&[0x90, 22, 64]), None);
        let values = std::array::from_fn(|_| AtomicU8::new(128));
        values[0].store(0, Ordering::Release);
        values[0].store(127, Ordering::Release);
        values[9].store(0, Ordering::Release);
        let mut emitted = Vec::new();
        poll_eq_knobs(&values, |command| {
            if let Command::EqKnob(deck, knob, value) = command {
                apply(&ui, Command::EqKnob(deck, knob, value));
                emitted.push(Command::EqKnob(deck, knob, value));
            } else {
                panic!("Expected EQ command");
            }
        });
        assert_eq!(
            emitted,
            vec![Command::EqKnob(0, 0, 127), Command::EqKnob(1, 4, 0)]
        );
        poll_eq_knobs(&values, |_| panic!("No repeated control without new MIDI"));
        assert_eq!(ui.get_eq_trim_a(), 12.);
        assert_eq!(ui.get_eq_filter_b(), -1.);
        for value in [0, 63, 64, 127] {
            for deck in 0..2 {
                for knob in 0..5 {
                    apply(&ui, Command::EqKnob(deck, knob, value));
                }
            }
            let band = if value == 0 {
                -24.
            } else if value == 127 {
                12.
            } else {
                0.
            };
            let trim = if value == 0 {
                -12.
            } else if value == 127 {
                12.
            } else {
                0.
            };
            let filter = if value == 0 {
                -1.
            } else if value == 127 {
                1.
            } else {
                0.
            };
            assert_eq!(
                [
                    ui.get_eq_high_a(),
                    ui.get_eq_mid_a(),
                    ui.get_eq_low_a(),
                    ui.get_eq_high_b(),
                    ui.get_eq_mid_b(),
                    ui.get_eq_low_b()
                ],
                [band; 6]
            );
            assert_eq!([ui.get_eq_trim_a(), ui.get_eq_trim_b()], [trim; 2]);
            assert_eq!([ui.get_eq_filter_a(), ui.get_eq_filter_b()], [filter; 2]);
        }
    }
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
        let mut end = std::time::Instant::now();
        tx.send((
            end - std::time::Duration::from_millis(1),
            Command::Toggle(0),
        ))
        .unwrap();
        tx.send((end, Command::Toggle(1))).unwrap();
        let mut applied = Vec::new();
        dispatch_buttons(&rx, &mut end, &AtomicBool::new(false), |command| {
            applied.push(command)
        });
        assert_eq!(applied, vec![Command::Toggle(1)]);
    }
    #[test]
    fn overflow_invalidates_queued_touch_beyond_dispatch_limit() {
        let (tx, rx) = mpsc::sync_channel(256);
        let received = std::time::Instant::now();
        let mut cutoff = received;
        for _ in 0..128 {
            tx.send((received, Command::Jog(0, 1))).unwrap();
        }
        tx.send((received, Command::PlatterTouch(0, true))).unwrap();
        let overflow = AtomicBool::new(true);
        assert!(dispatch_buttons(&rx, &mut cutoff, &overflow, |_| {}));
        assert!(cutoff > received);
        let mut held = false; // main's forced-release state
        assert!(!dispatch_buttons(&rx, &mut cutoff, &overflow, |command| {
            if let Command::PlatterTouch(_, value) = command {
                held = value;
            }
        }));
        assert!(
            !held,
            "old touch must not rearm a deck after its release was lost"
        );
        tx.send((std::time::Instant::now(), Command::PlatterTouch(0, true)))
            .unwrap();
        dispatch_buttons(&rx, &mut cutoff, &overflow, |command| {
            if let Command::PlatterTouch(_, value) = command {
                held = value;
            }
        });
        assert!(held, "a fresh press remains usable after recovery");
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
                Some(Command::Cue(usize::from(deck), true))
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
    fn platter_manufacturer_relative_mapping_and_release_messages() {
        for deck in 0..=1 {
            for (value, steps) in [(1, 1), (63, 63), (127, -1), (64, -64)] {
                assert_eq!(
                    decode(&[0xb0 | deck, 6, value]),
                    Some(Command::Jog(deck as usize, steps))
                );
            }
            assert_eq!(decode(&[0xb0 | deck, 6, 0]), None);
            assert_eq!(
                decode(&[0x90 | deck, 6, 127]),
                Some(Command::PlatterTouch(deck as usize, true))
            );
            for message in [
                [0x90 | deck, 6, 0],
                [0x80 | deck, 6, 0],
                [0x80 | deck, 6, 64],
            ] {
                assert_eq!(
                    decode(&message),
                    Some(Command::PlatterTouch(deck as usize, false))
                );
            }
            assert_eq!(
                decode(&[0x90 | deck, 7, 127]),
                Some(Command::Vinyl(deck as usize))
            );
            assert_eq!(decode(&[0x90 | deck, 7, 0]), None);
            assert_eq!(decode(&[0x80 | deck, 7, 0]), None);
        }
        for channel in 2..=15 {
            assert_eq!(decode(&[0xb0 | channel, 6, 1]), None);
            assert_eq!(decode(&[0x90 | channel, 6, 127]), None);
        }
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
                tint: slint::Color::from_rgb_u8(0, 0, 255),
                ..Default::default()
            },
            crate::HotCueRow::default(),
            crate::HotCueRow {
                occupied: true,
                tint: slint::Color::from_rgb_u8(255, 0, 0),
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
                [0x94, 0x16, 2],
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
        assert_eq!(messages[20], [0x90, 7, 127]);
        assert_eq!(messages[21], [0x91, 7, 127]);
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
        ui.on_temporary_cue(move |deck, held, hardware| {
            assert_eq!(deck, 1);
            assert!(held && hardware);
            count.set(count.get() + 1);
        });
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
        assert_eq!(decode(&[0x91, 1, 127]), Some(Command::Cue(1, true)));
        assert_eq!(decode(&[0x91, 4, 127]), Some(Command::Stutter(1)));
        assert_eq!(decode(&[0x91, 5, 127]), Some(Command::TrackStart(1)));
        assert_eq!(decode(&[0x81, 1, 127]), Some(Command::Cue(1, false)));
        assert_eq!(decode(&[0xb1, 28, 63]), Some(Command::Fader(1, 63)));
        assert_eq!(decode(&[0xbf, 8, 127]), Some(Command::Crossfader(127)));
        for data in [
            &[0x91, 0, 0][..],
            &[0x81, 0, 127],
            &[0x92, 0, 127],
            &[0xb2, 6, 1],
            &[0x91, 0],
            &[0xbf, 8, 128],
        ] {
            assert_eq!(decode(data), None);
        }
    }
}
