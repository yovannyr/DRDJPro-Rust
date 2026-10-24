//! User-defined constant-tempo 4/4 grid; no automatic beat or tempo detection.
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatGrid {
    bpm: f64,
    anchor: Duration,
}

#[derive(Debug)]
pub struct GridLine {
    pub percent: f32,
    pub downbeat: bool,
}

impl BeatGrid {
    pub fn new(bpm: f64, anchor: Duration) -> Result<Self, String> {
        if !bpm.is_finite() || !(40.0..=240.0).contains(&bpm) {
            return Err("Manual BPM must be between 40 and 240".into());
        }
        Ok(Self { bpm, anchor })
    }
    pub fn bpm(self) -> f64 {
        self.bpm
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
                    downbeat: index.rem_euclid(4) == 0,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
