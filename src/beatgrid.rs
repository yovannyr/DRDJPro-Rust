//! User-defined beatgrid; no automatic beat or tempo detection.
use std::time::Duration;
pub const MAX_METER_CHANGES: usize = 16;
pub const MAX_TEMPO_CHANGES: usize = 64;
const MAX_BAR: u32 = 1_000_000;
const MAX_BEAT: u64 = MAX_BAR as u64 * 16;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizeStep {
    Eighth,
    Quarter,
    Half,
    Beat,
    Two,
    Four,
}
impl TryFrom<i32> for QuantizeStep {
    type Error = String;
    fn try_from(index: i32) -> Result<Self, String> {
        match index {
            0 => Ok(Self::Eighth),
            1 => Ok(Self::Quarter),
            2 => Ok(Self::Half),
            3 => Ok(Self::Beat),
            4 => Ok(Self::Two),
            5 => Ok(Self::Four),
            _ => Err("Invalid quantization step".into()),
        }
    }
}
impl QuantizeStep {
    fn ratio(self) -> (i128, i128) {
        match self {
            Self::Eighth => (1, 8),
            Self::Quarter => (1, 4),
            Self::Half => (1, 2),
            Self::Beat => (1, 1),
            Self::Two => (2, 1),
            Self::Four => (4, 1),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterChange {
    pub bar: u32,
    pub beats: u8,
    #[serde(default = "quarter_note_denominator")]
    pub denominator: u8,
}
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TempoChange {
    pub beat: u64,
    pub bpm: f64,
}
const EMPTY_METER: MeterChange = MeterChange {
    bar: 0,
    beats: 0,
    denominator: 4,
};
const EMPTY_TEMPO: TempoChange = TempoChange { beat: 0, bpm: 0. };
const fn quarter_note_denominator() -> u8 {
    4
}
fn valid_meter_denominator(denominator: u8) -> bool {
    matches!(denominator, 2 | 4 | 8 | 16)
}
fn quarter_beats_per_bar(beats: u8, denominator: u8) -> f64 {
    f64::from(beats) * 4. / f64::from(denominator)
}
pub const MAX_ANCHOR: Duration = Duration::from_secs(7 * 24 * 60 * 60);
fn parse_bpm(text: &str) -> Result<f64, String> {
    text.trim()
        .replace(',', ".")
        .parse::<f64>()
        .map_err(|_| "Enter a decimal BPM between 40 and 240".into())
}
fn validate_bpm(bpm: f64) -> Result<(), String> {
    if !bpm.is_finite() || !(40.0..=240.0).contains(&bpm) {
        return Err("Manual BPM must be between 40 and 240".into());
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    bpm: f64,
    anchor: Duration,
    beats_per_bar: u8,
    meter_denominator: u8,
    changes: [MeterChange; MAX_METER_CHANGES],
    change_count: u8,
    tempo_changes: [TempoChange; MAX_TEMPO_CHANGES],
    tempo_change_count: u8,
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
        validate_bpm(bpm)?;
        if anchor > MAX_ANCHOR {
            return Err("Grid anchor exceeds seven days".into());
        }
        Ok(Self {
            bpm,
            anchor,
            beats_per_bar: 4,
            meter_denominator: 4,
            changes: [EMPTY_METER; MAX_METER_CHANGES],
            change_count: 0,
            tempo_changes: [EMPTY_TEMPO; MAX_TEMPO_CHANGES],
            tempo_change_count: 0,
        })
    }
    pub fn with_time_signature(mut self, beats: u8, denominator: u8) -> Result<Self, String> {
        if !(1..=16).contains(&beats) || !valid_meter_denominator(denominator) {
            return Err(
                "Time signatures require 1 through 16 beats and denominator 2, 4, 8, or 16".into(),
            );
        }
        self.beats_per_bar = beats;
        self.meter_denominator = denominator;
        Ok(self)
    }
    pub fn meter_changes(&self) -> &[MeterChange] {
        &self.changes[..usize::from(self.change_count)]
    }
    pub fn with_time_signature_change(
        mut self,
        bar: u32,
        beats: u8,
        denominator: u8,
    ) -> Result<Self, String> {
        if !(2..=MAX_BAR).contains(&bar)
            || !(1..=16).contains(&beats)
            || !valid_meter_denominator(denominator)
        {
            return Err(
                "Meter changes require bar 2 through 1000000, 1 through 16 beats, and denominator 2, 4, 8, or 16".into(),
            );
        }
        let index = match self
            .meter_changes()
            .binary_search_by_key(&bar, |change| change.bar)
        {
            Ok(index) => index,
            Err(index) => {
                let count = usize::from(self.change_count);
                if count == MAX_METER_CHANGES {
                    return Err("At most sixteen meter changes per track".into());
                }
                self.changes.copy_within(index..count, index + 1);
                self.change_count += 1;
                index
            }
        };
        self.changes[index] = MeterChange {
            bar,
            beats,
            denominator,
        };
        Ok(self)
    }
    pub fn without_meter_change(mut self, bar: u32) -> Result<Self, String> {
        let index = self
            .meter_changes()
            .binary_search_by_key(&bar, |change| change.bar)
            .map_err(|_| "No meter change at this bar")?;
        let count = usize::from(self.change_count);
        self.changes.copy_within(index + 1..count, index);
        self.change_count -= 1;
        self.changes[count - 1] = EMPTY_METER;
        Ok(self)
    }
    pub fn tempo_changes(&self) -> &[TempoChange] {
        &self.tempo_changes[..usize::from(self.tempo_change_count)]
    }
    pub fn with_tempo_change(mut self, beat: u64, bpm: f64) -> Result<Self, String> {
        validate_bpm(bpm)?;
        if !(1..=MAX_BEAT).contains(&beat) {
            return Err("Tempo changes require beat 1 through 16000000".into());
        }
        let index = match self
            .tempo_changes()
            .binary_search_by_key(&beat, |change| change.beat)
        {
            Ok(index) => index,
            Err(index) => {
                let count = usize::from(self.tempo_change_count);
                if count == MAX_TEMPO_CHANGES {
                    return Err("At most sixty-four tempo changes per track".into());
                }
                self.tempo_changes.copy_within(index..count, index + 1);
                self.tempo_change_count += 1;
                index
            }
        };
        self.tempo_changes[index] = TempoChange { beat, bpm };
        Ok(self)
    }
    pub fn without_tempo_change(mut self, beat: u64) -> Result<Self, String> {
        let index = self
            .tempo_changes()
            .binary_search_by_key(&beat, |change| change.beat)
            .map_err(|_| "No tempo change at this beat")?;
        let count = usize::from(self.tempo_change_count);
        self.tempo_changes.copy_within(index + 1..count, index);
        self.tempo_change_count -= 1;
        self.tempo_changes[count - 1] = EMPTY_TEMPO;
        Ok(self)
    }
    pub fn bar_start_beat(self, bar: u32) -> Result<f64, String> {
        if !(1..=MAX_BAR).contains(&bar) {
            return Err("Invalid bar number".into());
        }
        Ok(self.bar_start_beat_signed(i64::from(bar)))
    }
    fn bar_start_beat_signed(self, bar: i64) -> f64 {
        if bar <= 1 {
            return (bar - 1) as f64
                * quarter_beats_per_bar(self.beats_per_bar, self.meter_denominator);
        }
        let (mut current, mut start, mut beats, mut denominator) =
            (1_i64, 0_f64, self.beats_per_bar, self.meter_denominator);
        for change in self
            .meter_changes()
            .iter()
            .take_while(|change| i64::from(change.bar) <= bar)
        {
            start += (i64::from(change.bar) - current) as f64
                * quarter_beats_per_bar(beats, denominator);
            current = i64::from(change.bar);
            beats = change.beats;
            denominator = change.denominator;
        }
        start + (bar - current) as f64 * quarter_beats_per_bar(beats, denominator)
    }
    fn bar_at_beat(self, index: f64) -> (i64, bool) {
        let (mut bar, mut start, mut beats, mut denominator) =
            (1_i64, 0_f64, self.beats_per_bar, self.meter_denominator);
        for change in self.meter_changes() {
            let next = start
                + (i64::from(change.bar) - bar) as f64 * quarter_beats_per_bar(beats, denominator);
            if index < next {
                break;
            }
            start = next;
            bar = i64::from(change.bar);
            beats = change.beats;
            denominator = change.denominator;
        }
        let offset = index - start;
        let bar_length = quarter_beats_per_bar(beats, denominator);
        let bar_offset = (offset / bar_length).floor();
        let within_bar = offset - bar_offset * bar_length;
        (bar + bar_offset as i64, within_bar.abs() < 1e-9)
    }
    pub fn validate_duration(self, duration: Duration) -> Result<(), String> {
        if self.anchor > duration {
            return Err("Grid anchor is outside this track".into());
        }
        for change in self.meter_changes() {
            let time = self.time_at_beat(self.bar_start_beat(change.bar)?);
            if time >= duration.as_secs_f64() {
                return Err("Meter change is outside this track".into());
            }
        }
        for change in self.tempo_changes() {
            if self.time_at_beat(change.beat as f64) >= duration.as_secs_f64() {
                return Err("Tempo change is outside this track".into());
            }
        }
        Ok(())
    }
    fn time_at_beat(self, beat: f64) -> f64 {
        if beat <= 0. || self.tempo_changes().is_empty() {
            return self.anchor.as_secs_f64() + beat * 60. / self.bpm;
        }
        let mut current_beat = 0_f64;
        let mut current_bpm = self.bpm;
        let mut seconds = self.anchor.as_secs_f64();
        for change in self.tempo_changes() {
            let next_beat = change.beat as f64;
            if beat <= next_beat {
                return seconds + (beat - current_beat) * 60. / current_bpm;
            }
            seconds += (next_beat - current_beat) * 60. / current_bpm;
            current_beat = next_beat;
            current_bpm = change.bpm;
        }
        seconds + (beat - current_beat) * 60. / current_bpm
    }
    fn beat_at_time(self, seconds: f64) -> f64 {
        let elapsed = seconds - self.anchor.as_secs_f64();
        if elapsed <= 0. || self.tempo_changes().is_empty() {
            return elapsed * self.bpm / 60.;
        }
        let mut current_beat = 0_f64;
        let mut current_bpm = self.bpm;
        let mut current_seconds = self.anchor.as_secs_f64();
        for change in self.tempo_changes() {
            let next_beat = change.beat as f64;
            let next_seconds = current_seconds + (next_beat - current_beat) * 60. / current_bpm;
            if seconds <= next_seconds {
                return current_beat + (seconds - current_seconds) * current_bpm / 60.;
            }
            current_beat = next_beat;
            current_bpm = change.bpm;
            current_seconds = next_seconds;
        }
        current_beat + (seconds - current_seconds) * current_bpm / 60.
    }
    fn time_at_beat_nanos(self, beat: f64) -> i128 {
        const NANOS_PER_MINUTE: f64 = 60_000_000_000.;
        let mut time = self.anchor.as_nanos() as i128;
        if beat <= 0. || self.tempo_changes().is_empty() {
            return time + (beat * NANOS_PER_MINUTE / self.bpm).round() as i128;
        }
        let (mut current_beat, mut current_bpm) = (0_f64, self.bpm);
        for change in self.tempo_changes() {
            let next_beat = change.beat as f64;
            let elapsed = (next_beat - current_beat) * NANOS_PER_MINUTE / current_bpm;
            if beat <= next_beat {
                return time
                    + ((beat - current_beat) * NANOS_PER_MINUTE / current_bpm).round() as i128;
            }
            time += elapsed.round() as i128;
            current_beat = next_beat;
            current_bpm = change.bpm;
        }
        time + ((beat - current_beat) * NANOS_PER_MINUTE / current_bpm).round() as i128
    }
    fn reposition(self, bpm: f64, anchor: Duration) -> Result<Self, String> {
        Self::new(bpm, anchor)?;
        Ok(Self {
            bpm,
            anchor,
            ..self
        })
    }
    pub fn beats_per_bar(self) -> u8 {
        self.beats_per_bar
    }
    pub fn meter_denominator(self) -> u8 {
        self.meter_denominator
    }
    pub fn bpm(self) -> f64 {
        self.bpm
    }
    pub fn is_constant_tempo(self) -> bool {
        self.tempo_change_count == 0
    }
    pub fn anchor(self) -> Duration {
        self.anchor
    }
    /// Nearest whole-beat source frame; ties choose the later beat. Exclude negative beats.
    pub fn nearest_beat_frame(self, position: Duration, rate: u32) -> Result<usize, String> {
        self.quantized_frame(position, rate, QuantizeStep::Beat, false)
    }
    pub fn nearest_quantized_frame(
        self,
        position: Duration,
        rate: u32,
        step: QuantizeStep,
    ) -> Result<usize, String> {
        self.quantized_frame(position, rate, step, false)
    }
    /// Strictly later representable source frame on the selected anchored lattice.
    pub fn next_quantized_frame(
        self,
        position: Duration,
        rate: u32,
        step: QuantizeStep,
    ) -> Result<usize, String> {
        self.quantized_frame(position, rate, step, true)
    }
    fn quantized_frame(
        self,
        position: Duration,
        rate: u32,
        step: QuantizeStep,
        next: bool,
    ) -> Result<usize, String> {
        if rate == 0 {
            return Err("Invalid source sample rate".into());
        }
        if !self.is_constant_tempo() {
            return self.quantized_variable_frame(position, rate, step, next);
        }
        // BPM is a bounded positive normal f64. Preserve its exact binary rational
        // value so floor/ceil cannot cross a mathematical beat or nanosecond tie.
        let bits = self.bpm.to_bits();
        let (step_numerator, step_denominator) = step.ratio();
        let mantissa = i128::from((bits & ((1_u64 << 52) - 1)) | (1_u64 << 52)) * step_denominator;
        let denominator = (60_000_000_000_i128 << (1075 - ((bits >> 52) & 0x7ff))) * step_numerator;
        let anchor = (self.anchor.as_nanos() as i128)
            .checked_mul(mantissa)
            .ok_or("Aligned beat arithmetic overflow")?;
        let delta = (position.as_nanos() as i128 - self.anchor.as_nanos() as i128)
            .checked_mul(mantissa)
            .ok_or("Aligned beat arithmetic overflow")?;
        let lower = delta.div_euclid(denominator);
        let index = (lower + i128::from(next || delta.rem_euclid(denominator) * 2 >= denominator))
            .max(-(anchor / denominator));
        let to_frame = |index: i128| -> Result<usize, String> {
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
        };
        if !next {
            return to_frame(index);
        }
        // At extremely low rates several grid points can round onto one frame.
        // The shortest step is 1/32 s; 32 advances cover a whole frame even at 1 Hz.
        for offset in 0..=32 {
            let frame = to_frame(index + offset)?;
            if crate::looping::frame_time(frame, rate) > position {
                return Ok(frame);
            }
        }
        Err("No later representable quantization frame".into())
    }
    fn quantized_variable_frame(
        self,
        position: Duration,
        rate: u32,
        step: QuantizeStep,
        next: bool,
    ) -> Result<usize, String> {
        if position > MAX_ANCHOR {
            return Err("Grid position exceeds seven days".into());
        }
        let (numerator, denominator) = step.ratio();
        let unit = numerator as f64 / denominator as f64;
        let beat = self.beat_at_time(position.as_secs_f64());
        let scaled = beat / unit;
        let lower = scaled.floor() as i64;
        let position_ns = position.as_nanos() as i128;
        let frame_at = |target_beat: f64| -> Result<usize, String> {
            let raw_target_seconds = self.time_at_beat(target_beat);
            let target_seconds = raw_target_seconds.max(0.);
            let exact_frame = target_seconds * f64::from(rate);
            let lower = exact_frame.floor();
            let half = lower + 0.5;
            let relative_seconds = raw_target_seconds - self.anchor.as_secs_f64();
            let arithmetic_scale = self.anchor.as_secs_f64().abs() + relative_seconds.abs();
            let tie_tolerance = (arithmetic_scale * f64::from(rate) * f64::EPSILON * 4.)
                .max(exact_frame.abs().max(1.) * f64::EPSILON * 2.);
            let frame = if (exact_frame - half).abs() <= tie_tolerance {
                half.ceil()
            } else {
                exact_frame.round()
            };
            if !frame.is_finite() || frame >= usize::MAX as f64 {
                return Err("Aligned beat exceeds source-frame range".into());
            }
            Ok(frame as usize)
        };
        if next {
            for offset in 0..=32_i64 {
                let index = lower - 1 + offset;
                let target_beat = index as f64 * unit;
                let target_ns = self.time_at_beat_nanos(target_beat);
                if target_ns < 0 {
                    continue;
                }
                let frame = frame_at(target_beat)?;
                if crate::looping::frame_time(frame, rate) > position {
                    return Ok(frame);
                }
            }
            return Err("No later representable quantization frame".into());
        }
        let mut nearest: Option<(i64, i128, i128)> = None;
        for index in (lower - 1)..=(lower + 2) {
            let target_ns = self.time_at_beat_nanos(index as f64 * unit);
            if target_ns < 0 {
                continue;
            }
            let distance = (target_ns - position_ns).abs();
            if nearest.is_none_or(|(best_index, _, best_distance)| {
                distance < best_distance || (distance == best_distance && index > best_index)
            }) {
                nearest = Some((index, target_ns, distance));
            }
        }
        let (index, _, _) = nearest.ok_or("No nonnegative quantization point")?;
        frame_at(index as f64 * unit)
    }
    /// Return visible beat lines and any fractional-quarter downbeats.
    pub fn lines(self, start: Duration, end: Duration) -> Vec<GridLine> {
        if end <= start {
            return Vec::new();
        }
        let start = start.as_secs_f64();
        let end = end.as_secs_f64();
        let first = (self.beat_at_time(start) - 1e-9).ceil() as i64;
        let last = (self.beat_at_time(end) + 1e-9).floor() as i64;
        let mut lines: Vec<(f64, GridLine)> = (first..=last)
            .take(132)
            .filter_map(|index| {
                let beat = index as f64;
                let time = self.time_at_beat(beat);
                if time < start - 1e-7 || time > end + 1e-7 {
                    return None;
                }
                let (bar, downbeat) = self.bar_at_beat(beat);
                Some((
                    time,
                    GridLine {
                        percent: (100.0 * (time - start) / (end - start)).clamp(0.0, 100.0) as f32,
                        downbeat,
                        first: index == 0,
                        bar: bar.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                    },
                ))
            })
            .collect();
        let first_beat = self.beat_at_time(start);
        let last_beat = self.beat_at_time(end);
        let first_bar = self.bar_at_beat(first_beat).0 - 1;
        let last_bar = self.bar_at_beat(last_beat).0 + 1;
        for bar in (first_bar..=last_bar).take(660) {
            let beat = self.bar_start_beat_signed(bar);
            if (beat - beat.round()).abs() < 1e-9 {
                continue;
            }
            let time = self.time_at_beat(beat);
            if time >= start - 1e-7 && time <= end + 1e-7 {
                lines.push((
                    time,
                    GridLine {
                        percent: (100.0 * (time - start) / (end - start)).clamp(0.0, 100.0) as f32,
                        downbeat: true,
                        first: beat.abs() < 1e-9,
                        bar: bar.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                    },
                ));
            }
        }
        lines.sort_by(|left, right| left.0.total_cmp(&right.0));
        lines.dedup_by(|left, right| (left.0 - right.0).abs() < 1e-9);
        lines.into_iter().map(|(_, line)| line).collect()
    }
}

/// An isolated control-plane draft. It never owns or changes audio state.
#[derive(Clone, Copy)]
pub struct GridDraft {
    original: Option<BeatGrid>,
    value: Option<BeatGrid>,
    bpm: f64,
    base_beats: u8,
    base_denominator: u8,
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
        grid.validate_duration(duration)?;
        Ok(Self {
            original,
            value: Some(grid),
            bpm: grid.bpm(),
            base_beats: grid.beats_per_bar,
            base_denominator: grid.meter_denominator,
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
    pub fn set_base_meter(&mut self, beats: u8, denominator: u8) -> Result<(), String> {
        if !(1..=16).contains(&beats) || !valid_meter_denominator(denominator) {
            return Err(
                "Time signatures require 1 through 16 beats and denominator 2, 4, 8, or 16".into(),
            );
        }
        if let Some(grid) = self.value {
            let grid = grid.with_time_signature(beats, denominator)?;
            grid.validate_duration(self.duration)?;
            self.value = Some(grid);
        }
        self.base_beats = beats;
        self.base_denominator = denominator;
        Ok(())
    }
    pub fn set_meter_signature_change(
        &mut self,
        bar: u32,
        beats: u8,
        denominator: u8,
    ) -> Result<(), String> {
        let grid = self
            .value
            .ok_or("Set a grid before adding meter changes")?
            .with_time_signature_change(bar, beats, denominator)?;
        grid.validate_duration(self.duration)?;
        self.value = Some(grid);
        Ok(())
    }
    pub fn remove_meter_change(&mut self, bar: u32) -> Result<(), String> {
        let grid = self
            .value
            .ok_or("Set a grid first")?
            .without_meter_change(bar)?;
        grid.validate_duration(self.duration)?;
        self.value = Some(grid);
        Ok(())
    }
    pub fn set_tempo_change(&mut self, beat: u64, text: &str) -> Result<(), String> {
        let bpm = parse_bpm(text)?;
        let grid = self
            .value
            .ok_or("Set a grid before adding tempo changes")?
            .with_tempo_change(beat, bpm)?;
        grid.validate_duration(self.duration)?;
        self.value = Some(grid);
        Ok(())
    }
    pub fn remove_tempo_change(&mut self, beat: u64) -> Result<(), String> {
        let grid = self
            .value
            .ok_or("Set a grid first")?
            .without_tempo_change(beat)?;
        grid.validate_duration(self.duration)?;
        self.value = Some(grid);
        Ok(())
    }
    pub fn set_bpm(&mut self, text: &str) -> Result<(), String> {
        let bpm = parse_bpm(text)?;
        let grid = self
            .value
            .unwrap_or(
                BeatGrid::new(bpm, Duration::ZERO)?
                    .with_time_signature(self.base_beats, self.base_denominator)?,
            )
            .reposition(bpm, self.value.map_or(Duration::ZERO, BeatGrid::anchor))?;
        grid.validate_duration(self.duration)?;
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
        let grid = self
            .value
            .unwrap_or(
                BeatGrid::new(self.bpm, position)?
                    .with_time_signature(self.base_beats, self.base_denominator)?,
            )
            .reposition(self.bpm, position)?;
        grid.validate_duration(self.duration)?;
        self.value = Some(grid);
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
        let grid = grid.reposition(grid.bpm(), anchor)?;
        grid.validate_duration(self.duration)?;
        self.value = Some(grid);
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
    fn mixed_meters_number_bars_and_survive_draft_edits() {
        let grid = BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_time_signature_change(3, 5, 4)
            .unwrap()
            .with_time_signature_change(2, 3, 4)
            .unwrap();
        assert_eq!(grid.bar_start_beat(2).unwrap(), 4.);
        assert_eq!(grid.bar_start_beat(3).unwrap(), 7.);
        assert_eq!(grid.bar_start_beat(4).unwrap(), 12.);
        let bars: Vec<_> = grid
            .lines(Duration::ZERO, Duration::from_secs(9))
            .iter()
            .filter(|l| l.downbeat)
            .map(|l| l.bar)
            .collect();
        assert_eq!(bars, [1, 2, 3, 4, 5]);
        assert_eq!(grid.bar_at_beat(-1.), (0, false));
        let mut draft =
            GridDraft::new(Some(grid), 120., Duration::ZERO, Duration::from_secs(20)).unwrap();
        draft.set_bpm("125").unwrap();
        draft.anchor_here(Duration::from_millis(100)).unwrap();
        draft.nudge(10).unwrap();
        assert_eq!(draft.value().unwrap().meter_changes(), grid.meter_changes());
        let before = draft.value();
        assert!(draft.set_meter_signature_change(100, 3, 4).is_err());
        assert_eq!(draft.value(), before);
        draft.remove_meter_change(2).unwrap();
        assert_eq!(draft.value().unwrap().bar_start_beat(3).unwrap(), 8.);
        assert_eq!(
            grid.without_meter_change(2)
                .unwrap()
                .without_meter_change(3)
                .unwrap(),
            BeatGrid::new(120., Duration::ZERO).unwrap()
        );
        assert!(grid.with_time_signature_change(1, 3, 4).is_err());
        let mut full = BeatGrid::new(120., Duration::ZERO).unwrap();
        for bar in 2..=17 {
            full = full.with_time_signature_change(bar, 3, 4).unwrap();
        }

        assert!(full.with_time_signature_change(18, 3, 4).is_err());
        assert!(full.with_time_signature_change(17, 7, 4).is_ok());
        let base = BeatGrid::new(120., Duration::ZERO).unwrap();
        let mut edge =
            GridDraft::new(Some(base), 120., Duration::ZERO, Duration::from_secs(2)).unwrap();
        assert!(edge.set_meter_signature_change(2, 3, 4).is_err());
        assert_eq!(edge.value(), Some(base));
        let short = base.with_time_signature_change(2, 3, 4).unwrap();
        let mut edge =
            GridDraft::new(Some(short), 120., Duration::ZERO, Duration::from_secs(3)).unwrap();
        assert!(edge.set_bpm("40").is_err());
        assert_eq!(edge.value(), Some(short));
        assert_eq!(edge.bpm(), 120.);
        assert!(edge.nudge(1000).is_err());
        assert_eq!(edge.value(), Some(short));
        edge.clear();
        edge.set_base_meter(3, 4).unwrap();
        edge.anchor_here(Duration::ZERO).unwrap();
        assert_eq!(edge.value().unwrap().beats_per_bar(), 3);
        assert!(edge.value().unwrap().meter_changes().is_empty());
    }

    #[test]
    fn time_signature_denominators_map_fractional_quarter_bar_lines() {
        let grid = BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_time_signature(3, 4)
            .unwrap()
            .with_time_signature_change(3, 5, 8)
            .unwrap();
        assert_eq!(grid.bar_start_beat(2).unwrap(), 3.);
        assert_eq!(grid.bar_start_beat(3).unwrap(), 6.);
        assert_eq!(grid.bar_start_beat(4).unwrap(), 8.5);
        assert_eq!(grid.bar_at_beat(8.), (3, false));
        assert_eq!(grid.bar_at_beat(8.5), (4, true));

        let lines = grid.lines(Duration::ZERO, Duration::from_secs(5));
        let fractional_downbeat = lines
            .iter()
            .find(|line| line.bar == 4 && line.downbeat)
            .unwrap();
        assert!((fractional_downbeat.percent - 85.).abs() < 1e-4);
        assert_eq!(
            lines
                .iter()
                .filter(|line| line.downbeat)
                .map(|line| line.bar)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );

        let eighths = BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_time_signature(3, 8)
            .unwrap();
        assert_eq!(eighths.bar_start_beat(2).unwrap(), 1.5);
        assert!(eighths
            .lines(Duration::ZERO, Duration::from_secs(1))
            .iter()
            .any(|line| line.bar == 2 && line.downbeat));
        assert!(eighths.with_time_signature(3, 3).is_err());
        assert!(grid.with_time_signature_change(4, 3, 3).is_err());
    }

    #[test]
    fn variable_bar_lengths_preserve_downbeats_through_draft_edits() {
        for beats in [3, 5, 7] {
            let grid = BeatGrid::new(120.0, Duration::ZERO)
                .unwrap()
                .with_time_signature(beats, 4)
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
            .with_time_signature(0, 4)
            .is_err());
    }

    #[test]
    fn variable_tempo_changes_keep_beats_continuous_across_segments() {
        let grid = BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_tempo_change(8, 90.)
            .unwrap()
            .with_tempo_change(4, 60.)
            .unwrap();
        assert_eq!(grid.tempo_changes()[0], TempoChange { beat: 4, bpm: 60. });
        assert!((grid.time_at_beat(4.) - 2.).abs() < 1e-12);
        assert!((grid.time_at_beat(8.) - 6.).abs() < 1e-12);
        assert!((grid.time_at_beat(9.) - 6.666666666666667).abs() < 1e-12);
        assert!((grid.beat_at_time(2.) - 4.).abs() < 1e-12);
        assert!((grid.beat_at_time(6.) - 8.).abs() < 1e-12);

        let lines = grid.lines(Duration::ZERO, Duration::from_secs(8));
        let times: Vec<_> = lines.iter().map(|line| line.percent * 0.08).collect();
        assert_eq!(lines.len(), 12);
        assert!((times[4] - 2.).abs() < 1e-6);
        assert!((times[8] - 6.).abs() < 1e-6);
        assert_eq!(lines[8].bar, 3);
    }

    #[test]
    fn tempo_draft_edits_are_sorted_atomic_bounded_and_removable() {
        let base = BeatGrid::new(120., Duration::ZERO).unwrap();
        let mut draft =
            GridDraft::new(Some(base), 120., Duration::ZERO, Duration::from_secs(10)).unwrap();
        draft.set_tempo_change(8, "90").unwrap();
        draft.set_tempo_change(4, "105,5").unwrap();
        draft.set_tempo_change(8, "95").unwrap();
        assert_eq!(
            draft.value().unwrap().tempo_changes(),
            &[
                TempoChange {
                    beat: 4,
                    bpm: 105.5
                },
                TempoChange { beat: 8, bpm: 95. }
            ]
        );

        let before_invalid = draft.value();
        assert!(draft.set_tempo_change(0, "100").is_err());
        assert!(draft.set_tempo_change(5, "240.1").is_err());
        assert!(draft.set_tempo_change(21, "100").is_err());
        assert_eq!(draft.value(), before_invalid);

        draft.remove_tempo_change(4).unwrap();
        assert_eq!(
            draft.value().unwrap().tempo_changes(),
            &[TempoChange { beat: 8, bpm: 95. }]
        );
        let before_missing = draft.value();
        assert!(draft.remove_tempo_change(4).is_err());
        assert_eq!(draft.value(), before_missing);
    }

    #[test]
    fn variable_tempo_quantization_uses_local_segment_and_preserves_next_semantics() {
        let grid = BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_tempo_change(4, 60.)
            .unwrap();
        for rate in [44100, 48000, 96000] {
            assert_eq!(
                grid.nearest_beat_frame(Duration::from_millis(2499), rate)
                    .unwrap(),
                (2. * f64::from(rate)).round() as usize
            );
            assert_eq!(
                grid.nearest_beat_frame(Duration::from_millis(2500), rate)
                    .unwrap(),
                (3. * f64::from(rate)).round() as usize
            );
            assert_eq!(
                grid.nearest_quantized_frame(Duration::from_millis(2249), rate, QuantizeStep::Half)
                    .unwrap(),
                (2. * f64::from(rate)).round() as usize
            );
            assert_eq!(
                grid.next_quantized_frame(Duration::from_secs(2), rate, QuantizeStep::Beat)
                    .unwrap(),
                (3. * f64::from(rate)).round() as usize
            );
        }
        assert!(!grid.is_constant_tempo());
        assert!(BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .is_constant_tempo());
        assert!(grid.with_tempo_change(0, 100.).is_err());
        assert!(grid.with_tempo_change(5, 39.9).is_err());
        assert!(grid.with_tempo_change(5, f64::NAN).is_err());
    }

    #[test]
    fn variable_tempo_quantization_compares_neighboring_times_and_handles_float_boundaries() {
        let half_frame = BeatGrid::new(180., Duration::ZERO)
            .unwrap()
            .with_tempo_change(4, 120.)
            .unwrap();
        assert_eq!(
            half_frame
                .nearest_quantized_frame(Duration::from_millis(208), 44_100, QuantizeStep::Eighth)
                .unwrap(),
            9_188
        );
        let half_frame_100_bpm = BeatGrid::new(100., Duration::ZERO)
            .unwrap()
            .with_tempo_change(4, 120.)
            .unwrap();
        assert_eq!(
            half_frame_100_bpm
                .nearest_quantized_frame(Duration::from_millis(1275), 44_100, QuantizeStep::Eighth)
                .unwrap(),
            56_228
        );
        let cancelled_half_frame = BeatGrid::new(100., Duration::from_millis(5100))
            .unwrap()
            .with_tempo_change(4, 120.)
            .unwrap();
        assert_eq!(
            cancelled_half_frame
                .nearest_quantized_frame(Duration::from_millis(75), 44_100, QuantizeStep::Eighth)
                .unwrap(),
            3_308
        );

        let boundary = BeatGrid::new(120., Duration::ZERO)
            .unwrap()
            .with_tempo_change(1, 60.)
            .unwrap();
        assert_eq!(
            boundary
                .nearest_quantized_frame(Duration::from_millis(700), 48_000, QuantizeStep::Two)
                .unwrap(),
            0
        );

        let anchored = BeatGrid::new(120., Duration::from_millis(313))
            .unwrap()
            .with_tempo_change(1, 90.)
            .unwrap();
        assert_eq!(
            anchored
                .nearest_beat_frame(Duration::from_millis(563), 48_000)
                .unwrap(),
            39_024
        );
        assert_eq!(
            anchored
                .nearest_beat_frame(Duration::from_nanos(562_999_999), 48_000)
                .unwrap(),
            15_024
        );
        assert_eq!(
            anchored
                .nearest_beat_frame(Duration::from_nanos(563_000_001), 48_000)
                .unwrap(),
            39_024
        );

        let negative_beats = BeatGrid::new(200., Duration::from_millis(5100))
            .unwrap()
            .with_tempo_change(1, 100.)
            .unwrap();
        assert_eq!(
            negative_beats
                .nearest_beat_frame(Duration::ZERO, 48_000)
                .unwrap(),
            0
        );
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
    #[test]
    fn quantization_steps_preserve_ties_anchors_and_strict_future_frames() {
        for rate in [44100, 48000, 96000] {
            let grid = BeatGrid::new(120., Duration::from_secs(2)).unwrap();
            for index in 0..6 {
                let step = QuantizeStep::try_from(index).unwrap();
                let (n, d) = step.ratio();
                let period_ns = (500_000_000 * n / d) as u64;
                let midpoint = Duration::from_nanos(2_000_000_000 + period_ns / 2);
                let later = ((2_000_000_000 + period_ns) as u128 * u128::from(rate) + 500_000_000)
                    / 1_000_000_000;
                assert_eq!(
                    grid.nearest_quantized_frame(midpoint, rate, step).unwrap() as u128,
                    later
                );
                let earlier = grid
                    .nearest_quantized_frame(midpoint - Duration::from_nanos(1), rate, step)
                    .unwrap();
                assert_eq!(earlier, rate as usize * 2);
                for position in [
                    Duration::ZERO,
                    Duration::from_nanos(1),
                    midpoint,
                    MAX_ANCHOR,
                ] {
                    let next = grid.next_quantized_frame(position, rate, step).unwrap();
                    assert!(crate::looping::frame_time(next, rate) > position);
                }
            }
            let grid = BeatGrid::new(123.456, Duration::from_nanos(123456789)).unwrap();
            for index in 0..6 {
                let step = QuantizeStep::try_from(index).unwrap();
                for position in [
                    Duration::ZERO,
                    Duration::from_millis(355),
                    Duration::from_secs(120),
                ] {
                    let frame = grid.nearest_quantized_frame(position, rate, step).unwrap();
                    let (n, d) = step.ratio();
                    let period = 60. / grid.bpm() * n as f64 / d as f64;
                    let tick = ((position.as_secs_f64() - grid.anchor().as_secs_f64()) / period)
                        .round()
                        .max((-grid.anchor().as_secs_f64() / period).ceil());
                    let expected = ((grid.anchor().as_secs_f64() + tick * period) * f64::from(rate))
                        .round() as usize;
                    assert_eq!(frame, expected);
                }
            }
        }
        let grid = BeatGrid::new(240., Duration::from_nanos(1)).unwrap();
        assert_eq!(
            grid.next_quantized_frame(Duration::ZERO, 1, QuantizeStep::Eighth)
                .unwrap(),
            1
        );
        assert!(grid
            .next_quantized_frame(Duration::ZERO, 0, QuantizeStep::Beat)
            .is_err());
        assert!(grid
            .nearest_quantized_frame(Duration::MAX, 48000, QuantizeStep::Eighth)
            .is_err());
        assert!(QuantizeStep::try_from(-1).is_err());
        assert!(QuantizeStep::try_from(6).is_err());
    }
}
