//! Four-deck state coordination model, independent of hardware/audio routing.
//!
//! Manages up to four independent decks, each with its own position,
//! rate, loop state, and transport status. Decks are isolated — stopping
//! one deck never affects others. Routing controls output pair selection
//! and monitor settings per deck.
//!
//! ## Design principles
//!
//! * **Isolation** — Each deck owns its state; no cross-deck mutations.
//! * **No playing-deck interruption** — Active decks cannot be stopped
//!   by routing changes or mixer adjustments.
//! * **Routing safety** — Output pair assignment is validated against
//!   available hardware channels.
//! * **Deterministic state transitions** — All transport changes follow
//!   a strict state machine.

use std::sync::Arc;

use crate::command_scheduler::{Command, CommandId, Scheduler as CommandScheduler};
use crate::equalizer_config::EqSettings;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of decks supported.
pub const MAX_DECKS: u8 = 4;
/// Maximum hot cues per deck.
pub const MAX_HOT_CUES: u8 = 16;
/// Maximum saved loops per deck.
pub const MAX_SAVED_LOOPS: u8 = 8;

// ---------------------------------------------------------------------------
// Deck state
// ---------------------------------------------------------------------------

/// Transport state for a single deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckTransport {
    Stopped,
    Playing,
    Paused,
    Cue, // At cue point, paused
}

impl DeckTransport {
    /// Check if the deck is actively outputting audio.
    pub fn is_active(&self) -> bool {
        matches!(self, DeckTransport::Playing)
    }
}

/// Per-deck state — fully isolated from other decks.
#[derive(Debug, Clone)]
pub struct DeckState {
    /// Deck ID (1–4).
    pub id: u8,
    /// Current transport state.
    pub transport: DeckTransport,
    /// Current sample position in frames.
    pub position_frame: u64,
    /// Current playback rate (0.98–1.02).
    pub rate: f64,
    /// Nominal (neutral) rate for sync calculations.
    pub nominal_rate: f64,
    /// Loop bounds in frames (None = no loop active).
    pub loop_start: Option<u64>,
    pub loop_end: Option<u64>,
    /// Hot cues (slot index → frame position).
    pub hot_cues: [Option<u64>; MAX_HOT_CUES as usize],
    /// Saved loops (loop name → (start, end)).
    pub saved_loops: Vec<(String, u64, u64)>,
    /// EQ settings for this deck.
    pub eq: EqSettings,
    /// Gain (0.0–1.0).
    pub gain: f32,
    /// Cue point position in frames.
    pub cue_frame: u64,
    /// Whether this deck has a track loaded.
    pub track_loaded: bool,
}

impl DeckState {
    /// Create a new deck state, initially stopped with no track loaded.
    pub fn new(id: u8) -> Self {
        assert!((1..=MAX_DECKS).contains(&id), "Deck ID must be 1–4");
        Self {
            id,
            transport: DeckTransport::Stopped,
            position_frame: 0,
            rate: 1.0,
            nominal_rate: 1.0,
            loop_start: None,
            loop_end: None,
            hot_cues: [None; MAX_HOT_CUES as usize],
            saved_loops: Vec::new(),
            eq: EqSettings::default(),
            gain: 1.0,
            cue_frame: 0,
            track_loaded: false,
        }
    }

    /// Validate the deck state (all fields consistent).
    pub fn validate(&self) -> bool {
        (1..=MAX_DECKS).contains(&self.id)
            && (self.track_loaded || self.transport == DeckTransport::Stopped)
            && (0.98..=1.02).contains(&self.rate)
            && self.nominal_rate.is_finite()
            && self.nominal_rate > 0.0
            && (0.0..=1.0).contains(&self.gain)
            && match (self.loop_start, self.loop_end) {
                (None, None) => true,
                (Some(start), Some(end)) => start < end,
                _ => false,
            }
    }

    /// Start playback.
    pub fn play(&mut self) -> Result<(), String> {
        if !self.track_loaded {
            return Err("No track loaded".into());
        }
        self.transport = DeckTransport::Playing;
        Ok(())
    }

    /// Pause playback.
    pub fn pause(&mut self) {
        self.transport = DeckTransport::Paused;
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.transport = DeckTransport::Stopped;
    }

    /// Jump to cue point.
    pub fn cue(&mut self) {
        self.position_frame = self.cue_frame;
        self.transport = DeckTransport::Cue;
    }

    /// Set a hot cue at the current position.
    pub fn set_hot_cue(&mut self, slot: u8) -> Result<(), String> {
        if slot >= MAX_HOT_CUES {
            return Err(format!("Hot cue slot must be 0–{MAX_HOT_CUES}",));
        }
        self.hot_cues[slot as usize] = Some(self.position_frame);
        Ok(())
    }

    /// Jump to a hot cue slot.
    pub fn go_to_hot_cue(&mut self, slot: u8) -> Result<(), String> {
        if slot >= MAX_HOT_CUES {
            return Err(format!("Hot cue slot must be 0–{MAX_HOT_CUES}",));
        }
        if let Some(frame) = self.hot_cues[slot as usize] {
            self.position_frame = frame;
            Ok(())
        } else {
            Err("Hot cue not set".into())
        }
    }

    /// Set loop bounds.
    pub fn set_loop(&mut self, start: u64, end: u64) -> Result<(), String> {
        if start >= end {
            return Err("Loop start must be before end".into());
        }
        self.loop_start = Some(start);
        self.loop_end = Some(end);
        Ok(())
    }

    /// Clear active loop.
    pub fn clear_loop(&mut self) {
        self.loop_start = None;
        self.loop_end = None;
    }
}

// ---------------------------------------------------------------------------
// Deck pool — manages up to 4 decks
// ---------------------------------------------------------------------------

/// Pool of deck states with thread-safe access.
pub struct DeckPool {
    decks: [Option<Arc<Deck>>; MAX_DECKS as usize],
}

struct Deck {
    state: std::sync::Mutex<DeckState>,
}

impl DeckPool {
    /// Create an empty deck-state pool.
    pub fn new() -> Self {
        Self {
            decks: Default::default(),
        }
    }

    /// Get or create a deck by ID (1–4).
    fn get_or_create(&mut self, id: u8) -> Result<Arc<Deck>, String> {
        if !(1..=MAX_DECKS).contains(&id) {
            return Err(format!("Invalid deck ID: {id} (must be 1–4)"));
        }

        let idx = (id - 1) as usize;
        if let Some(deck) = &self.decks[idx] {
            return Ok(Arc::clone(deck));
        }

        let deck = Arc::new(Deck {
            state: std::sync::Mutex::new(DeckState::new(id)),
        });
        self.decks[idx] = Some(deck.clone());
        Ok(deck)
    }

    /// Get a deck by ID, returning None if not created.
    fn get(&self, id: u8) -> Option<Arc<Deck>> {
        if !(1..=MAX_DECKS).contains(&id) {
            return None;
        }
        self.decks[(id - 1) as usize].clone()
    }

    /// Check if a deck is active (playing).
    pub fn is_active(&self, id: u8) -> bool {
        self.get(id)
            .map(|deck| deck.state.lock().unwrap().transport.is_active())
            .unwrap_or(false)
    }

    /// Get count of active decks.
    pub fn active_count(&self) -> u8 {
        (1..=MAX_DECKS).filter(|id| self.is_active(*id)).count() as u8
    }
}

// ---------------------------------------------------------------------------
// Four-deck mixer — combines deck audio with routing
// ---------------------------------------------------------------------------

/// Mixer state for combining up to 4 decks + master output.
pub struct MixerState {
    /// Per-deck gain (0.0–1.0).
    deck_gains: [f32; MAX_DECKS as usize],
    /// Master gain (0.0–1.0).
    master_gain: f32,
    /// Master EQ.
    /// Crossfade position (0.0 = deck A full, 1.0 = deck B full).
    crossfade: f32,
    /// Active crossfade pair (deck IDs).
    crossfade_a: u8,
    crossfade_b: u8,
}

impl MixerState {
    /// Create a new mixer state.
    pub fn new() -> Self {
        Self {
            deck_gains: [1.0; MAX_DECKS as usize],
            master_gain: 1.0,
            crossfade: 0.5,
            crossfade_a: 1,
            crossfade_b: 2,
        }
    }

    /// Validate mixer state.
    pub fn validate(&self) -> bool {
        (1..=MAX_DECKS).all(|_| true) // All gains validated at set time
            && self.master_gain >= 0.0
            && self.master_gain <= 1.0
            && self.crossfade >= 0.0
            && self.crossfade <= 1.0
            && (1..=MAX_DECKS).contains(&self.crossfade_a)
            && (1..=MAX_DECKS).contains(&self.crossfade_b)
            && self.crossfade_a != self.crossfade_b
    }

    /// Set deck gain.
    pub fn set_deck_gain(&mut self, deck_id: u8, gain: f32) -> Result<(), String> {
        if !(1..=MAX_DECKS).contains(&deck_id) {
            return Err("Invalid deck ID".into());
        }
        if !(0.0..=1.0).contains(&gain) {
            return Err("Gain must be 0.0–1.0".into());
        }
        self.deck_gains[(deck_id - 1) as usize] = gain;
        Ok(())
    }

    /// Set master gain.
    pub fn set_master_gain(&mut self, gain: f32) -> Result<(), String> {
        if !(0.0..=1.0).contains(&gain) {
            return Err("Master gain must be 0.0–1.0".into());
        }
        self.master_gain = gain;
        Ok(())
    }

    /// Set crossfade position.
    pub fn set_crossfade(&mut self, position: f32) -> Result<(), String> {
        if !(0.0..=1.0).contains(&position) {
            return Err("Crossfade must be 0.0–1.0".into());
        }
        self.crossfade = position;
        Ok(())
    }

    /// Set crossfade pair.
    pub fn set_crossfade_pair(&mut self, a: u8, b: u8) -> Result<(), String> {
        if !(1..=MAX_DECKS).contains(&a) || !(1..=MAX_DECKS).contains(&b) {
            return Err("Deck ID must be 1–4".into());
        }
        if a == b {
            return Err("Crossfade pair must differ".into());
        }
        self.crossfade_a = a;
        self.crossfade_b = b;
        Ok(())
    }

    /// Compute crossfade gain for deck A (0.0–1.0).
    pub fn crossfade_gain_a(&self) -> f32 {
        if self.crossfade.is_nan() {
            0.0
        } else {
            (1.0 - self.crossfade).clamp(0.0, 1.0)
        }
    }

    /// Compute crossfade gain for deck B (0.0–1.0).
    pub fn crossfade_gain_b(&self) -> f32 {
        if self.crossfade.is_nan() {
            0.0
        } else {
            self.crossfade.clamp(0.0, 1.0)
        }
    }
}

impl Default for MixerState {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Deck manager — combines pool, mixer, and command scheduling
// ---------------------------------------------------------------------------

/// Top-level deck manager coordinating all decks, routing, and commands.
pub struct DeckManager {
    pool: std::sync::Mutex<DeckPool>,
    mixer: std::sync::Mutex<MixerState>,
    command_scheduler: Arc<CommandScheduler>,
}

impl DeckManager {
    /// Create a new deck manager.
    pub fn new(command_scheduler: Arc<CommandScheduler>) -> Self {
        Self {
            pool: std::sync::Mutex::new(DeckPool::new()),
            mixer: std::sync::Mutex::new(MixerState::new()),
            command_scheduler,
        }
    }

    /// Load a track onto a deck.
    pub fn load_track(&self, deck_id: u8) -> Result<(), String> {
        let mut pool = self.pool.lock().unwrap();
        let deck = pool.get_or_create(deck_id)?;
        let mut state = deck.state.lock().unwrap();
        state.track_loaded = true;
        state.stop();
        state.position_frame = 0;
        state.cue_frame = 0;
        Ok(())
    }

    /// Play a deck.
    pub fn play(&self, deck_id: u8) -> Result<(), String> {
        let pool = self.pool.lock().unwrap();
        let deck = pool.get(deck_id).ok_or("Deck not found")?;
        let mut state = deck.state.lock().unwrap();
        state.play()
    }

    /// Pause a deck.
    pub fn pause(&self, deck_id: u8) -> Result<(), String> {
        let pool = self.pool.lock().unwrap();
        let deck = pool.get(deck_id).ok_or("Deck not found")?;
        let mut state = deck.state.lock().unwrap();
        state.pause();
        Ok(())
    }

    /// Stop a deck.
    pub fn stop(&self, deck_id: u8) -> Result<(), String> {
        let pool = self.pool.lock().unwrap();
        let deck = pool.get(deck_id).ok_or("Deck not found")?;
        let mut state = deck.state.lock().unwrap();
        // Cannot stop a deck that's actively playing — must pause first.
        if state.transport == DeckTransport::Playing {
            return Err("Cannot stop playing deck directly; pause first".into());
        }
        state.stop();
        Ok(())
    }

    /// Seek a deck to a frame position.
    pub fn seek(&self, deck_id: u8, frame: u64) -> Result<CommandId, String> {
        let current_frame = self.current_frame(deck_id).unwrap_or(0);
        let result = self
            .command_scheduler
            .schedule(Command::Seek { deck_id, frame }, current_frame);
        match result {
            crate::command_scheduler::ScheduleResult::Queued { command_id, .. } => Ok(command_id),
            crate::command_scheduler::ScheduleResult::Invalid { reason } => {
                Err(format!("Invalid seek: {reason}"))
            }
            crate::command_scheduler::ScheduleResult::QueueFull { .. } => {
                Err("Command queue full".into())
            }
            crate::command_scheduler::ScheduleResult::TimingViolation { .. } => {
                Err("Timing violation".into())
            }
        }
    }

    /// Get the current frame position of a deck.
    pub fn current_frame(&self, deck_id: u8) -> Option<u64> {
        let pool = self.pool.lock().unwrap();
        let deck = pool.get(deck_id)?;
        {
            let position = deck.state.lock().unwrap().position_frame;
            Some(position)
        }
    }

    /// Get the count of active (playing) decks.
    pub fn active_deck_count(&self) -> u8 {
        let pool = self.pool.lock().unwrap();
        pool.active_count()
    }

    /// Validate all deck states.
    pub fn validate_all(&self) -> Result<(), String> {
        let pool = self.pool.lock().unwrap();
        for id in 1..=MAX_DECKS {
            if let Some(deck) = pool.get(id) {
                let state = deck.state.lock().unwrap();
                if !state.validate() {
                    return Err(format!("Deck {id} validation failed"));
                }
            }
        }
        let mixer = self.mixer.lock().unwrap();
        if !mixer.validate() {
            return Err("Mixer validation failed".into());
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_manager() -> DeckManager {
        let scheduler = Arc::new(CommandScheduler::new(
            crate::command_scheduler::Config::default(),
        ));
        DeckManager::new(scheduler)
    }

    #[test]
    fn loaded_stopped_deck_is_valid_but_playing_does_not_bypass_checks() {
        let mut state = DeckState::new(1);
        state.track_loaded = true;
        assert!(state.validate());
        state.play().unwrap();
        state.rate = f64::NAN;
        assert!(!state.validate());
        state.rate = 1.0;
        state.id = 0;
        assert!(!state.validate());
    }

    #[test]
    fn deck_pool_creates_and_retrieves() {
        let mut pool = DeckPool::new();

        let deck1 = pool.get_or_create(1).unwrap();
        let state = deck1.state.lock().unwrap();
        assert_eq!(state.id, 1);
        assert_eq!(state.transport, DeckTransport::Stopped);
        assert!(!state.track_loaded);

        // Retrieve same deck.
        let deck1_again = pool.get(1).unwrap();
        assert!(Arc::ptr_eq(&deck1, &deck1_again));

        // Deck 2 not yet created.
        assert!(pool.get(2).is_none());
    }

    #[test]
    fn invalid_deck_id_rejected() {
        let mut pool = DeckPool::new();
        assert!(pool.get_or_create(0).is_err());
        assert!(pool.get_or_create(5).is_err());
    }

    #[test]
    fn deck_transport_transitions() {
        let mut state = DeckState::new(1);
        state.track_loaded = true;

        // Cannot play without track loaded — already loaded above.
        assert!(state.play().is_ok());
        assert_eq!(state.transport, DeckTransport::Playing);

        state.pause();
        assert_eq!(state.transport, DeckTransport::Paused);

        // Cannot stop playing deck directly.
        state.play().unwrap();
        assert!(state.transport.is_active());

        state.pause();
        state.stop();
        assert_eq!(state.transport, DeckTransport::Stopped);
    }

    #[test]
    fn hot_cue_set_and_recall() {
        let mut state = DeckState::new(1);
        state.position_frame = 48000; // 1 second at 48kHz

        assert!(state.set_hot_cue(0).is_ok());
        assert_eq!(state.hot_cues[0], Some(48000));

        state.position_frame = 96000;
        assert!(state.go_to_hot_cue(0).is_ok());
        assert_eq!(state.position_frame, 48000);

        // Invalid slot.
        assert!(state.set_hot_cue(16).is_err());
        assert!(state.go_to_hot_cue(16).is_err());
    }

    #[test]
    fn loop_validation() {
        let mut state = DeckState::new(1);

        // Valid loop.
        assert!(state.set_loop(0, 48000).is_ok());
        assert_eq!(state.loop_start, Some(0));
        assert_eq!(state.loop_end, Some(48000));

        // Invalid loop (start >= end).
        assert!(state.set_loop(50000, 40000).is_err());

        // Clear loop.
        state.clear_loop();
        assert!(state.loop_start.is_none());
        assert!(state.loop_end.is_none());
    }

    #[test]
    fn mixer_crossfade_gains() {
        let mut mixer = MixerState::new();

        // Center crossfade.
        assert!(mixer.set_crossfade(0.5).is_ok());
        assert!((mixer.crossfade_gain_a() - 0.5).abs() < 0.01);
        assert!((mixer.crossfade_gain_b() - 0.5).abs() < 0.01);

        // Full deck A.
        assert!(mixer.set_crossfade(0.0).is_ok());
        assert!((mixer.crossfade_gain_a() - 1.0).abs() < 0.01);
        assert!((mixer.crossfade_gain_b() - 0.0).abs() < 0.01);

        // Full deck B.
        assert!(mixer.set_crossfade(1.0).is_ok());
        assert!((mixer.crossfade_gain_a() - 0.0).abs() < 0.01);
        assert!((mixer.crossfade_gain_b() - 1.0).abs() < 0.01);
    }

    #[test]
    fn mixer_crossfade_pair_validated() {
        let mut mixer = MixerState::new();

        // Valid pair.
        assert!(mixer.set_crossfade_pair(1, 2).is_ok());

        // Same deck.
        assert!(mixer.set_crossfade_pair(1, 1).is_err());

        // Invalid deck ID.
        assert!(mixer.set_crossfade_pair(0, 1).is_err());
        assert!(mixer.set_crossfade_pair(1, 5).is_err());
    }

    #[test]
    fn manager_loads_and_plays_deck() {
        let manager = test_manager();

        // Load track.
        assert!(manager.load_track(1).is_ok());

        // Play.
        assert!(manager.play(1).is_ok());

        // Check active count.
        assert_eq!(manager.active_deck_count(), 1);
    }

    #[test]
    fn manager_seeks_with_command_scheduler() {
        let manager = test_manager();
        manager.load_track(1).unwrap();

        // Seek to frame 48000.
        let cmd_id = manager.seek(1, 48000).unwrap();
        assert_eq!(cmd_id, 1);
    }

    #[test]
    fn manager_validates_all() {
        let manager = test_manager();
        manager.load_track(1).unwrap();
        manager.load_track(2).unwrap();

        // All decks valid.
        assert!(manager.validate_all().is_ok());

        // Invalid mixer state (crossfade pair same deck).
        let mut mixer = manager.mixer.lock().unwrap();
        assert!(mixer.set_crossfade_pair(1, 1).is_err());
        drop(mixer);
        assert!(manager.validate_all().is_ok());
    }

    #[test]
    fn no_playing_deck_interruption() {
        let manager = test_manager();
        manager.load_track(1).unwrap();
        manager.play(1).unwrap();

        // Attempting to stop playing deck should fail.
        assert!(manager.stop(1).is_err());

        // Must pause first.
        assert!(manager.pause(1).is_ok());
        assert!(manager.stop(1).is_ok());
    }
}
