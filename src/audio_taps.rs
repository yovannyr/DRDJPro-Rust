//! Audio tap modules for Deck/Master/Cue signal extraction.
//!
//! Taps provide non-invasive access to audio frames for analysis and
//! encoding without blocking the audio callback. They measure signal
//! queue overflow and transfer counts. Source-time dropouts require timestamps
//! not present in this API; those counters remain unavailable (zero).
//!
//! ## Design principles
//!
//! * **Zero-allocation in audio path** — Taps pre-allocate all buffers.
//! * **Lock-free reads** — Audio callbacks only write; consumers read
//!   via snapshot or atomics.
//! * **Overflow protection** — Tap queues have fixed capacity; excess
//!   frames are dropped with an overflow counter.
//! * **Dropout detection** — Unavailable without source-frame timestamps.
//! * **Limited transfer** — Only a bounded number of frames per callback
//!   are forwarded to analysis/encoding to prevent audio thread blocking.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum frames buffered in a tap queue.
const MAX_TAP_FRAMES: usize = 8192;
/// Maximum frames transferred per callback to analysis/encoding.
const MAX_TRANSFER_PER_CALLBACK: usize = 256;
// ---------------------------------------------------------------------------
// Tap statistics
// ---------------------------------------------------------------------------

/// Live statistics from a tap.
#[derive(Debug, Clone, Copy)]
pub struct TapStats {
    /// Total frames produced.
    pub frames_produced: u64,
    /// Total frames consumed by observers.
    pub frames_consumed: u64,
    /// Frames dropped due to queue overflow.
    pub overflow_drops: u64,
    /// Number of detected dropouts (frame gaps).
    pub dropout_count: u64,
    /// Total dropout duration in samples.
    pub dropout_samples: u64,
    /// Whether the tap is currently connected (has observers).
    pub connected: bool,
}

impl TapStats {
    /// Check if the tap is healthy (low overflow and dropout rates).
    pub fn is_healthy(&self) -> bool {
        let total_issues = self.overflow_drops + self.dropout_count;
        if self.frames_produced == 0 {
            return true;
        }
        let issue_rate = total_issues as f64 / self.frames_produced as f64;
        issue_rate < 0.01 // Less than 1% issue rate
    }
}

// ---------------------------------------------------------------------------
// Tap queue — lock-free ring buffer for audio frames
// ---------------------------------------------------------------------------

/// Fixed-capacity ring buffer for stereo audio frames.
pub struct TapQueue {
    buffer: crossbeam::queue::ArrayQueue<[f32; 2]>,
    capacity: usize,
    produced: AtomicU64,
    consumed: AtomicU64,
    dropped: AtomicU64,
}

impl TapQueue {
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0 && capacity <= MAX_TAP_FRAMES);
        Self {
            buffer: crossbeam::queue::ArrayQueue::new(capacity),
            capacity,
            produced: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        }
    }
    /// Publish at most capacity frames; full queues drop the remainder.
    /// Allocation occurs only at construction, never here. No mutex is taken.
    pub fn write(&self, frames: &[[f32; 2]]) -> usize {
        let mut written = 0;
        for &frame in frames.iter().take(self.capacity) {
            if self.buffer.push(frame).is_err() {
                break;
            }
            written += 1;
        }
        self.produced.fetch_add(written as u64, Ordering::Relaxed);
        self.dropped
            .fetch_add((frames.len() - written) as u64, Ordering::Relaxed);
        written
    }
    /// Drain a bounded batch on the observer thread; this method may allocate.
    pub fn read(&self) -> Vec<[f32; 2]> {
        let mut frames = Vec::with_capacity(self.buffer.len().min(MAX_TRANSFER_PER_CALLBACK));
        while frames.len() < MAX_TRANSFER_PER_CALLBACK {
            let Some(frame) = self.buffer.pop() else {
                break;
            };
            frames.push(frame);
        }
        self.consumed
            .fetch_add(frames.len() as u64, Ordering::Relaxed);
        frames
    }
    /// Counter snapshots are independent; connected retains the queued-data indicator.
    pub fn stats(&self) -> TapStats {
        TapStats {
            frames_produced: self.produced.load(Ordering::Relaxed),
            frames_consumed: self.consumed.load(Ordering::Relaxed),
            overflow_drops: self.dropped.load(Ordering::Relaxed),
            dropout_count: 0,
            dropout_samples: 0,
            connected: !self.buffer.is_empty(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tap — high-level audio tap for Deck/Master/Cue
// ---------------------------------------------------------------------------

/// Type of audio tap.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapType {
    /// Tap on a specific deck's output.
    Deck(u8),
    /// Tap on the master mix output.
    Master,
    /// Tap on the cue (headphone) mix.
    Cue,
}

impl TapType {
    /// Return a human-readable name.
    pub fn name(&self) -> String {
        match self {
            TapType::Deck(id) => format!("Deck {id}"),
            TapType::Master => "Master".into(),
            TapType::Cue => "Cue".into(),
        }
    }
}

/// A connected audio tap with position tracking and overflow protection.
pub struct AudioTap {
    tap_type: TapType,
    queue: Arc<TapQueue>,
    enabled: AtomicBool,
    started_at: Instant,
}

impl AudioTap {
    /// Create a new audio tap.
    pub fn new(tap_type: TapType) -> Self {
        Self {
            tap_type,
            queue: Arc::new(TapQueue::new(MAX_TAP_FRAMES)),
            enabled: AtomicBool::new(true),
            started_at: Instant::now(),
        }
    }

    /// Write frames from the audio thread.
    pub fn write(&self, frames: &[[f32; 2]]) -> usize {
        if !self.enabled.load(Ordering::Acquire) {
            return 0;
        }
        self.queue.write(frames)
    }

    /// Read frames for analysis (non-blocking, limited).
    pub fn read(&self) -> Vec<[f32; 2]> {
        self.queue.read()
    }

    /// Get tap statistics.
    pub fn stats(&self) -> TapStats {
        self.queue.stats()
    }

    /// Enable or disable the tap.
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// Get the tap type.
    pub fn tap_type(&self) -> TapType {
        self.tap_type
    }

    /// Get the uptime of this tap.
    pub fn uptime(&self) -> Instant {
        self.started_at
    }
}

impl std::fmt::Display for AudioTap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "AudioTap({}) [{} frames, {} overflows, {} dropouts]",
            self.tap_type.name(),
            self.stats().frames_produced,
            self.stats().overflow_drops,
            self.stats().dropout_count,
        )
    }
}

// ---------------------------------------------------------------------------
// Tap manager — coordinates all deck/master/cue taps
// ---------------------------------------------------------------------------

/// Manages audio taps for all decks, master, and cue outputs.
pub struct TapManager {
    deck_taps: [Option<Arc<AudioTap>>; 4],
    master_tap: Arc<AudioTap>,
    cue_tap: Arc<AudioTap>,
}

impl TapManager {
    /// Create a new tap manager with all taps initialized.
    pub fn new() -> Self {
        let mut deck_taps = [None, None, None, None];
        for i in 1..=4 {
            deck_taps[i - 1] = Some(Arc::new(AudioTap::new(TapType::Deck(i as u8))));
        }

        Self {
            deck_taps,
            master_tap: Arc::new(AudioTap::new(TapType::Master)),
            cue_tap: Arc::new(AudioTap::new(TapType::Cue)),
        }
    }

    /// Get the tap for a specific deck.
    pub fn deck_tap(&self, deck_id: u8) -> Option<Arc<AudioTap>> {
        if !(1..=4).contains(&deck_id) {
            return None;
        }
        self.deck_taps[(deck_id - 1) as usize].clone()
    }

    /// Get the master tap.
    pub fn master_tap(&self) -> Arc<AudioTap> {
        Arc::clone(&self.master_tap)
    }

    /// Get the cue tap.
    pub fn cue_tap(&self) -> Arc<AudioTap> {
        Arc::clone(&self.cue_tap)
    }

    /// Get all tap statistics.
    pub fn all_stats(&self) -> Vec<(TapType, TapStats)> {
        let mut stats = Vec::new();
        for i in 1..=4 {
            if let Some(tap) = self.deck_tap(i) {
                stats.push((TapType::Deck(i), tap.stats()));
            }
        }
        stats.push((TapType::Master, self.master_tap.stats()));
        stats.push((TapType::Cue, self.cue_tap.stats()));
        stats
    }

    /// Check if all taps are healthy.
    pub fn all_healthy(&self) -> bool {
        self.all_stats().iter().all(|(_, s)| s.is_healthy())
    }

    /// Enable or disable all taps.
    pub fn set_all_enabled(&self, enabled: bool) {
        for i in 1..=4 {
            if let Some(tap) = self.deck_tap(i) {
                tap.set_enabled(enabled);
            }
        }
        self.master_tap.set_enabled(enabled);
        self.cue_tap.set_enabled(enabled);
    }
}

impl Default for TapManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #[test]
    fn queue_wraparound_preserves_order_and_writer_does_not_allocate() {
        let queue = super::TapQueue::new(4);
        let first = [[1.0, -1.0], [2.0, -2.0], [3.0, -3.0], [4.0, -4.0]];
        let allocations = crate::test_alloc::counted_allocations(|| {
            assert_eq!(queue.write(&first), 4);
        });
        assert_eq!(allocations, 0);
        assert_eq!(queue.read(), first);
        let second = [[5.0, -5.0], [6.0, -6.0]];
        assert_eq!(queue.write(&second), 2);
        assert_eq!(queue.read(), second);
        assert_eq!(queue.stats().dropout_count, 0);
        assert_eq!(queue.stats().frames_consumed, 6);
    }

    use super::*;

    fn test_tap() -> AudioTap {
        AudioTap::new(TapType::Deck(1))
    }

    fn test_tap_manager() -> TapManager {
        TapManager::new()
    }

    #[test]
    fn tap_writes_and_reads_frames() {
        let tap = test_tap();

        // Create test frames (stereo).
        let frames: Vec<[f32; 2]> = vec![[1.0, -1.0]; 100];

        // Write frames.
        let written = tap.write(&frames);
        assert_eq!(written, 100);

        // Read frames.
        let read = tap.read();
        assert_eq!(read.len(), 100);
        assert_eq!(read[0], [1.0, -1.0]);
    }

    #[test]
    fn tap_overflow_drops_excess_frames() {
        let tap = AudioTap::new(TapType::Deck(1));

        // Queue is empty, write more than capacity.
        let large_frame_count = MAX_TAP_FRAMES + 1000;
        let frames: Vec<[f32; 2]> = vec![[0.5, -0.5]; large_frame_count];
        let written = tap.write(&frames);

        // Should not exceed capacity.
        assert!(written <= MAX_TAP_FRAMES);

        // Check overflow stats.
        let stats = tap.stats();
        assert!(stats.overflow_drops > 0);
    }

    #[test]
    fn tap_reads_limited_per_callback() {
        let tap = test_tap();

        // Fill the queue.
        let frames: Vec<[f32; 2]> = vec![[1.0, -1.0]; MAX_TAP_FRAMES];
        tap.write(&frames);

        // Read should be limited to MAX_TRANSFER_PER_CALLBACK.
        let read = tap.read();
        assert_eq!(read.len(), MAX_TRANSFER_PER_CALLBACK);

        // Second read should get more.
        let read2 = tap.read();
        assert_eq!(read2.len(), MAX_TRANSFER_PER_CALLBACK);
    }

    #[test]
    fn tap_disabled_produces_no_frames() {
        let tap = test_tap();
        tap.set_enabled(false);

        let frames: Vec<[f32; 2]> = vec![[1.0, -1.0]; 100];
        let written = tap.write(&frames);
        assert_eq!(written, 0);
    }

    #[test]
    fn tap_manager_has_all_taps() {
        let manager = test_tap_manager();

        // All deck taps exist.
        for i in 1..=4 {
            assert!(manager.deck_tap(i).is_some());
        }

        // Master and cue taps exist.
        assert!(manager.master_tap().tap_type() == TapType::Master);
        assert!(manager.cue_tap().tap_type() == TapType::Cue);

        // All stats available.
        let stats = manager.all_stats();
        assert_eq!(stats.len(), 6); // 4 decks + master + cue
    }

    #[test]
    fn tap_stats_track_produced_consumed() {
        let tap = test_tap();
        let frames: Vec<[f32; 2]> = vec![[1.0, -1.0]; 500];
        tap.write(&frames);

        let read = tap.read();
        assert_eq!(read.len(), 256); // Limited by MAX_TRANSFER

        let stats = tap.stats();
        assert_eq!(stats.frames_produced, 500);
        assert_eq!(stats.frames_consumed, 256);
    }

    #[test]
    fn tap_manager_all_healthy_initially() {
        let manager = test_tap_manager();
        assert!(manager.all_healthy());
    }

    #[test]
    fn tap_manager_enable_disable_all() {
        let manager = test_tap_manager();
        manager.set_all_enabled(false);

        for i in 1..=4 {
            let tap = manager.deck_tap(i).unwrap();
            assert!(!tap.stats().connected);
        }

        manager.set_all_enabled(true);
        // Enabled doesn't change connected status (that depends on activity).
    }

    #[test]
    fn tap_display_format() {
        let tap = test_tap();
        let display = format!("{}", tap);
        assert!(display.contains("Deck 1"));
        assert!(display.contains("frames"));
    }

    #[test]
    fn tap_queue_empty_read_returns_empty() {
        let tap = test_tap();
        let read = tap.read();
        assert!(read.is_empty());
    }
}
