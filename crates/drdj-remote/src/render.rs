/// Logical framebuffer sizes supported by the generic window adapter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resolution {
    #[default]
    Full,
    Half,
}
impl Resolution {
    pub fn height(self) -> usize {
        if self == Self::Full {
            64
        } else {
            32
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Widgets {
    pub track: bool,
    pub loops: bool,
    pub cues: bool,
    pub levels: bool,
    pub eq: bool,
    pub crossfader: bool,
}
impl Default for Widgets {
    fn default() -> Self {
        Self {
            track: true,
            loops: true,
            cues: true,
            levels: true,
            eq: true,
            crossfader: true,
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Config {
    pub resolution: Resolution,
    pub widgets: Widgets,
    /// Half-height displays select metadata (false) or transport widgets (true).
    pub transport_page: bool,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Cue {
    pub position: f32,
    pub rgb: [u8; 3],
    pub occupied: bool,
    pub active: bool,
}
/// Immutable control-plane observations. Position/loop/cue coordinates are 0..1.
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub config: Config,
    pub title: String,
    pub artist: String,
    pub bpm: String,
    pub key: String,
    pub time: String,
    pub loop_caption: String,
    pub duration_seconds: f32,
    pub loaded: bool,
    pub playing: bool,
    pub position: f32,
    pub loop_range: Option<(f32, f32)>,
    pub cues: [Cue; 8],
    pub levels: [f32; 2],
    /// EQ values normalized from -24..+12 dB to 0..1 by the adapter.
    pub eq: [f32; 3],
    pub crossfader: f32,
}
impl Snapshot {
    pub(crate) fn bound_text(&mut self) {
        for text in [
            &mut self.title,
            &mut self.artist,
            &mut self.bpm,
            &mut self.key,
            &mut self.time,
            &mut self.loop_caption,
        ] {
            if let Some((end, _)) = text.char_indices().nth(128) {
                text.truncate(end);
            }
        }
    }
}
/// Interleaved RGB888 pixels, always 128 columns. Allocation occurs on the worker.
#[derive(Clone, Debug)]
pub struct Frame {
    pub height: usize,
    pub pixels: Vec<u8>,
}
impl Frame {
    fn pixel(&mut self, x: usize, y: usize, color: [u8; 3]) {
        if x < 128 && y < self.height {
            let i = (y * 128 + x) * 3;
            self.pixels[i..i + 3].copy_from_slice(&color);
        }
    }
    fn line(&mut self, x: usize, y: usize, width: usize, color: [u8; 3]) {
        for px in x..(x.saturating_add(width)).min(128) {
            self.pixel(px, y, color);
        }
    }
    fn text(&mut self, y: usize, value: &str, color: [u8; 3]) {
        for (column, ch) in value.chars().take(21).enumerate() {
            let rows = glyph(ch.to_ascii_uppercase());
            for (dy, row) in rows.into_iter().enumerate() {
                for dx in 0..5 {
                    if row & (1 << (4 - dx)) != 0 {
                        self.pixel(column * 6 + dx, y + dy, color);
                    }
                }
            }
        }
    }
}
fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    }
}
/// Pure bounded rendering: no UI, MIDI, filesystem, or audio-engine access.
pub fn render(s: &Snapshot) -> Frame {
    let mut f = Frame {
        height: s.config.resolution.height(),
        pixels: vec![0; 128 * s.config.resolution.height() * 3],
    };
    let w = s.config.widgets;
    let full = s.config.resolution == Resolution::Full;
    let white = [235, 235, 235];
    let gray = [95, 95, 95];
    let green = [70, 225, 110];
    if (full || !s.config.transport_page) && w.track {
        f.text(0, if s.loaded { &s.title } else { "NO TRACK" }, white);
        f.text(7, &s.artist, gray);
        f.text(
            14,
            &format!(
                "{} {} {}",
                if s.playing { "PLAY" } else { "SEL" },
                s.bpm.chars().take(12).collect::<String>(),
                s.key.chars().take(6).collect::<String>()
            ),
            green,
        );
        f.text(21, &s.time, white);
    }
    if full || s.config.transport_page {
        let base = if full { 28 } else { 0 };
        if w.loops {
            let range = s
                .loop_range
                .filter(|(a, b)| a.is_finite() && b.is_finite() && *a >= 0. && *b <= 1. && a < b);
            let caption = match range {
                Some((a, b)) if s.duration_seconds.is_finite() && s.duration_seconds > 0. => {
                    let seconds = s.duration_seconds.min(86400.);
                    format!(
                        "L{:.0}-{:.0} R{:.1}",
                        a * seconds,
                        b * seconds,
                        ((b - unit(s.position)) * seconds).clamp(0., (b - a) * seconds)
                    )
                }
                Some(_) => s.loop_caption.chars().take(21).collect(),
                None => "LOOP OFF".into(),
            };
            f.text(base, &caption, white);
            if let Some((a, b)) = range {
                let x = (a * 127.) as usize;
                let end = (b * 127.) as usize;
                f.line(x, base + 6, (end - x + 1).max(1), green);
                f.pixel((unit(s.position) * 127.) as usize, base + 6, white);
            }
        }
        if w.cues {
            for (i, c) in s.cues.iter().enumerate() {
                let color = if c.occupied { c.rgb } else { gray };
                f.line(i * 16, base + 8, 13, color);
                if c.occupied {
                    f.pixel(i * 16 + (unit(c.position) * 12.) as usize, base + 10, white);
                    if c.active {
                        f.line(i * 16, base + 12, 13, white);
                    }
                }
            }
        }
        if w.levels {
            for (i, level) in s.levels.iter().enumerate() {
                f.line(0, base + 15 + i * 2, 63, gray);
                f.line(0, base + 15 + i * 2, (unit(*level) * 63.) as usize, green);
            }
        }
        if w.eq {
            for (i, value) in s.eq.iter().enumerate() {
                f.line(70, base + 15 + i * 2, 58, gray);
                f.line(
                    70,
                    base + 15 + i * 2,
                    (unit(*value) * 58.) as usize,
                    [70, 155, 240],
                );
            }
        }
        if w.crossfader {
            f.line(0, base + 25, 128, gray);
            let x = (unit(s.crossfader) * 127.) as usize;
            for y in base + 23..base + 28 {
                f.pixel(x, y, white);
            }
        }
    }
    f
}
// Original compact 5x6 ASCII glyphs. Unsupported Unicode is shown as '?'; the
// window adapter retains the complete original strings in accessible metadata.
fn glyph(c: char) -> [u8; 6] {
    match c {
        'A' => [14, 17, 31, 17, 17, 17],
        'B' => [30, 17, 30, 17, 17, 30],
        'C' => [15, 16, 16, 16, 16, 15],
        'D' => [30, 17, 17, 17, 17, 30],
        'E' => [31, 16, 30, 16, 16, 31],
        'F' => [31, 16, 30, 16, 16, 16],
        'G' => [15, 16, 19, 17, 17, 15],
        'H' => [17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 18, 12],
        'K' => [17, 18, 28, 18, 17, 17],
        'L' => [16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17],
        'O' => [14, 17, 17, 17, 17, 14],
        'P' => [30, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 21, 18, 13],
        'R' => [30, 17, 30, 20, 18, 17],
        'S' => [15, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 27, 17],
        'X' => [17, 10, 4, 4, 10, 17],
        'Y' => [17, 10, 4, 4, 4, 4],
        'Z' => [31, 2, 4, 8, 16, 31],
        '0' => [14, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 14],
        '2' => [14, 17, 2, 4, 8, 31],
        '3' => [30, 1, 6, 1, 17, 14],
        '4' => [2, 6, 10, 31, 2, 2],
        '5' => [31, 16, 30, 1, 17, 14],
        '6' => [14, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8],
        '8' => [14, 17, 14, 17, 17, 14],
        '9' => [14, 17, 15, 1, 2, 12],
        ' ' => [0; 6],
        '-' => [0, 0, 31, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 4],
        ':' => [0, 4, 0, 0, 4, 0],
        '/' => [1, 2, 4, 8, 16, 0],
        '+' => [0, 4, 14, 4, 0, 0],
        '%' => [17, 2, 4, 8, 17, 0],
        _ => [14, 17, 2, 4, 0, 4],
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimensions_pages_and_widget_switches() {
        let mut s = Snapshot {
            loaded: true,
            title: "HELLO".into(),
            artist: "ARTIST".into(),
            bpm: "128".into(),
            ..Default::default()
        };
        let a = render(&s);
        assert_eq!(a.pixels.len(), 128 * 64 * 3);
        assert!(a.pixels.iter().any(|x| *x != 0));
        s.config.resolution = Resolution::Half;
        assert_eq!(render(&s).pixels.len(), 128 * 32 * 3);
        s.config.transport_page = true;
        assert_ne!(render(&s).pixels, &a.pixels[..128 * 32 * 3]);
        s.config.widgets = Widgets {
            track: false,
            loops: false,
            cues: false,
            levels: false,
            eq: false,
            crossfader: false,
        };
        assert!(render(&s).pixels.iter().all(|x| *x == 0));
    }
    #[test]
    fn malformed_values_and_unicode_are_bounded() {
        let mut s = Snapshot {
            title: "é😀".repeat(1000),
            loaded: true,
            position: f32::NAN,
            levels: [f32::INFINITY, -1.],
            eq: [f32::NAN; 3],
            crossfader: f32::INFINITY,
            loop_range: Some((0.9, 0.1)),
            ..Default::default()
        };
        s.bound_text();
        assert_eq!(s.title.chars().count(), 128);
        assert_eq!(render(&s).pixels.len(), 24576);
        s.bpm = "9".repeat(1_000_000);
        s.key = "😀".repeat(1_000_000);
        let direct = render(&s);
        s.bpm.truncate(12);
        s.key = "😀".repeat(6);
        assert_eq!(direct.pixels, render(&s).pixels);
    }
    #[test]
    fn cue_colors_positions_and_active_marker() {
        let mut s = Snapshot::default();
        s.cues[7] = Cue {
            position: 1.,
            rgb: [231, 13, 77],
            occupied: true,
            active: true,
        };
        let f = render(&s);
        assert_eq!(&f.pixels[((36 * 128 + 112) * 3)..][..3], &[231, 13, 77]);
        assert_eq!(&f.pixels[((40 * 128 + 112) * 3)..][..3], &[235, 235, 235]);
    }
    #[test]
    fn loop_caption_tracks_remaining_and_handles_invalid_duration() {
        let mut s = Snapshot {
            loop_range: Some((0.25, 0.5)),
            duration_seconds: 120.,
            position: 0.375,
            loop_caption: "CUSTOM".into(),
            ..Default::default()
        };
        let first = render(&s);
        s.position = 0.45;
        assert_ne!(first.pixels, render(&s).pixels);
        s.duration_seconds = f32::NAN;
        let fallback = render(&s);
        s.duration_seconds = -1.;
        assert_eq!(fallback.pixels, render(&s).pixels);
        s.loop_range = None;
        assert_ne!(fallback.pixels, render(&s).pixels);
    }
}
