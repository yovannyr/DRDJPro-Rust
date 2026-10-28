//! User-defined constant-tempo grid; no automatic beat or tempo detection.
use std::time::Duration;
pub const MAX_ANCHOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    bpm: f64,
    anchor: Duration,
    beats_per_bar: u8,
}

#[derive(Debug)]
pub struct GridLine {
    pub percent: f32,
    pub downbeat: bool,
    pub first: bool,
    pub bar: i32,
}

impl BeatGrid {
    pub fn new(bpm: f64, anchor: Duration) -> Result<Self, String> {
        if !bpm.is_finite() || !(40.0..=240.0).contains(&bpm) {
            return Err("Manual BPM must be between 40 and 240".into());
        }
        if anchor > MAX_ANCHOR {
            return Err("Grid anchor exceeds seven days".into());
        }
        Ok(Self {
            bpm,
            anchor,
            beats_per_bar: 4,
        })
    }
    pub fn with_beats_per_bar(mut self, beats: u8) -> Result<Self, String> {
        if !(1..=16).contains(&beats) {
            return Err("Beats per bar must be 1 through 16".into());
        }
        self.beats_per_bar = beats;
        Ok(self)
    }
    pub fn beats_per_bar(self) -> u8 {
        self.beats_per_bar
    }
    pub fn bpm(self) -> f64 {
        self.bpm
    }
    pub fn anchor(self) -> Duration {
        self.anchor
    }
    /// Nearest whole-beat source frame; ties choose the later beat. Exclude negative beats.
    pub fn nearest_beat_frame(self, position: Duration, rate: u32) -> Result<usize, String> {
        if rate == 0 {
            return Err("Invalid source sample rate".into());
        }
        // BPM is a bounded positive normal f64. Preserve its exact binary rational
        // value so floor/ceil cannot cross a mathematical beat or nanosecond tie.
        let bits = self.bpm.to_bits();
        let mantissa = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52));
        let denominator = 60_000_000_000_i128 << (1075 - ((bits >> 52) & 0x7ff));
        let anchor = (self.anchor.as_nanos() as i128)
            .checked_mul(mantissa)
            .ok_or("Aligned beat arithmetic overflow")?;
        let delta = (position.as_nanos() as i128 - self.anchor.as_nanos() as i128)
            .checked_mul(mantissa)
            .ok_or("Aligned beat arithmetic overflow")?;
        let lower = delta.div_euclid(denominator);
        let index = (lower + i128::from(delta.rem_euclid(denominator) * 2 >= denominator))
            .max(-(anchor / denominator));
        let time = anchor
            .checked_add(
                index
                    .checked_mul(denominator)
                    .ok_or("Aligned beat arithmetic overflow")?,
            )
            .ok_or("Aligned beat arithmetic overflow")?;
        let numerator = time
            .checked_mul(i128::from(rate))
            .ok_or("Aligned beat arithmetic overflow")?;
        let divisor = mantissa * 1_000_000_000;
        let frame = numerator / divisor + i128::from((numerator % divisor) * 2 >= divisor);
        usize::try_from(frame).map_err(|_| "Aligned beat exceeds source-frame range".into())
    }
    /// Return only the visible lines. The supported 32 s/240 BPM view has at most 129.
    pub fn lines(self, start: Duration, end: Duration) -> Vec<GridLine> {
        if end <= start {
            return Vec::new();
        }
        let start = start.as_secs_f64();
        let end = end.as_secs_f64();
        let anchor = self.anchor.as_secs_f64();
        let period = 60.0 / self.bpm;
        let first = ((start - anchor) / period - 1e-9).ceil() as i64;
        let last = ((end - anchor) / period + 1e-9).floor() as i64;
        (first..=last)
            .take(132)
            .filter_map(|index| {
                let time = anchor + index as f64 * period;
                if time < start - 1e-7 || time > end + 1e-7 {
                    return None;
                }
                Some(GridLine {
                    percent: (100.0 * (time - start) / (end - start)).clamp(0.0, 100.0) as f32,
                    downbeat: index.rem_euclid(i64::from(self.beats_per_bar)) == 0,
                    first: index == 0,
                    bar: (index.div_euclid(i64::from(self.beats_per_bar)) + 1)
                        .clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                })
            })
            .collect()
    }
}

/// An isolated control-plane draft. It never owns or changes audio state.
pub struct GridDraft {
    original: Option<BeatGrid>,
    value: Option<BeatGrid>,
    bpm: f64,
    duration: Duration,
}
impl GridDraft {
    pub fn new(
        original: Option<BeatGrid>,
        seed: f64,
        position: Duration,
        duration: Duration,
    ) -> Result<Self, String> {
        if position > duration {
            return Err("Playhead is outside this track".into());
        }
        let grid = if let Some(grid) = original {
            grid
        } else {
            BeatGrid::new(seed, position)?
        };
        if grid.anchor() > duration {
            return Err("Grid anchor is outside this track".into());
        }
        Ok(Self {
            original,
            value: Some(grid),
            bpm: grid.bpm(),
            duration,
        })
    }
    pub fn original(&self) -> Option<BeatGrid> {
        self.original
    }
    pub fn value(&self) -> Option<BeatGrid> {
        self.value
    }
    pub fn bpm(&self) -> f64 {
        self.bpm
    }
    pub fn set_beats_per_bar(&mut self, beats: u8) -> Result<(), String> {
        if let Some(grid) = self.value {
            self.value = Some(grid.with_beats_per_bar(beats)?);
        }
        Ok(())
    }
    pub fn set_bpm(&mut self, text: &str) -> Result<(), String> {
        let bpm = text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .map_err(|_| "Enter a decimal BPM between 40 and 240")?;
        let grid = BeatGrid::new(bpm, self.value.map_or(Duration::ZERO, BeatGrid::anchor))?
            .with_beats_per_bar(self.value.map_or(4, |grid| grid.beats_per_bar))?;
        self.bpm = bpm;
        if self.value.is_some() {
            self.value = Some(grid);
        }
        Ok(())
    }
    pub fn anchor_here(&mut self, position: Duration) -> Result<(), String> {
        if position > self.duration {
            return Err("Anchor is outside this track".into());
        }
        self.value = Some(
            BeatGrid::new(self.bpm, position)?
                .with_beats_per_bar(self.value.map_or(4, |grid| grid.beats_per_bar))?,
        );
        Ok(())
    }
    pub fn nudge(&mut self, milliseconds: i32) -> Result<(), String> {
        let grid = self
            .value
            .ok_or("Set a downbeat before shifting the grid")?;
        let delta = Duration::from_millis(u64::from(milliseconds.unsigned_abs()));
        let anchor = if milliseconds < 0 {
            grid.anchor().saturating_sub(delta)
        } else {
            grid.anchor().saturating_add(delta).min(self.duration)
        };
        self.value =
            Some(BeatGrid::new(grid.bpm(), anchor)?.with_beats_per_bar(grid.beats_per_bar)?);
        Ok(())
    }
    pub fn clear(&mut self) {
        self.value = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn variable_bar_lengths_preserve_downbeats_through_draft_edits() {
        for beats in [3, 5, 7] {
            let grid = BeatGrid::new(120.0, Duration::ZERO)
                .unwrap()
                .with_beats_per_bar(beats)
                .unwrap();
            let lines = grid.lines(Duration::ZERO, Duration::from_secs(8));
            for (i, line) in lines.iter().enumerate() {
                assert_eq!(line.downbeat, i % usize::from(beats) == 0);
                assert_eq!(line.bar, (i / usize::from(beats)) as i32 + 1);
            }
            let mut draft =
                GridDraft::new(Some(grid), 120.0, Duration::ZERO, Duration::from_secs(10)).unwrap();
            draft.set_bpm("125").unwrap();
            draft.anchor_here(Duration::from_secs(1)).unwrap();
            draft.nudge(10).unwrap();
            assert_eq!(draft.value().unwrap().beats_per_bar, beats);
        }
        assert!(BeatGrid::new(120.0, Duration::ZERO)
            .unwrap()
            .with_beats_per_bar(0)
            .is_err());
    }

    #[test]
    fn snap_handles_nonbinary_zero_beats_and_nanosecond_sides_of_midpoint() {
        for rate in [44100, 48000, 96000] {
            let zero = BeatGrid::new(150., Duration::from_millis(1200)).unwrap();
            assert_eq!(zero.nearest_beat_frame(Duration::ZERO, rate).unwrap(), 0);
            assert_eq!(
                zero.nearest_beat_frame(Duration::from_nanos(1), rate)
                    .unwrap(),
                0
            );
            let grid = BeatGrid::new(120., Duration::from_millis(313)).unwrap();
            for (time, target_ms) in [(562999999_u64, 313_u32), (563000000, 813), (563000001, 813)]
            {
                assert_eq!(
                    grid.nearest_beat_frame(Duration::from_nanos(time), rate)
                        .unwrap(),
                    ((u64::from(target_ms) * u64::from(rate) + 500) / 1000) as usize
                );
            }
        }
    }
    #[test]
    fn nearest_beat_uses_anchor_later_ties_valid_first_beat_and_one_frame_rounding() {
        let grid = BeatGrid::new(120., Duration::from_millis(250)).unwrap();
        for rate in [44100, 48000, 96000] {
            for (position, target) in [
                (Duration::ZERO, 0.25),
                (Duration::from_nanos(499999999), 0.25),
                (Duration::from_millis(500), 0.75),
                (Duration::from_nanos(500000001), 0.75),
                (Duration::from_millis(1100), 1.25),
            ] {
                assert_eq!(
                    grid.nearest_beat_frame(position, rate).unwrap(),
                    (target * f64::from(rate)).round() as usize
                );
            }
            let before_anchor = BeatGrid::new(120., Duration::from_secs(2)).unwrap();
            assert_eq!(
                before_anchor
                    .nearest_beat_frame(Duration::ZERO, rate)
                    .unwrap(),
                0
            );
            let first = BeatGrid::new(120., Duration::from_millis(400)).unwrap();
            assert_eq!(
                first.nearest_beat_frame(Duration::ZERO, rate).unwrap(),
                (0.4 * f64::from(rate)).round() as usize
            );
            let thirds = BeatGrid::new(180., Duration::ZERO).unwrap();
            assert_eq!(
                thirds
                    .nearest_beat_frame(Duration::from_millis(300), rate)
                    .unwrap(),
                rate as usize / 3
            );
        }
        assert!(grid.nearest_beat_frame(Duration::ZERO, 0).is_err());
        assert!(grid
            .nearest_beat_frame(Duration::from_secs(u64::MAX), 48000)
            .is_err());
    }
    #[test]
    fn validates_bpm_and_generates_anchored_beats_before_and_after_anchor() {
        for bpm in [0.0, -120.0, 39.9, 240.1, f64::NAN, f64::INFINITY] {
            assert!(BeatGrid::new(bpm, Duration::ZERO).is_err());
        }
        let grid = BeatGrid::new(120.0, Duration::from_secs(1)).unwrap();
        let lines = grid.lines(Duration::ZERO, Duration::from_secs(3));
        assert_eq!(lines.len(), 7);
        for (i, line) in lines.iter().enumerate() {
            assert!((line.percent - i as f32 * 100.0 / 6.0).abs() < 0.001);
        }
        assert!(lines[2].downbeat && lines[6].downbeat);
        assert!(!lines[0].downbeat && !lines[1].downbeat);
        assert!(grid
            .lines(Duration::from_secs(2), Duration::from_secs(2))
            .is_empty());
    }
    #[test]
    fn bounded_zoom_and_fractional_tempo_do_not_drift_or_escape_viewport() {
        let grid = BeatGrid::new(240.0, Duration::ZERO).unwrap();
        let lines = grid.lines(Duration::ZERO, Duration::from_secs(32));
        assert_eq!(lines.len(), 129);
        assert_eq!(lines.first().unwrap().percent, 0.0);
        assert_eq!(lines.last().unwrap().percent, 100.0);
        let grid = BeatGrid::new(123.45, Duration::from_secs(100)).unwrap();
        let start = std::time::Instant::now();
        for second in 0..1000 {
            let lines = grid.lines(Duration::from_secs(second), Duration::from_secs(second + 8));
            assert!((16..=17).contains(&lines.len()));
            assert!(lines
                .iter()
                .all(|line| (0.0..=100.0).contains(&line.percent)));
            assert!(lines
                .windows(2)
                .all(|pair| pair[0].percent < pair[1].percent));
        }
        eprintln!("1000 manual grid view calculations: {:?}", start.elapsed());
    }
    #[test]
    fn draft_decimal_edits_clear_and_clamped_nudges_leave_original_unchanged() {
        let original = BeatGrid::new(120., Duration::from_secs(1)).unwrap();
        let mut draft = GridDraft::new(
            Some(original),
            f64::NAN,
            Duration::ZERO,
            Duration::from_secs(5),
        )
        .unwrap();
        draft.set_bpm("128,5").unwrap();
        assert_eq!(draft.value().unwrap().bpm(), 128.5);
        assert_eq!(draft.original(), Some(original));
        let before = draft.value();
        for text in ["NaN", "inf", "39.99", "240.01", "invalid"] {
            assert!(draft.set_bpm(text).is_err());
            assert_eq!(draft.value(), before);
        }
        draft.nudge(10).unwrap();
        assert_eq!(draft.value().unwrap().anchor(), Duration::from_millis(1010));
        draft.nudge(i32::MIN).unwrap();
        assert_eq!(draft.value().unwrap().anchor(), Duration::ZERO);
        draft.nudge(i32::MAX).unwrap();
        assert_eq!(draft.value().unwrap().anchor(), Duration::from_secs(5));
        draft.clear();
        assert!(draft.value().is_none());
        assert!(draft.nudge(10).is_err());
        draft.anchor_here(Duration::from_secs(2)).unwrap();
        assert!(draft.anchor_here(Duration::from_secs(6)).is_err());
        assert_eq!(draft.value().unwrap().anchor(), Duration::from_secs(2));
        assert_eq!(draft.original(), Some(original));
    }
    #[test]
    fn downbeat_anchor_is_bar_one_with_numbered_bars_before_and_after_it() {
        let grid = BeatGrid::new(120., Duration::from_secs(2)).unwrap();
        let lines = grid.lines(Duration::ZERO, Duration::from_secs(4));
        let bars: Vec<_> = lines
            .iter()
            .filter(|line| line.downbeat)
            .map(|line| (line.bar, line.first))
            .collect();
        assert_eq!(bars, vec![(0, false), (1, true), (2, false)]);
        assert_eq!(lines.iter().filter(|line| line.first).count(), 1);
        assert!(BeatGrid::new(120., MAX_ANCHOR + Duration::from_nanos(1)).is_err());
    }
}
