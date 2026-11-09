//! Sample-accurate, non-blocking command scheduler for deck and mixer control.
//!
//! The scheduler processes transport, pitch, loop, cue, and nudge commands
//! with sample-level precision while never blocking audio callbacks. Commands
//! are queued on the control plane and applied on the audio thread at the
//! correct sample position using a lock-free ring buffer.
//!
//! ## Design principles
//!
//! * **Sample accuracy** — Commands are timestamped in output frames and
//!   applied exactly at the scheduled position.
//! * **Non-blocking** — The control thread never waits; when the queue is
//!   full, newer commands are rejected with an explicit error.
//! * **Fixed ownership** — The scheduler owns the ring buffer and a single
//!   worker thread; the audio thread only reads via `&Arc<Scheduler>`.
//! * **Overflow protection** — The queue has a hard capacity; attempts to
//!   exceed it fail fast rather than causing latency spikes.
//! * **Clock contract** — All timing uses a single master clock (output
//!   sample rate); drift compensation is bounded to ±2%.

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use crossbeam::queue::ArrayQueue;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum commands queued for audio application.
const MAX_COMMANDS: usize = 256;
/// Maximum drift from the master clock (in frames) before resync.
const MAX_DRIFT_FRAMES: u64 = 4800; // 100ms at 48kHz
/// Minimum gap between commands in frames (prevents storm).
const MIN_COMMAND_GAP_FRAMES: u64 = 48; // 1ms at 48kHz

// ---------------------------------------------------------------------------
// Command types
// ---------------------------------------------------------------------------

/// Unique command identifier.
pub type CommandId = u64;

/// Sample-accurate command targeting a deck or the mixer.
#[derive(Debug, Clone)]
pub enum Command {
    /// Seek to an absolute frame position on a deck.
    Seek { deck_id: u8, frame: u64 },
    /// Adjust playback rate (1.0 = normal, 0.98–1.02 range).
    SetRate { deck_id: u8, rate: f64 },
    /// Start/stop/resume playback.
    Transport {
        deck_id: u8,
        action: TransportAction,
    },
    /// Set loop bounds in frames.
    SetLoop { deck_id: u8, start: u64, end: u64 },
    /// Clear loop bounds.
    ClearLoop { deck_id: u8 },
    /// Set a hot cue at the current position.
    SetHotCue { deck_id: u8, slot: u8, frame: u64 },
    /// Jump to a hot cue slot.
    GoToHotCue { deck_id: u8, slot: u8 },
    /// Nudge pitch temporarily (±5%, auto-centers).
    Nudge {
        deck_id: u8,
        direction: i8, // -1 = slow, +1 = fast
        amount: f64,   // fraction (0.01–0.05)
    },
    /// Apply a tap-tempo event.
    TapTempo { deck_id: u8 },
    /// Sync this deck to the reference deck.
    Sync { target_deck: u8, reference_deck: u8 },
}

/// Transport action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportAction {
    Play,
    Pause,
    Stop,
    Cue, // Return to cue point and pause
}

impl Command {
    /// Return the deck this command targets (0 = mixer, 1–4 = decks).
    pub fn deck_id(&self) -> u8 {
        match self {
            Command::Seek { deck_id, .. }
            | Command::SetRate { deck_id, .. }
            | Command::Transport { deck_id, .. }
            | Command::SetLoop { deck_id, .. }
            | Command::ClearLoop { deck_id }
            | Command::SetHotCue { deck_id, .. }
            | Command::GoToHotCue { deck_id, .. }
            | Command::Nudge { deck_id, .. } => *deck_id,
            Command::TapTempo { deck_id } => *deck_id,
            Command::Sync { target_deck, .. } => *target_deck,
        }
    }
}

// ---------------------------------------------------------------------------
// Timestamped command (enqueued with sample position)
// ---------------------------------------------------------------------------

/// A command with its scheduled application position in output frames.
pub struct TimestampedCommand {
    pub id: CommandId,
    pub command: Command,
    /// Sample position (in output frames) when this command should apply.
    pub apply_at_frame: u64,
    /// Wall-clock time when the command was queued.
    pub queued_at: Instant,
}

impl TimestampedCommand {
    /// Check if this command is ready to apply at the given position.
    pub fn is_ready(&self, current_frame: u64) -> bool {
        current_frame >= self.apply_at_frame
    }

    /// Check if this command has expired (scheduled too far in the past).
    pub fn has_expired(&self, current_frame: u64) -> bool {
        // Expired if we're more than MAX_DRIFT_FRAMES past the scheduled position.
        current_frame > self.apply_at_frame + MAX_DRIFT_FRAMES
    }
}

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

/// Configuration for the command scheduler.
pub struct Config {
    /// Output sample rate (used for frame-to-time conversion).
    pub sample_rate: u32,
    /// Queue capacity (default: 256).
    pub queue_capacity: usize,
    /// Latency buffer in milliseconds (commands apply this many frames ahead).
    pub latency_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sample_rate: 48000,
            queue_capacity: MAX_COMMANDS,
            latency_ms: 10, // 10ms safety buffer
        }
    }
}

/// Result of scheduling a command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleResult {
    /// Command queued successfully.
    Queued {
        command_id: CommandId,
        apply_at_frame: u64,
    },
    /// Command rejected — queue full.
    QueueFull { current_frame: u64 },
    /// Command rejected — invalid parameters.
    Invalid { reason: String },
    /// Command rejected — timing violation (too close to previous command).
    TimingViolation { min_gap_frames: u64 },
}

impl std::fmt::Display for ScheduleResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleResult::Queued {
                command_id,
                apply_at_frame,
            } => write!(f, "Queued command {command_id} at frame {apply_at_frame}"),
            ScheduleResult::QueueFull { current_frame } => {
                write!(f, "Queue full at frame {current_frame}")
            }
            ScheduleResult::Invalid { reason } => write!(f, "Invalid: {reason}"),
            ScheduleResult::TimingViolation { min_gap_frames } => {
                write!(f, "Timing violation: need {min_gap_frames} frame gap")
            }
        }
    }
}

/// Sample-accurate command scheduler.
///
/// Commands are queued with a target frame position and applied by the
/// audio thread at the correct time. The scheduler enforces bounds,
/// timing constraints, and overflow protection.
pub struct Scheduler {
    queue: Arc<ArrayQueue<TimestampedCommand>>,
    next_id: AtomicUsize,
    sample_rate: u32,
    latency_frames: u64,
    last_command_frame: AtomicU64,
    pending: AtomicUsize,
    shutdown: AtomicBool,
    stats: SchedulerStats,
}

struct SchedulerStats {
    queued: AtomicUsize,
    applied: AtomicUsize,
    expired: AtomicUsize,
    rejected_full: AtomicUsize,
    rejected_timing: AtomicUsize,
}

impl Scheduler {
    /// Create a new command scheduler.
    pub fn new(config: Config) -> Self {
        let latency_frames = (config.sample_rate as u64 * config.latency_ms) / 1000;

        Self {
            queue: Arc::new(ArrayQueue::new(config.queue_capacity)),
            next_id: AtomicUsize::new(1),
            sample_rate: config.sample_rate,
            latency_frames,
            last_command_frame: AtomicU64::new(u64::MAX),
            pending: AtomicUsize::new(0),
            shutdown: AtomicBool::new(false),
            stats: SchedulerStats {
                queued: AtomicUsize::new(0),
                applied: AtomicUsize::new(0),
                expired: AtomicUsize::new(0),
                rejected_full: AtomicUsize::new(0),
                rejected_timing: AtomicUsize::new(0),
            },
        }
    }

    /// Schedule a command for sample-accurate application.
    ///
    /// The command will apply at `current_frame + latency_frames` unless
    /// a later frame is specified via [`ScheduleOptions`].
    pub fn schedule(&self, command: Command, current_frame: u64) -> ScheduleResult {
        if self.shutdown.load(Ordering::Acquire) {
            return ScheduleResult::Invalid {
                reason: "Scheduler is shut down".to_string(),
            };
        }

        // Validate command parameters.
        if let Err(reason) = self.validate_command(&command) {
            return ScheduleResult::Invalid { reason };
        }

        let Some(apply_at_frame) = current_frame.checked_add(self.latency_frames) else {
            return ScheduleResult::Invalid {
                reason: "Application frame overflows the render clock".into(),
            };
        };
        // Reserve a slot until application/expiry, including while a future command
        // is temporarily popped by the reader. Producers cannot steal that slot.
        let mut pending = self.pending.load(Ordering::Acquire);
        loop {
            if pending >= self.queue.capacity() {
                self.stats.rejected_full.fetch_add(1, Ordering::Relaxed);
                return ScheduleResult::QueueFull { current_frame };
            }
            match self.pending.compare_exchange_weak(
                pending,
                pending + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(observed) => pending = observed,
            }
        }
        let mut previous = self.last_command_frame.load(Ordering::Acquire);
        loop {
            if previous != u64::MAX
                && current_frame.saturating_sub(previous) < MIN_COMMAND_GAP_FRAMES
            {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                self.stats.rejected_timing.fetch_add(1, Ordering::Relaxed);
                return ScheduleResult::TimingViolation {
                    min_gap_frames: MIN_COMMAND_GAP_FRAMES,
                };
            }
            match self.last_command_frame.compare_exchange_weak(
                previous,
                current_frame,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(current) => previous = current,
            }
        }

        // Create timestamped command.
        let id = self.next_id.fetch_add(1, Ordering::AcqRel) as CommandId;
        let cmd = TimestampedCommand {
            id,
            command,
            apply_at_frame,
            queued_at: Instant::now(),
        };

        // Try to enqueue (non-blocking).
        if let Err(_cmd) = self.queue.push(cmd) {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            let _ = self.last_command_frame.compare_exchange(
                current_frame,
                previous,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
            self.stats.rejected_full.fetch_add(1, Ordering::Release);
            return ScheduleResult::QueueFull { current_frame };
        }

        self.stats.queued.fetch_add(1, Ordering::Release);

        ScheduleResult::Queued {
            command_id: id,
            apply_at_frame,
        }
    }

    /// Get all ready commands for the current frame position.
    ///
    /// Returns commands that are ready to apply and removes them from
    /// the queue. Expired commands are discarded.
    pub fn drain_ready(&self, current_frame: u64) -> Vec<TimestampedCommand> {
        let mut ready = Vec::new();

        for _ in 0..self.queue.len() {
            let Some(cmd) = self.queue.pop() else {
                break;
            };
            if cmd.has_expired(current_frame) {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                self.stats.expired.fetch_add(1, Ordering::Release);
                continue;
            }

            if cmd.is_ready(current_frame) {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                self.stats.applied.fetch_add(1, Ordering::Release);
                ready.push(cmd);
            } else {
                // Keep scanning this bounded snapshot so a future head cannot hide
                // an earlier due command. Its reserved slot guarantees requeue space.
                let _ = self.queue.push(cmd);
            }
        }

        ready.sort_unstable_by_key(|command| (command.apply_at_frame, command.id));
        ready
    }

    /// Get the current queue depth.
    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    /// Get scheduler statistics.
    pub fn stats(&self) -> SchedulerStatsSnapshot {
        SchedulerStatsSnapshot {
            queued: self.stats.queued.load(Ordering::Acquire),
            applied: self.stats.applied.load(Ordering::Acquire),
            expired: self.stats.expired.load(Ordering::Acquire),
            rejected_full: self.stats.rejected_full.load(Ordering::Acquire),
            rejected_timing: self.stats.rejected_timing.load(Ordering::Acquire),
            current_depth: self.queue.len(),
        }
    }

    /// Shut down the scheduler gracefully.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Release);
    }

    /// Validate command parameters (bounds checks).
    fn validate_command(&self, command: &Command) -> Result<(), String> {
        match command {
            Command::Seek { deck_id, frame } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if *frame > 7 * 24 * 60 * 60 * 48000 {
                    return Err("Frame position exceeds 7 days".into());
                }
                Ok(())
            }
            Command::SetRate { deck_id, rate } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if !(*rate >= 0.98 && *rate <= 1.02) {
                    return Err("Rate must be between 0.98 and 1.02".into());
                }
                Ok(())
            }
            Command::Transport { deck_id, .. } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                Ok(())
            }
            Command::SetLoop {
                deck_id,
                start,
                end,
            } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if *start >= *end {
                    return Err("Loop start must be before end".into());
                }
                Ok(())
            }
            Command::ClearLoop { deck_id } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                Ok(())
            }
            Command::SetHotCue {
                deck_id,
                slot,
                frame: _,
            } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if *slot > 16 {
                    return Err("Hot cue slot must be 1-16".into());
                }
                Ok(())
            }
            Command::GoToHotCue { deck_id, slot } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if *slot > 16 {
                    return Err("Hot cue slot must be 1-16".into());
                }
                Ok(())
            }
            Command::Nudge {
                deck_id,
                direction,
                amount,
            } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if ![-1, 1].contains(direction) {
                    return Err("Nudge direction must be -1 or 1".into());
                }
                if !(*amount >= 0.01 && *amount <= 0.05) {
                    return Err("Nudge amount must be between 0.01 and 0.05".into());
                }
                Ok(())
            }
            Command::TapTempo { deck_id } => {
                if *deck_id > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                Ok(())
            }
            Command::Sync {
                target_deck,
                reference_deck,
            } => {
                if *target_deck > 4 || *reference_deck > 4 {
                    return Err("Deck ID out of range (1-4)".into());
                }
                if *target_deck == *reference_deck {
                    return Err("Target and reference must differ".into());
                }
                Ok(())
            }
        }
    }
}

impl Drop for Scheduler {
    fn drop(&mut self) {
        self.shutdown();
        // Drain remaining commands.
        while self.queue.pop().is_some() {}
    }
}

/// Snapshot of scheduler statistics.
#[derive(Debug, Clone, Copy)]
pub struct SchedulerStatsSnapshot {
    pub queued: usize,
    pub applied: usize,
    pub expired: usize,
    pub rejected_full: usize,
    pub rejected_timing: usize,
    pub current_depth: usize,
}

impl SchedulerStatsSnapshot {
    /// Check if the scheduler is healthy (no excessive rejections).
    pub fn is_healthy(&self) -> bool {
        let total_rejected = self.rejected_full + self.rejected_timing;
        let total_attempted = self.queued + total_rejected;
        if total_attempted == 0 {
            return true;
        }
        (total_rejected as f64 / total_attempted as f64) < 0.1
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn test_scheduler() -> Scheduler {
        Scheduler::new(Config {
            sample_rate: 48000,
            queue_capacity: 64,
            latency_ms: 10,
        })
    }

    #[test]
    fn scheduler_creates_and_shuts_down() {
        let scheduler = test_scheduler();
        assert_eq!(scheduler.queue_depth(), 0);
        scheduler.shutdown();
        // After shutdown, scheduling should fail.
        let result = scheduler.schedule(
            Command::Transport {
                deck_id: 1,
                action: TransportAction::Play,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }

    #[test]
    fn valid_commands_are_queued() {
        let scheduler = test_scheduler();
        let result = scheduler.schedule(
            Command::Seek {
                deck_id: 1,
                frame: 48000, // 1 second
            },
            0,
        );
        match result {
            ScheduleResult::Queued {
                command_id,
                apply_at_frame,
            } => {
                assert_eq!(command_id, 1);
                assert_eq!(apply_at_frame, 480); // 10 ms at 48 kHz = 480 frames
            }
            other => panic!("Expected Queued, got {other}"),
        }
        assert_eq!(scheduler.queue_depth(), 1);
    }

    #[test]
    fn invalid_deck_id_rejected() {
        let scheduler = test_scheduler();
        let result = scheduler.schedule(
            Command::Seek {
                deck_id: 5, // Out of range
                frame: 0,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }

    #[test]
    fn rate_out_of_range_rejected() {
        let scheduler = test_scheduler();
        let result = scheduler.schedule(
            Command::SetRate {
                deck_id: 1,
                rate: 1.05, // Exceeds 1.02 limit
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }

    #[test]
    fn queue_full_rejects_new_commands() {
        let scheduler = Scheduler::new(Config {
            sample_rate: 48000,
            queue_capacity: 4,
            latency_ms: 10,
        });

        // Fill the queue.
        for i in 0..4 {
            let result = scheduler.schedule(
                Command::Seek {
                    deck_id: 1,
                    frame: i * 48000,
                },
                i * MIN_COMMAND_GAP_FRAMES,
            );
            assert!(matches!(result, ScheduleResult::Queued { .. }));
        }

        // Next should be full.
        let result = scheduler.schedule(
            Command::Seek {
                deck_id: 1,
                frame: 4 * 48000,
            },
            4 * MIN_COMMAND_GAP_FRAMES,
        );
        assert!(matches!(result, ScheduleResult::QueueFull { .. }));
    }

    #[test]
    fn timing_violation_rejects_rapid_commands() {
        let scheduler = test_scheduler();

        // First command at frame 0.
        let _ = scheduler.schedule(
            Command::Transport {
                deck_id: 1,
                action: TransportAction::Play,
            },
            0,
        );

        // Immediate command should be rejected (gap < 48 frames).
        let result = scheduler.schedule(
            Command::Transport {
                deck_id: 1,
                action: TransportAction::Pause,
            },
            10, // Only 10 frames gap
        );
        assert!(matches!(result, ScheduleResult::TimingViolation { .. }));
    }

    #[test]
    fn drain_ready_returns_applicable_commands() {
        let scheduler = test_scheduler();

        // Queue commands at different frame positions.
        let _ = scheduler.schedule(
            Command::Seek {
                deck_id: 1,
                frame: 5, // Seek destination; scheduling uses the render clock.
            },
            0,
        );

        let _ = scheduler.schedule(
            Command::Seek {
                deck_id: 1,
                frame: 100, // Independent seek destination
            },
            100,
        );

        // Drain at frame 10 — first command ready, second not.
        assert!(scheduler.drain_ready(479).is_empty());
        let ready = scheduler.drain_ready(480);
        assert_eq!(ready.len(), 1);

        // Drain at frame 110 — second command now ready.
        let ready = scheduler.drain_ready(580);
        assert_eq!(ready.len(), 1);
    }

    #[test]
    fn expired_commands_are_discarded() {
        let scheduler = Scheduler::new(Config {
            sample_rate: 48000,
            queue_capacity: 64,
            latency_ms: 10, // apply at frame 10
        });

        let _ = scheduler.schedule(
            Command::Seek {
                deck_id: 1,
                frame: 0,
            },
            0,
        );

        // Drain at frame 10000 — way past MAX_DRIFT_FRAMES (4800) from apply position (10).
        let ready = scheduler.drain_ready(10000);
        assert_eq!(ready.len(), 0);

        let stats = scheduler.stats();
        assert_eq!(stats.expired, 1);
    }

    #[test]
    fn stats_are_tracked_correctly() {
        let scheduler = test_scheduler();

        // Queue one command.
        let _ = scheduler.schedule(
            Command::Transport {
                deck_id: 1,
                action: TransportAction::Play,
            },
            0,
        );

        // Drain it.
        let _ = scheduler.drain_ready(480);

        let stats = scheduler.stats();
        assert_eq!(stats.queued, 1);
        assert_eq!(stats.applied, 1);
        assert!(stats.is_healthy());
    }

    #[test]
    fn loop_bounds_validated() {
        let scheduler = test_scheduler();

        // Valid loop.
        let result = scheduler.schedule(
            Command::SetLoop {
                deck_id: 1,
                start: 0,
                end: 480000, // 10 seconds
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Queued { .. }));

        // Invalid loop (start >= end).
        let result = scheduler.schedule(
            Command::SetLoop {
                deck_id: 1,
                start: 500000,
                end: 400000,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }

    #[test]
    fn hot_cue_slot_validated() {
        let scheduler = test_scheduler();

        // Valid hot cue (slot 5).
        let result = scheduler.schedule(
            Command::SetHotCue {
                deck_id: 1,
                slot: 5,
                frame: 100000,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Queued { .. }));

        // Invalid hot cue (slot 17).
        let result = scheduler.schedule(
            Command::SetHotCue {
                deck_id: 1,
                slot: 17,
                frame: 100000,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }

    #[test]
    fn sync_target_reference_must_differ() {
        let scheduler = test_scheduler();
        let result = scheduler.schedule(
            Command::Sync {
                target_deck: 1,
                reference_deck: 1, // Same deck
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }

    #[test]
    fn nudge_amount_validated() {
        let scheduler = test_scheduler();

        // Valid nudge.
        let result = scheduler.schedule(
            Command::Nudge {
                deck_id: 1,
                direction: 1,
                amount: 0.02,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Queued { .. }));

        // Invalid nudge (amount too small).
        let result = scheduler.schedule(
            Command::Nudge {
                deck_id: 1,
                direction: 1,
                amount: 0.001,
            },
            0,
        );
        assert!(matches!(result, ScheduleResult::Invalid { .. }));
    }
}
