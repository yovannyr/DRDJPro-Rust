# Offline track preparation and library assistance

## Beat grid and phrase cues

Build with `cargo build -j1 --locked --features ml-import`. For beat analysis,
set `DRDJ_ORT_LIBRARY` and `DRDJ_BEAT_MODELS` to absolute paths to the CPU
runtime and both verified Beat This! graphs. Install FFmpeg on PATH. Explicit
file/folder imports and Analyze Files run on the analysis worker. The deck load
and audio callback have no model dependency. See the [pinned models, licensing
notes, F1 measurements and 10 ms playback-load results](BEAT-TRACKING.md).

A constant grid requires at least 32 beats and eight downbeats, coverage of
90% of the decoded track, 95% agreement with a 40–240 BPM pulse within 70 ms
and 95% downbeat agreement with a four-beat bar. A qualifying estimate seeds
a new grid draft with the first supported downbeat. Saving the draft is an
explicit action. Otherwise the classical estimate is available. The import
test uses the complete 96 BPM Groove MIDI Dataset funk recording
`drummer8/session1/8` and checks BPM and the first grid anchor.

**Add phrase cues** uses bar-aligned changes in the analyzed amplitude envelope.
A rising section can produce Buildup or Chorus / Drop, a falling section Break
or Outro, and a quieter opening Intro. Flat or insufficient evidence yields no
cues. A saved constant 4/4 grid takes precedence. The action fills empty slots
and can be repeated. The synthetic 80 s fixture checks four internal boundaries
at 16 s intervals and their labels. For recorded references, set
`DRDJ_PHRASE_FILES` to an OS path list of audio files alongside same-stem
`.txt` SALAMI uppercase annotations and run the ignored application test
`score_recorded_phrase_boundaries`. It counts one-to-one internal boundary
matches within three seconds. Personal annotated tracks and Serato grids
are pending. [SALAMI annotations and recording identifiers](https://github.com/DDMAL/salami-data-public).

| SALAMI ID | Internal proposals | Annotated boundaries | Matches within 3 s |
| --- | ---: | ---: | ---: |
| 956 | 7 | 9 | 1 |
| 958 | 6 | 9 | 0 |
| 960 | 7 | 8 | 2 |

On these three complete concert recordings the pooled precision was 15.0%
and recall was 11.5% (F1 13.0%). These are experimental cue suggestions:
amplitude changes alone missed most of the human song boundaries. The counts
use annotator 1, exclude the first and last three seconds, and measure times
only. [Annotation revision](https://github.com/DDMAL/salami-data-public/tree/8e4f95d18a3ab628c53011fa5a43e9d3be27965d).

## Four-stem preparation

Set `DRDJ_STEM_MODELS` to the absolute directory containing
`vocals.fp16.onnx`, `drums.fp16.onnx`, `bass.fp16.onnx` and `other.fp16.onnx`
from the pinned export, and `DRDJ_STEM_CACHE` to an absolute writable directory.
The four graphs total 78,856,560 bytes and are SHA-256 verified before use.
The Spleeter authors release the code and pretrained models under MIT. The
training corpus is private. Weights are installed separately. [Deezer paper](https://github.com/deezer/spleeter/blob/master/paper.md)
and [pinned ONNX conversion](https://huggingface.co/Best-Practice/spleeter-4stems-onnx/tree/486643021f18a4d057d282fff1d11b89a30e9887).

The worker streams stereo at 44.1 kHz through four single-threaded graphs.
The 4,096-sample FFT uses a 1,024-sample hop. Each 512-frame model window
supplies up to 448 central frames to the overlap-add reconstruction. Cache
entries contain interleaved signed 16-bit stems compressed with zlib. The
source hash determines one of two replaceable 128 MiB slots. A writer uses
one additional bounded temporary file before atomic publication. A process-wide
file-system reservation admits one writer across the desktop and CLI and lets
the next writer reclaim abandoned staging data. The reader
checks a digest covering frame count and payload. A long or poorly compressible
track can exceed one slot and finish its ordinary library analysis without stems.

The offline export accepts four gains between zero and one in vocals, drums,
bass, other order. For an isolated vocal WAV, use a binary-safe shell pipeline.

```sh
drdjpro-ui-spike --stem-mix song.flac 1 0 0 0 | ffmpeg -f f32le -ar 44100 -ac 2 -i pipe:0 vocal.wav
```

Qualification used two 14 s stereo arrangements with isolated references
covering a model-window seam. `tools/stem_qualification.py` takes the binary,
a spoken reference and a percussion recording, with optional `--panned`.
The other sources are deterministic bass and keyboard signals. The percussion
is Candombe performance 46 by Martín Rocamora and Luis Jure. [Dataset](https://doi.org/10.60895/redata/AY9CGZ).

| Arrangement | Vocals SDR | Drums SDR | Bass SDR | Other SDR | Reconstructed mix SDR |
| --- | ---: | ---: | ---: | ---: | ---: |
| Centred | 6.97 dB | 1.01 dB | 17.18 dB | 8.29 dB | 70.33 dB |
| Panned | 7.04 dB | −0.22 dB | 20.27 dB | 4.55 dB | 67.87 dB |

The panned drum estimate has more error power than reference signal power.
All four stems improve on using the mixture as the estimate in these fixtures.
A commercial multitrack corpus and listening tests are still needed to judge
isolation on songs. On an i7-11850H Windows development build a 211.82 s
bachata recording took 116.34 s cold and 4.91 s cached to export a full mix
in the initial cache timing run. Repeating with payload checksums while the
test suite and analysis jobs were active took 150.14 s cold and 16.53 s cached,
including the full mix export. The cache file was 131,834,774 bytes. These
are offline processing measurements. Deck loading uses the ordinary audio path.

## Local retrieval, transition suggestions and MCP

Use Python 3.11+ and the saved library SQLite database. Set
`DRDJ_LIBRARY_ASSISTANT` to the absolute installed path of
`tools/library_assistant.py` to enable **Similar** and **Transition** in the
Prepare tab. Keep `library_similarity.py` alongside it. Windows uses
`python` on PATH and other platforms use `python3`. A background process
returns one candidate and explains its evidence. The CLI can return more.

The optional descriptor dictionary embeds 20 fixed descriptions of measured
tempo, key mode and amplitude with `qwen-3-vl-embedding-2b-sovereign`.
Track ranking computes cosine similarity locally from their Gram matrix.
Set `DRDJ_EMBEDDING_DICTIONARY` for the GUI or use `--dictionary` for the CLI.
Without a dictionary, similar-track search ranks measured tempo differences.
Transition ranking combines relative tempo distance, Camelot compatibility,
descriptor similarity and an available Outro/Intro pair. Missing evidence is
shown as unavailable. The entry position can also come from a saved or proposed grid.
Saved grids with tempo changes or warp markers are excluded from constant-tempo
transition ranking. Their anchors remain visible in the track details.

```sh
python tools/library_assistant.py --dictionary dictionary.json build-dictionary https://provider.example/v1
python tools/library_assistant.py --library library.sqlite3 --dictionary dictionary.json similar 12 --limit 5
python tools/library_assistant.py --library library.sqlite3 --dictionary dictionary.json transition 12
python tools/library_assistant.py --library library.sqlite3 --dictionary dictionary.json text "quiet introduction" --endpoint https://provider.example/v1
python tools/library_assistant.py --library library.sqlite3 --dictionary dictionary.json compare-history --limit 50
```

Dictionary generation sends the fixed vocabulary. Explicit text search sends
the entered query to the chosen endpoint. Use `DRDJ_EMBEDDING_API_KEY` for
authentication. Recordings, track names and library paths stay local. The
measured dictionary occupied 118,095 bytes. A query on a 3,484-track library
with 3,471 analyses returned five results. Its 25 aggregate history records
yielded just one unambiguous adjacent pair within ten minutes. Both the tempo
baseline and the embedding ranking missed its successor in the top 50. More
ordered mix history and a DJ evaluation are needed to assess recommendations.

Install `mcp==1.26.0` in the Python environment serving library clients.
Configure the client to launch the following command with absolute paths.

```sh
python -B tools/library_assistant.py --library library.sqlite3 --dictionary dictionary.json serve
```

The stdio server opens the database read-only. It exposes `search_tracks`,
`track_details`, `similar_tracks` and `transition_suggestions`. Search uses a
track-ID cursor. All lists cap at 50 results. Run
`python -B -m unittest discover -s tools -p test_library_assistant.py` for
the transport handshake, tool calls, missing IDs, read-only enforcement,
ranking, phrase cues and dictionary projection.
