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
