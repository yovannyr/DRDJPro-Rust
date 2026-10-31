use ebur128::{EbuR128, Mode};
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::fmt;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;
use symphonia::core::audio::{SampleBuffer, SignalSpec};
use symphonia::core::codecs::{Decoder, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatReader;
use symphonia::core::meta::{Limit, MetadataOptions};

/// Ziel-Auflösung der Waveform (Anzahl Peak-Werte pro Track).
const WAVEFORM_BINS: u64 = 2000;
/// Maximale Anzahl Stream-Resets, bevor abgebrochen wird (verhindert Endlosschleifen).
const MAX_RESETS: u32 = 8;
/// Ab diesem Anteil fehlerhafter Pakete gilt die Datei als korrupt.
const CORRUPT_ERROR_RATIO: f64 = 0.10;
/// Ziel-Lautheit in LUFS (integrated), auf die der Gain normalisiert.
const TARGET_LUFS: f64 = -14.0;
/// Erlaubter Gain-Bereich (linear): ca. -20 dB bis +12 dB.
const MIN_GAIN: f64 = 0.1;
const MAX_GAIN: f64 = 4.0;

// --- BPM-Erkennung ---
/// Abtastrate der Energie-Hüllkurve (Werte pro Sekunde).
const ENV_RATE_HZ: u64 = 200;
/// Suchbereich für das Tempo.
const MIN_BPM: f64 = 70.0;
const MAX_BPM: f64 = 180.0;
/// Schwache Tempo-Vorliebe (Gauß in Oktaven um PRIOR_BPM), löst Oktav-Mehrdeutigkeiten auf.
const PRIOR_BPM: f64 = 125.0;
const PRIOR_SIGMA_OCTAVES: f64 = 0.8;
/// Mindestlänge der Hüllkurve in Sekunden, darunter wird kein BPM geschätzt.
const MIN_ANALYSIS_SECONDS: f64 = 8.0;

// --- Key-Erkennung ---
/// FFT-Größe und Schrittweite der Chroma-Analyse (bei 44,1 kHz: ca. 2,7 Hz pro Bin).
const CHROMA_FFT_SIZE: usize = 16384;
const CHROMA_HOP: usize = 8192;
/// Frequenzbereich, der in die Chroma-Berechnung einfließt.
const CHROMA_MIN_HZ: f64 = 100.0;
const CHROMA_MAX_HZ: f64 = 3500.0;
/// Frames unterhalb dieses RMS-Pegels (ca. -60 dBFS) gelten als Stille und werden übersprungen.
const CHROMA_SILENCE_RMS: f32 = 1e-3;
/// Mindest-Korrelation, ab der ein Key akzeptiert wird.
const MIN_KEY_CORRELATION: f64 = 0.3;

/// Krumhansl-Kessler-Tonartprofile (Tonika an Index 0).
const KK_MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KK_MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];
/// Camelot-Nummer der Dur-Tonart je Tonika (C, C#, D, ..., B). Moll = Parallel-Dur (Tonika + 3).
const CAMELOT_MAJOR: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrackAnalysisResult {
    pub file_path: String,
    /// Geschätztes Tempo in BPM. `0.0` bedeutet "nicht bestimmbar" (zu kurz, Stille, kein Beat).
    pub bpm: f32,
    /// Tonart in Camelot-Notation (z. B. "8A", "11B"). Leer bedeutet "nicht bestimmbar".
    pub musical_key: String,
    pub gain: f32,
    pub waveform_overview: Vec<u8>,
    pub is_corrupt: bool,
}
impl TrackAnalysisResult {
    pub(crate) fn validate(&self) -> bool {
        self.bpm.is_finite()
            && (0.0..=240.0).contains(&self.bpm)
            && self.gain.is_finite()
            && (MIN_GAIN as f32..=MAX_GAIN as f32).contains(&self.gain)
            && (self.musical_key.is_empty()
                || (1..=12).any(|n| {
                    self.musical_key == format!("{n}A") || self.musical_key == format!("{n}B")
                }))
            && self.waveform_overview.len() <= 4096
            && self.file_path.chars().count() <= 32768
    }
}

#[derive(Debug)]
pub enum AnalyzerError {
    Cancelled,
    IoError(String),
    DecoderError(String),
    UnsupportedFormat(String),
}

impl fmt::Display for AnalyzerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnalyzerError::Cancelled => write!(f, "Operation cancelled"),
            AnalyzerError::IoError(msg) => write!(f, "I/O-Fehler: {msg}"),
            AnalyzerError::DecoderError(msg) => write!(f, "Decoder-Fehler: {msg}"),
            AnalyzerError::UnsupportedFormat(msg) => write!(f, "Format nicht unterstützt: {msg}"),
        }
    }
}

impl std::error::Error for AnalyzerError {}

pub struct AudioAnalyzer;

/// Sammelt eine RMS-Hüllkurve des Mono-Mixdowns mit fester Rate (`env_rate`).
struct EnvelopeBuilder {
    hop: u64,
    env_rate: f64,
    acc: f64,
    count: u64,
    values: Vec<f32>,
}

impl EnvelopeBuilder {
    fn new(sample_rate: u32) -> Self {
        let hop = (sample_rate as u64 / ENV_RATE_HZ).max(1);
        Self {
            hop,
            env_rate: sample_rate as f64 / hop as f64,
            acc: 0.0,
            count: 0,
            values: Vec::new(),
        }
    }

    fn push_frame(&mut self, mono: f32) {
        self.acc += (mono as f64) * (mono as f64);
        self.count += 1;
        if self.count >= self.hop {
            self.values
                .push((self.acc / self.count as f64).sqrt() as f32);
            self.acc = 0.0;
            self.count = 0;
        }
    }
}

/// Sammelt über den ganzen Track ein 12-dimensionales Chroma-Profil (Tonhöhenklassen C..B).
struct ChromaBuilder {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    /// Tonhöhenklasse je FFT-Bin (None = außerhalb des Frequenzbereichs).
    bin_pitch_class: Vec<Option<usize>>,
    buffer: Vec<f32>,
    chroma: [f64; 12],
}

impl ChromaBuilder {
    fn new(sample_rate: u32) -> Self {
        let fft = FftPlanner::<f32>::new().plan_fft_forward(CHROMA_FFT_SIZE);

        // Hann-Fenster
        let window = (0..CHROMA_FFT_SIZE)
            .map(|i| {
                let phase = 2.0 * std::f64::consts::PI * i as f64 / (CHROMA_FFT_SIZE - 1) as f64;
                (0.5 - 0.5 * phase.cos()) as f32
            })
            .collect();

        let bin_hz = sample_rate as f64 / CHROMA_FFT_SIZE as f64;
        let bin_pitch_class = (0..CHROMA_FFT_SIZE / 2)
            .map(|k| {
                let f = k as f64 * bin_hz;
                if !(CHROMA_MIN_HZ..=CHROMA_MAX_HZ).contains(&f) {
                    return None;
                }
                // MIDI-Note 69 = A4 = 440 Hz; Tonhöhenklasse = MIDI mod 12 (C = 0).
                let midi = 69.0 + 12.0 * (f / 440.0).log2();
                Some((midi.round() as i64).rem_euclid(12) as usize)
            })
            .collect();

        Self {
            fft,
            window,
            bin_pitch_class,
            buffer: Vec::with_capacity(CHROMA_FFT_SIZE + CHROMA_HOP),
            chroma: [0.0; 12],
        }
    }

    fn push_frame(&mut self, mono: f32) {
        self.buffer.push(mono);
        if self.buffer.len() >= CHROMA_FFT_SIZE {
            self.process_block();
            self.buffer.drain(..CHROMA_HOP);
        }
    }

    fn process_block(&mut self) {
        let block = &self.buffer[..CHROMA_FFT_SIZE];

        let rms = (block.iter().map(|s| s * s).sum::<f32>() / CHROMA_FFT_SIZE as f32).sqrt();
        if rms < CHROMA_SILENCE_RMS {
            return;
        }

        let mut data: Vec<Complex<f32>> = block
            .iter()
            .zip(&self.window)
            .map(|(&s, &w)| Complex::new(s * w, 0.0))
            .collect();
        self.fft.process(&mut data);

        // Wurzel-komprimierte Magnituden: verhindert, dass einzelne laute Bass-Peaks dominieren.
        let mut frame = [0.0_f64; 12];
        for (k, pc) in self.bin_pitch_class.iter().enumerate() {
            if let Some(pc) = pc {
                frame[*pc] += (data[k].norm() as f64).sqrt();
            }
        }

        // Pro Frame normalisieren, damit laute Passagen nicht überproportional zählen.
        let total: f64 = frame.iter().sum();
        if total > 0.0 {
            for (chroma, frame) in self.chroma.iter_mut().zip(frame) {
                *chroma += frame / total;
            }
        }
    }
}

/// Pearson-Korrelation zweier Chroma-Vektoren; None bei fehlender Varianz.
fn pearson(a: &[f64; 12], b: &[f64; 12]) -> Option<f64> {
    let mean_a = a.iter().sum::<f64>() / 12.0;
    let mean_b = b.iter().sum::<f64>() / 12.0;
    let (mut num, mut var_a, mut var_b) = (0.0, 0.0, 0.0);
    for i in 0..12 {
        let x = a[i] - mean_a;
        let y = b[i] - mean_b;
        num += x * y;
        var_a += x * x;
        var_b += y * y;
    }
    let den = (var_a * var_b).sqrt();
    if den <= 1e-12 {
        None
    } else {
        Some(num / den)
    }
}

/// Camelot-Code für Tonika (0 = C) und Modus.
fn camelot(tonic: usize, minor: bool) -> String {
    if minor {
        format!("{}A", CAMELOT_MAJOR[(tonic + 3) % 12])
    } else {
        format!("{}B", CAMELOT_MAJOR[tonic % 12])
    }
}

/// Schätzt die Tonart durch Abgleich mit den 24 rotierten Krumhansl-Kessler-Profilen.
fn estimate_key(chroma: &[f64; 12]) -> Option<String> {
    let mut best: Option<(f64, usize, bool)> = None;

    for tonic in 0..12 {
        for minor in [false, true] {
            let profile = if minor { &KK_MINOR } else { &KK_MAJOR };
            let mut rotated = [0.0_f64; 12];
            for i in 0..12 {
                rotated[i] = profile[(i + 12 - tonic) % 12];
            }
            if let Some(corr) = pearson(chroma, &rotated) {
                if best.is_none_or(|(b, _, _)| corr > b) {
                    best = Some((corr, tonic, minor));
                }
            }
        }
    }

    let (corr, tonic, minor) = best?;
    if corr < MIN_KEY_CORRELATION {
        return None;
    }
    Some(camelot(tonic, minor))
}

/// Wählt den ersten gültigen Audiotrack, baut den Decoder und bestimmt,
/// wie viele Frames in einen Waveform-Peak einfließen.
fn append_peak(peaks: &mut Vec<u8>, frames_per_peak: &mut u64, peak: u8) {
    peaks.push(peak);
    if peaks.len() == 4096 {
        for i in 0..2048 {
            peaks[i] = peaks[2 * i].max(peaks[2 * i + 1]);
        }
        peaks.truncate(2048);
        *frames_per_peak = frames_per_peak.saturating_mul(2);
    }
}
fn open_decoder(format: &dyn FormatReader) -> Result<(u32, Box<dyn Decoder>, u64), AnalyzerError> {
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| AnalyzerError::DecoderError("Kein gültiger Audiotrack gefunden".into()))?;

    let decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| AnalyzerError::DecoderError(e.to_string()))?;

    // Feste Waveform-Auflösung, wenn die Länge bekannt ist; sonst ca. 20 Peaks pro Sekunde.
    let frames_per_peak = match track.codec_params.n_frames {
        Some(n) if n > 0 => (n / WAVEFORM_BINS).max(1),
        _ => (track.codec_params.sample_rate.unwrap_or(44_100) as u64 / 20).max(1),
    };

    Ok((track.id, decoder, frames_per_peak))
}

/// Linear interpolierter Zugriff auf die Autokorrelation; außerhalb des Bereichs 0.
fn ac_at(ac: &[f64], x: f64) -> f64 {
    if x < 0.0 {
        return 0.0;
    }
    let i = x.floor() as usize;
    if i + 1 >= ac.len() {
        return 0.0;
    }
    let frac = x - i as f64;
    ac[i] * (1.0 - frac) + ac[i + 1] * frac
}

/// Schätzt das Tempo aus einer Energie-Hüllkurve (Onset-Flux + Autokorrelation).
///
/// Ablauf:
/// 1. Log-Kompression der Hüllkurve, positive Differenz = Onset-Stärke
/// 2. Mittelwert abziehen, normierte Autokorrelation für Lags bis 4x der längsten Periode
/// 3. Score pro Kandidat-BPM = AC(lag) + 0.5*AC(2*lag) + 0.25*AC(4*lag), gewichtet mit Tempo-Prior
#[cfg(test)]
fn estimate_bpm(envelope: &[f32], env_rate: f64) -> Option<f32> {
    estimate_bpm_cancellable(
        envelope,
        env_rate,
        &crate::cancellation::Cancellation::default(),
    )
    .expect("uncancelled BPM estimate")
}
fn estimate_bpm_cancellable(
    envelope: &[f32],
    env_rate: f64,
    cancel: &crate::cancellation::Cancellation,
) -> Result<Option<f32>, AnalyzerError> {
    if cancel.is_cancelled() {
        return Err(AnalyzerError::Cancelled);
    }
    let n = envelope.len();
    if env_rate <= 0.0 || (n as f64) < env_rate * MIN_ANALYSIS_SECONDS {
        return Ok(None);
    }

    // 1. Onset-Funktion
    let compressed: Vec<f64> = envelope
        .iter()
        .map(|&e| (1.0 + 100.0 * e as f64).ln())
        .collect();
    let mut onset = vec![0.0_f64; n];
    for i in 1..n {
        onset[i] = (compressed[i] - compressed[i - 1]).max(0.0);
    }
    let mean = onset.iter().sum::<f64>() / n as f64;
    for o in onset.iter_mut() {
        *o -= mean;
    }

    // 2. Autokorrelation
    let lag_max_period = (60.0 * env_rate / MIN_BPM).ceil() as usize;
    let max_lag = (lag_max_period * 4).min(n.saturating_sub(1));
    if max_lag < 2 {
        return Ok(None);
    }
    let mut ac = vec![0.0_f64; max_lag + 1];
    for (lag, slot) in ac.iter_mut().enumerate() {
        if cancel.is_cancelled() {
            return Err(AnalyzerError::Cancelled);
        }
        let mut sum = 0.0;
        for i in lag..n {
            sum += onset[i] * onset[i - lag];
        }
        *slot = sum / (n - lag) as f64;
    }
    let ac0 = ac[0];
    if ac0 <= 1e-12 {
        return Ok(None); // Stille oder konstante Lautstärke
    }
    for v in ac.iter_mut() {
        *v /= ac0;
    }

    // 3. Kandidaten bewerten
    let mut best_bpm = 0.0_f64;
    let mut best_score = f64::MIN;
    let mut bpm = MIN_BPM;
    while bpm <= MAX_BPM {
        let lag = 60.0 * env_rate / bpm;
        let raw = ac_at(&ac, lag) + 0.5 * ac_at(&ac, 2.0 * lag) + 0.25 * ac_at(&ac, 4.0 * lag);
        let octaves = (bpm / PRIOR_BPM).log2() / PRIOR_SIGMA_OCTAVES;
        let prior = (-0.5 * octaves * octaves).exp();
        let score = raw * prior;
        if score > best_score {
            best_score = score;
            best_bpm = bpm;
        }
        bpm += 0.1;
    }

    if best_score <= 0.0 {
        return Ok(None); // kein erkennbarer periodischer Beat
    }
    Ok(Some(((best_bpm * 10.0).round() / 10.0) as f32))
}

impl AudioAnalyzer {
    #[cfg(test)]
    pub fn analyze_file<P: AsRef<Path>>(path: P) -> Result<TrackAnalysisResult, AnalyzerError> {
        Self::analyze_cancellable(path.as_ref(), &crate::cancellation::Cancellation::default())
    }
    pub(crate) fn analyze_cancellable(
        path_ref: &Path,
        cancel: &crate::cancellation::Cancellation,
    ) -> Result<TrackAnalysisResult, AnalyzerError> {
        if cancel.is_cancelled() {
            return Err(AnalyzerError::Cancelled);
        }
        let path_str = path_ref.to_string_lossy().to_string();

        let (probed, warning) = crate::media_input::probe(
            path_ref,
            &MetadataOptions {
                limit_metadata_bytes: Limit::Maximum(512 * 1024),
                limit_visual_bytes: Limit::Maximum(0),
            },
        )
        .map_err(|e| AnalyzerError::UnsupportedFormat(e.to_string()))?;

        let mut format = probed.format;
        let (mut track_id, mut decoder, mut frames_per_peak) = open_decoder(format.as_ref())?;

        let mut peaks: Vec<u8> = Vec::with_capacity(WAVEFORM_BINS as usize + 1);
        let mut block_max: f32 = 0.0;
        let mut frames_in_block: u64 = 0;

        let mut sample_count: u64 = 0;

        // Lautheit (EBU R128) und BPM-Hüllkurve werden mit der Spec des ersten Pakets angelegt.
        // A format change cannot share the existing loudness/envelope state.
        let mut first_spec: Option<SignalSpec> = None;
        let mut meter: Option<EbuR128> = None;
        let mut envelope: Option<EnvelopeBuilder> = None;
        let mut chroma: Option<ChromaBuilder> = None;

        let mut packet_count: u64 = 0;
        let mut decode_errors: u64 = 0;
        let mut resets: u32 = 0;

        let mut sample_buf: Option<SampleBuffer<f32>> = None;
        let mut current_spec: Option<SignalSpec> = None;

        loop {
            if cancel.is_cancelled() {
                return Err(AnalyzerError::Cancelled);
            }
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                // Reguläres Dateiende wird von Symphonia als UnexpectedEof gemeldet.
                Err(Error::IoError(e)) if e.kind() == ErrorKind::UnexpectedEof => break,
                // Track-Liste hat sich geändert: Decoder komplett neu aufbauen.
                Err(Error::ResetRequired) => {
                    resets += 1;
                    if resets > MAX_RESETS {
                        return Err(AnalyzerError::DecoderError(
                            "Zu viele Stream-Resets, Datei vermutlich defekt".into(),
                        ));
                    }
                    let (id, dec, _) = open_decoder(format.as_ref())?;
                    track_id = id;
                    decoder = dec;
                    sample_buf = None;
                    current_spec = None;
                    continue;
                }
                // Alle übrigen I/O-Fehler sind echte Fehler und kein Dateiende.
                Err(Error::IoError(e)) => return Err(AnalyzerError::IoError(e.to_string())),
                Err(e) => return Err(AnalyzerError::DecoderError(e.to_string())),
            };

            if packet.track_id() != track_id {
                continue;
            }
            packet_count += 1;

            match decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    let channels = spec.channels.count().max(1);
                    let needed = decoded.capacity() * channels;

                    // Puffer neu anlegen, wenn sich Spec ändert oder er zu klein ist.
                    let must_realloc = current_spec != Some(spec)
                        || sample_buf.as_ref().is_none_or(|b| b.capacity() < needed);
                    if must_realloc {
                        sample_buf =
                            Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                        current_spec = Some(spec);
                    }

                    if let Some(buf) = sample_buf.as_mut() {
                        buf.copy_interleaved_ref(decoded);

                        if first_spec.is_none() {
                            first_spec = Some(spec);
                            meter = Some(
                                EbuR128::new(channels as u32, spec.rate, Mode::I | Mode::TRUE_PEAK)
                                    .map_err(|e| AnalyzerError::DecoderError(e.to_string()))?,
                            );
                            envelope = Some(EnvelopeBuilder::new(spec.rate));
                            chroma = Some(ChromaBuilder::new(spec.rate));
                        }
                        let spec_matches = first_spec == Some(spec);
                        if !spec_matches {
                            return Err(AnalyzerError::DecoderError(
                                "Audio format changed during analysis".into(),
                            ));
                        }

                        if spec_matches {
                            if let Some(m) = meter.as_mut() {
                                m.add_frames_f32(buf.samples())
                                    .map_err(|e| AnalyzerError::DecoderError(e.to_string()))?;
                            }
                        }

                        // Frameweise verarbeiten: ein Frame = ein Sample pro Kanal.
                        for frame in buf.samples().chunks_exact(channels) {
                            let mut frame_peak: f32 = 0.0;
                            let mut frame_sum: f32 = 0.0;
                            for &s in frame {
                                if !s.is_finite() {
                                    return Err(AnalyzerError::DecoderError(
                                        "Non-finite audio sample".into(),
                                    ));
                                }
                                frame_peak = frame_peak.max(s.abs());
                                frame_sum += s;
                            }
                            sample_count += channels as u64;
                            if sample_count > spec.rate as u64 * channels as u64 * 60 * 60 {
                                return Err(AnalyzerError::DecoderError(
                                    "Analysis limited to one hour per track".into(),
                                ));
                            }

                            if spec_matches {
                                let mono = frame_sum / channels as f32;
                                if let Some(env) = envelope.as_mut() {
                                    env.push_frame(mono);
                                }
                                if let Some(ch) = chroma.as_mut() {
                                    ch.push_frame(mono);
                                }
                            }

                            block_max = block_max.max(frame_peak);
                            frames_in_block += 1;
                            if frames_in_block >= frames_per_peak {
                                append_peak(
                                    &mut peaks,
                                    &mut frames_per_peak,
                                    (block_max.clamp(0.0, 1.0) * 255.0) as u8,
                                );
                                block_max = 0.0;
                                frames_in_block = 0;
                            }
                        }
                    }
                }
                Err(Error::DecodeError(_)) => {
                    decode_errors += 1;
                    continue;
                }
                Err(e) => return Err(AnalyzerError::DecoderError(e.to_string())),
            }
        }

        // Letzten, unvollständigen Block nicht verlieren.
        if frames_in_block > 0 {
            append_peak(
                &mut peaks,
                &mut frames_per_peak,
                (block_max.clamp(0.0, 1.0) * 255.0) as u8,
            );
        }

        // Silence has no integrated loudness; actual measurement failures propagate.
        let calculated_gain = if let Some(m) = meter.as_ref() {
            let lufs = m
                .loudness_global()
                .map_err(|e| AnalyzerError::DecoderError(e.to_string()))?;
            if lufs == f64::NEG_INFINITY {
                1.0
            } else {
                if !lufs.is_finite() {
                    return Err(AnalyzerError::DecoderError(
                        "Invalid integrated loudness".into(),
                    ));
                }
                let mut gain = 10f64.powf((TARGET_LUFS - lufs) / 20.0);

                // Clipping-Schutz: Gain so begrenzen, dass der True Peak 0 dBFS nicht überschreitet.
                let mut true_peak = 0.0_f64;
                for ch in 0..m.channels() {
                    let peak = m
                        .true_peak(ch)
                        .map_err(|e| AnalyzerError::DecoderError(e.to_string()))?;
                    if !peak.is_finite() || peak < 0.0 {
                        return Err(AnalyzerError::DecoderError("Invalid true peak".into()));
                    }
                    true_peak = true_peak.max(peak);
                }
                if true_peak > 0.0 {
                    gain = gain.min(1.0 / true_peak);
                }
                gain
            }
        } else {
            1.0
        };

        // Tempo aus der gesammelten Hüllkurve schätzen; 0.0 = nicht bestimmbar.
        let bpm = match envelope.as_ref() {
            Some(e) => estimate_bpm_cancellable(&e.values, e.env_rate, cancel)?.unwrap_or(0.0),
            None => 0.0,
        };

        // Tonart aus dem gesammelten Chroma-Profil schätzen; leer = nicht bestimmbar.
        let musical_key = chroma
            .as_ref()
            .and_then(|c| estimate_key(&c.chroma))
            .unwrap_or_default();

        let error_ratio = if packet_count > 0 {
            decode_errors as f64 / packet_count as f64
        } else {
            0.0
        };
        let is_corrupt =
            sample_count == 0 || warning.is_some() || error_ratio > CORRUPT_ERROR_RATIO;

        Ok(TrackAnalysisResult {
            file_path: path_str,
            bpm,
            musical_key,
            gain: calculated_gain.clamp(MIN_GAIN, MAX_GAIN) as f32,
            waveform_overview: peaks,
            is_corrupt,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adaptive_waveform_stays_bounded_and_retains_peaks() {
        let mut peaks = Vec::new();
        let mut frames = 100;
        append_peak(&mut peaks, &mut frames, 255);
        for _ in 0..20_000 {
            append_peak(&mut peaks, &mut frames, 0);
            assert!(peaks.len() < 4096);
        }
        assert_eq!(peaks[0], 255);
        assert!(frames > 100);
    }
    #[test]
    fn existing_mp3_fixture_produces_valid_estimates_without_modifying_the_file() {
        let path = crate::test_support::fixture();
        let before = std::fs::read(&path).unwrap();
        let result = AudioAnalyzer::analyze_file(&path).unwrap();
        assert!(result.validate());
        assert!(!result.is_corrupt);
        assert!(!result.waveform_overview.is_empty());
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
    fn silence_file() -> tempfile::NamedTempFile {
        let mut file = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        let samples = 48_000u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + samples * 2).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&48_000u32.to_le_bytes());
        bytes.extend_from_slice(&96_000u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&(samples * 2).to_le_bytes());
        bytes.resize(bytes.len() + samples as usize * 2, 0);
        std::io::Write::write_all(&mut file, &bytes).unwrap();
        file
    }
    #[test]
    fn real_silence_decode_has_honest_unknown_estimates_and_bounded_waveform() {
        let file = silence_file();
        let result = AudioAnalyzer::analyze_file(file.path()).unwrap();
        assert!(result.validate());
        assert_eq!(result.bpm, 0.);
        assert!(result.musical_key.is_empty());
        assert_eq!(result.gain, 1.);
        assert!(!result.is_corrupt);
        assert!(!result.waveform_overview.is_empty());
        assert!(result.waveform_overview.iter().all(|p| *p == 0));
    }
    #[test]
    fn cancelled_analysis_and_invalid_inputs_are_explicit_errors() {
        let file = silence_file();
        let cancel = crate::cancellation::Cancellation::default();
        cancel.request();
        assert!(matches!(
            AudioAnalyzer::analyze_cancellable(file.path(), &cancel),
            Err(AnalyzerError::Cancelled)
        ));
        assert!(AudioAnalyzer::analyze_file(file.path().with_extension("missing")).is_err());
        let invalid = tempfile::Builder::new().suffix(".wav").tempfile().unwrap();
        std::fs::write(invalid.path(), b"not audio").unwrap();
        assert!(AudioAnalyzer::analyze_file(invalid.path()).is_err());
        assert!(matches!(
            estimate_bpm_cancellable(&vec![0.; 2000], 200., &cancel),
            Err(AnalyzerError::Cancelled)
        ));
    }
    #[test]
    fn persisted_analysis_rejects_invalid_values_and_unknown_fields() {
        let mut value = AudioAnalyzer::analyze_file(silence_file().path()).unwrap();
        let json = serde_json::to_value(&value).unwrap();
        let mut invalid = json.clone();
        invalid["unknown"] = true.into();
        assert!(serde_json::from_value::<TrackAnalysisResult>(invalid).is_err());
        value.bpm = f32::NAN;
        assert!(!value.validate());
        value.bpm = 120.;
        value.gain = 0.;
        assert!(!value.validate());
        value.gain = 1.;
        value.musical_key = "13A".into();
        assert!(!value.validate());
        value.musical_key.clear();
        value.waveform_overview = vec![0; 4097];
        assert!(!value.validate());
    }

    /// Synthetische Hüllkurve: Basispegel mit kurzen Pulsen im Abstand der BPM-Periode.
    fn click_envelope(bpm: f64, seconds: f64, env_rate: f64) -> Vec<f32> {
        let n = (seconds * env_rate) as usize;
        let period = 60.0 * env_rate / bpm;
        let mut env = vec![0.02_f32; n];
        let mut k = 0.0;
        loop {
            let idx = (k * period).round() as usize;
            if idx + 3 >= n {
                break;
            }
            env[idx] = 0.8;
            env[idx + 1] = 0.4;
            env[idx + 2] = 0.2;
            k += 1.0;
        }
        env
    }

    #[test]
    fn detects_120_bpm() {
        let env = click_envelope(120.0, 30.0, 200.0);
        let bpm = estimate_bpm(&env, 200.0).expect("BPM erwartet");
        assert!((bpm - 120.0).abs() < 1.0, "erkannt: {bpm}");
    }

    #[test]
    fn detects_128_bpm() {
        let env = click_envelope(128.0, 30.0, 200.0);
        let bpm = estimate_bpm(&env, 200.0).expect("BPM erwartet");
        assert!((bpm - 128.0).abs() < 1.0, "erkannt: {bpm}");
    }

    #[test]
    fn silence_has_no_bpm() {
        let env = vec![0.0_f32; 200 * 30];
        assert!(estimate_bpm(&env, 200.0).is_none());
    }

    #[test]
    fn too_short_has_no_bpm() {
        let env = click_envelope(120.0, 3.0, 200.0);
        assert!(estimate_bpm(&env, 200.0).is_none());
    }

    /// Chroma-Vektor, der exakt dem rotierten Tonartprofil entspricht.
    fn profile_chroma(profile: &[f64; 12], tonic: usize) -> [f64; 12] {
        let mut c = [0.0; 12];
        for i in 0..12 {
            c[i] = profile[(i + 12 - tonic) % 12];
        }
        c
    }

    #[test]
    fn camelot_mapping_is_correct() {
        assert_eq!(camelot(0, false), "8B"); // C-Dur
        assert_eq!(camelot(9, true), "8A"); // a-Moll
        assert_eq!(camelot(2, false), "10B"); // D-Dur
        assert_eq!(camelot(7, false), "9B"); // G-Dur
        assert_eq!(camelot(4, true), "9A"); // e-Moll
        assert_eq!(camelot(5, true), "4A"); // f-Moll
    }

    #[test]
    fn detects_key_from_profile() {
        assert_eq!(
            estimate_key(&profile_chroma(&KK_MAJOR, 0)).as_deref(),
            Some("8B")
        );
        assert_eq!(
            estimate_key(&profile_chroma(&KK_MAJOR, 2)).as_deref(),
            Some("10B")
        );
        assert_eq!(
            estimate_key(&profile_chroma(&KK_MINOR, 9)).as_deref(),
            Some("8A")
        );
        assert_eq!(
            estimate_key(&profile_chroma(&KK_MINOR, 5)).as_deref(),
            Some("4A")
        );
    }

    #[test]
    fn flat_chroma_has_no_key() {
        assert!(estimate_key(&[1.0; 12]).is_none());
    }

    #[test]
    fn chroma_builder_finds_c_major_triad() {
        let sample_rate = 44_100_u32;
        let mut builder = ChromaBuilder::new(sample_rate);
        let freqs = [261.63_f64, 329.63, 392.00]; // C4, E4, G4
        for n in 0..(sample_rate as usize * 6) {
            let t = n as f64 / sample_rate as f64;
            let s: f64 = freqs
                .iter()
                .map(|f| (2.0 * std::f64::consts::PI * f * t).sin())
                .sum::<f64>()
                / 3.0;
            builder.push_frame(s as f32 * 0.5);
        }

        let mut idx: Vec<usize> = (0..12).collect();
        idx.sort_by(|&a, &b| builder.chroma[b].partial_cmp(&builder.chroma[a]).unwrap());
        let mut top3 = idx[..3].to_vec();
        top3.sort();
        assert_eq!(top3, vec![0, 4, 7], "Chroma: {:?}", builder.chroma);
    }
}
