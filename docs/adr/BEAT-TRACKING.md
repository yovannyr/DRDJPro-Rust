# Slice B — offline beat and downbeat evaluation

The offline Rust tracker returns two sorted arrays of timestamps in milliseconds.
It uses the trained Beat This! small0 model. The classical library analyzer,
deck loader and user-edited grids retain their current behavior. Results are
experimental. No model weights, recordings or Python ML dependencies are bundled.

## Reproduce

Use the existing ORT CPU library from [A2](ML-INFERENCE-SERVICE.md). Download
only the following files into a local directory from the [pinned Re:Kord
export](https://huggingface.co/mazun/rekord/tree/79ba1fe33427af235e2f5932a95224a8d77249ba).

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| logmel.onnx | 4,467,499 | 140a676b72c056a447ecff773ea7e9079b1a8065d3bd05102947e0d9fa9b3c8b |
| beatthis_small0.onnx | 10,555,794 | c2c0778c9243c8b3ea62c0d00750b555581c29b91591893d690a76b48d6496dd |

Keep the accompanying `beatthis_small0.LICENSE.txt` with the files. JKU
licenses the published weights under MIT. Its training files include material
under copyright and limited CC terms. The ONNX conversion is third-party.
Commercial-use assessment and training/evaluation overlap remain open.
[Upstream weights and training notes](https://github.com/CPJKU/beat_this/blob/b95c8ab0c58c2d9fcfd40508ae8dffbc05ac4f5c/README.md)

Build with one Cargo job. Set `DRDJ_ORT_LIBRARY` to the absolute library path.
For a shell with binary-safe pipes (cmd.exe on Windows), run:

```text
cargo build -j1 -p drdj-ml --example beat_track --features ort --locked
ffmpeg -v error -i track.wav -ac 1 -ar 22050 -f f32le - | path-to-beat_track model-directory > prediction.json
python tools/beat_score.py reference.json prediction.json
```

Both JSON files contain `beats_ms` and `downbeats_ms` arrays. FFmpeg supplies
mono PCM; the tracker streams it and verifies both model graphs before loading.
A 1024-sample centred STFT with zero edge padding feeds 50 frames/s. The
transformer processes up to 1500 frames with six-frame prediction borders.
The last window shifts back for context. In overlapped regions earlier output
wins. A seven-frame peak filter operates on the export's sigmoid outputs.
Adjacent peaks share their midpoint; downbeats snap to the closest beat.
A one-hour input cap bounds the accumulated score arrays. No CUDA, audio
callback or installation-time network request is involved.

## Paired results on Windows

The [paired counts and PCM hashes](evidence/beat-small0.json) cover eight 36-second excerpts starting at file time zero. No initial beat or intro
was excluded. The scorer matches each reference and prediction at most once
within 70 ms. For the classical baseline we measured the app import analyzer's
BPM on the identical mono samples, then projected a constant 4/4 grid with
anchor zero. This phase assumption matters: the classical analyzer returns
BPM, not event timestamps. These scores do not compare against a user-aligned
grid. To repeat the baseline, decode the excerpts to mono 22,050 Hz float WAV,
set the OS path list `DRDJ_BEAT_EVAL_FILES`, then run the ignored binary test
`report_classical_bpm_for_beat_evaluation`.

| Excerpt | Classical BPM | Classical F1 beat / downbeat | Neural F1 beat / downbeat |
| --- | ---: | ---: | ---: |
| Candombe 46 | 131.8 | .026 / .000 | .980 / 1.000 |
| Candombe 48 | 131.8 | .646 / .000 | .987 / 1.000 |
| GMD funk 105 | 104.9 | 1.000 / 1.000 | 1.000 / 1.000 |
| GMD hip-hop 87 | 117.0 | .210 / .062 | 1.000 / 1.000 |
| GMD jazz fusion 96 | 130.7 | .248 / .114 | .983 / 1.000 |
| GMD soul 102, drummer 1 | 101.7 | .661 / .688 | 1.000 / 1.000 |
| GMD soul 102, drummer 5 | 101.5 | .407 / .438 | 1.000 / 1.000 |
| GMD soul 102, drummer 7 | 102.8 | .258 / .250 | 1.000 / 1.000 |

Candombe uses human beat annotations (timestamps in seconds and position in the
bar). Martín Rocamora and Luis Jure, *Uruguayan candombe drumming*, CC BY 4.0,
v1.0, `zavala.muniz.2014_46` and `_48`, audio file IDs 250 and 237, annotations
224 and 205. For the optional regression test, place recording 46 and its
annotations beside the model files as `46.flac` and `46.beats`, set
`DRDJ_BEAT_TEST_DIR` to that directory, and run
`cargo test -p drdj-ml --features ort --test beat_tracking -- --include-ignored`.
[Candombe data and license](https://doi.org/10.60895/redata/AY9CGZ)

The other six references are metronome grids derived from the published tempo,
4/4 meter and a zero bar origin, rather than hand-labelled beat times. Google
Groove MIDI Dataset v1.0.0, CC BY 4.0, test IDs: drummer1/eval_session/2,
/6 and /10, drummer5/eval_session/10, drummer7/eval_session/10, and
drummer7/session2/112. Only individual entries were fetched from the archive.
[Groove MIDI Dataset](https://magenta.withgoogle.com/datasets/groove)

The sample covers percussion and repeats one groove across three performers.
It cannot establish full-song or mixed-genre accuracy. Before enabling neural
grid proposals in the application, compare a broader held-out corpus and
approved Serato grids, measure concurrent playback cost, and preserve explicit
user edits and the classical fallback.

## Full recordings on Windows

Both analyzers received all samples through EOF, decoded by FFmpeg to mono
22,050 Hz. The classical comparator used the import analyzer on a float WAV,
then projected the measured BPM from zero to the audio duration in 4/4.
The table uses the same 70 ms one-to-one scorer as the excerpt evaluation.
Counts are matched / reference / predicted for the neural tracker.

| Recording | Audio seconds | Classical BPM | Classical F1 beat / downbeat | Neural F1 beat / downbeat | Beat counts | Downbeat counts |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Candombe 46 | 116.63 | 132.1 | .249 / .094 | .992 / 1.000 | 248/249/251 | 63/63/63 |
| Candombe 48 | 151.12 | 133.6 | .469 / .000 | .996 / 1.000 | 333/333/336 | 84/84/84 |
| GMD progressive rock, 190 BPM | 95.99 | 94.7 | .281 / .158 | .881 / .918 | 289/304/352 | 73/76/83 |
| GMD funk, 96 BPM | 160.00 | 96.1 | .421 / .419 | 1.000 / 1.000 | 256/256/256 | 64/64/64 |
| GMD jazz-funk, 116 BPM | 76.49 | 115.9 | 1.000 / 1.000 | .997 / .973 | 147/148/147 | 36/37/37 |

Candombe uses every published human annotation. For the Groove MIDI Dataset
test split we selected the longest available 4/4 beat recordings between 35
and 180 seconds in rock, funk and jazz, excluding IDs from the excerpt trial.
The IDs in table order are `drummer1/session2/110`, `drummer8/session1/8`
and `drummer1/session1/4`. To reproduce the reference, generate quarter notes
from time zero through the published `info.csv` duration inclusive at the
listed BPM, taking every fourth beat as a downbeat. Score **all** predictions
through audio EOF, including any tail beyond the last reference beat. These
are tempo-derived percussion references. In progressive rock there were 63
unmatched beat predictions and 15 missed grid beats. On jazz-funk the
classical projection scored higher. This sample supports keeping neural grid
proposals experimental.

### Concurrent two-deck playback

The ignored `b_headless_render_with_full_track_load` test reuses the A1
renderer. Each phase loops two known signals through the real Rodio mixer and
checks every sample. Each 480-frame block at 48 kHz must finish by its
scheduled start plus 10 ms. The Windows render thread uses the WASAPI callback
priority. In the second phase a separate process repeatedly decodes and tracks
complete songs, one at a time. Each analysis starts a fresh CPU tracker and
streams FFmpeg output directly into it. Only songs that reach EOF in both
processes count. The test requires at least two completions and coverage of
every listed song within the loaded phase.

To repeat, build the offline tracker as described above. Set
`DRDJ_B_TRACKER` to its absolute executable path, `DRDJ_BEAT_TEST_DIR` to
the model directory, `DRDJ_ORT_LIBRARY` to the CPU runtime and
`DRDJ_B_TRACK_LIST` to a UTF-8 JSON array file containing absolute song
paths. Python 3 and FFmpeg must be on PATH. Use a shared Cargo target to reuse
the application dependencies. The default phase length is 600 seconds.

```text
cargo --config 'profile.test.package.drdjpro-ui-spike.opt-level=1' test -j1 --bin drdjpro-ui-spike --locked b_headless_render_with_full_track_load -- --ignored --nocapture
python -B -m unittest discover -s tools -p "test_beat*.py"
```

The Windows measurement used an Intel Core i7-11850H, Rust 1.99.1, application
package opt-level 1, the debug offline tracker with one ORT thread and the
1.31.0 CPU runtime. The load manifest contained the first MP3 in filename
order with duration between 2 and 6 minutes in each of five genre folders.
Only anonymized statistics are reported. The timing includes decoder, process
startup, model load and analysis. A new process for each song makes this a
host contention test of the offline tracker. In-device playback, analysis
within the application process and comparison with Serato grids are separate
qualification steps.

| Local genre | Audio seconds | Beats / downbeats | First / last beat (s) | Longest interval (s) | Median interval as BPM |
| --- | ---: | ---: | ---: | ---: | ---: |
| Bachata | 211.82 | 457 / 115 | 0.08 / 211.74 | 0.48 | 130.4 |
| Salsa | 281.59 | 499 / 161 | 0.42 / 277.26 | 0.70 | 93.8 |
| Kizomba | 334.53 | 484 / 126 | 0.16 / 329.48 | 0.70 | 88.2 |
| Merengue | 255.52 | 565 / 150 | 0.28 / 252.74 | 0.50 | 136.4 |
| Reggaeton | 203.50 | 471 / 120 | 0.00 / 201.64 | 0.64 | 136.4 |

These five library tracks have no reference labels in this evaluation.
Sorted timestamps, downbeat membership and detections near the end verify
output coverage. The greater downbeat density on the salsa track (499 beats
and 161 downbeats) is a concrete case for later bar-alignment inspection.

| Phase | Blocks in 600 s | Render p99 / max (µs) | Dispatch lateness p99 / max (µs) | 10 ms deadline misses | Wrong-output blocks | Render allocations |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Two decks alone | 60,000 | 1,211 / 3,163 | 654 / 1,702 | 0 | 0 | 0 |
| Two decks with full-track analysis | 60,000 | 1,802 / 2,764 | 620 / 1,883 | 0 | 0 | 0 |

The tracker finished 37 songs across seven complete passes plus two songs.
Individual full analyses took 11.48–25.86 seconds. The next analysis was
interrupted at the phase boundary and excluded from the count. Render p99
increased by 591 µs. Neither phase recorded a late start beyond 10 ms or a
render overrun. This measures mixer output and scheduling on one host with a
known constant signal. Hardware underruns and audible glitches require a
device-based check.
