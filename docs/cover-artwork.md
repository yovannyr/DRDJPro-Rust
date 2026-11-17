# Embedded cover artwork — Modern decks and Library

## Behavior and scope

Modern A/B headers and platters display embedded cover images. The common Library
table gains column 11, Cover, including its existing tabs. Thumbnails fit entirely
inside the unchanged 22px rows without cropping or distortion. Cover can be moved
or hidden; it has no sorting action. Legacy 11-column preferences are validated
and upgraded without losing relative order or hidden columns.

Extraction examines both probe metadata (e.g. ID3 APIC) and format metadata (e.g.
FLAC PICTURE/MP4 artwork), preferring a decodable front cover, then other pictures.
Missing, malformed, unsupported or excessive artwork uses the existing background
or an empty cell. No network artwork search or adjacent folder images are added.
C/D remain the existing unsupported/read-only slots without a loaded audio source.

## Performance and resource bounds

- Track preparation no longer extracts artwork or retains full RGBA covers.
  Audio decoding, tempo analysis and realtime rendering are otherwise unchanged.
- One separate worker performs cover I/O/decoding; UI lookups only inspect the
  cache and use bounded `try_send`. No joining or filesystem calls in UI/audio.
- At most four tracked requests, bounded request/result channels of four entries,
  and at most four completions processed per UI tick. One stalled media read may
  delay later covers, but it cannot block track-load completion or the UI thread.
- PNG/JPEG decoding input is capped at 4 MiB after metadata extraction. Source
  dimensions are capped at 4096px with a best-effort 32 MiB decoder allocation
  limit. Unsupported/oversized inputs are rejected. Output is at most 128×128 RGBA,
  preserving aspect ratio without upscaling small images.
- 256 LRU entries fit a complete 200-row library page plus decks, avoiding repeated
  eviction/redecoding on each repaint. Cache-owned RGBA pixel data is at most
  **16 MiB**; this is not the total application RAM budget. Decoder/probe memory,
  queued thumbnails, transient copies, displayed Slint handles and GPU resources
  are additional. Slint image clones share their pixel storage.
- The probe requests 512 KiB metadata and 4 MiB visual limits, but upstream
  Symphonia 0.5.5 does not enforce them in every reader. In particular ID3 frames
  can allocate declared payload sizes before our image checks. Thus malformed or
  excessive tags can exceed the thumbnail/decoder budgets during probing.
  General metadata-parser/OOM hardening remains open; this change does not claim
  a hard probe or total-process RAM limit. One worker limits concurrency only.
- Negative results are cached. Explicit reload invalidates that file's entry;
  generations discard stale results. Windows verbatim prefixes are normalized
  without filesystem I/O, Unicode replacement or case-folding distinct files.
- Library requests are lazy; notifications/retries are bounded by the existing
  200-row page. Hiding Cover avoids library thumbnail requests. There is no disk
  image cache, media rewrite or database image blob.

## Automated verification and independent review (2026-11-17)

Windows, Rust 1.99.1: `cargo test --workspace --locked` passed **2,464 tests**,
with six existing ignored tests. `cargo +1.100.0 clippy --workspace --locked
--all-targets --target x86_64-pc-windows-msvc -- -D warnings`, formatting and
`git diff --check` passed. No dependencies or permanent build profiles changed.

Regression coverage includes embedded ID3/FLAC images; PNG/JPEG decoding,
proportional downscaling and excessive/corrupt/missing input; bounded queue/LRU
and complete-page reuse; stale generations; native Windows path identity;
hidden-to-visible delegate invalidation; shared deck/library images and eject;
legacy persisted columns; and track-load completion while the cover worker is
deliberately blocked. Independent review: GPT-6.1 Sol, medium; actionable findings
were corrected and follow-up review found no remaining blockers for this scope.

### Direct loading comparison

The unmodified previous Git state `17755b1` was built from a temporary source
archive with the same Rust version and **Dev profile** as the feature. Compiler
invocations confirmed app optimization level 1; codec optimization stays at 2.
The existing asynchronous Controller A/B benchmark and its strict five-second
assertion were unchanged. Four read-only Deto files were measured in three
alternating before/after rounds, with no simultaneous compilation or playback.

Median seconds from three rounds per file/deck:

| File in `E:/DJ_Music/08_DJ_Remixes/Deto` | Before A | After A | Before B | After B |
|---|---:|---:|---:|---:|
| `01 ritmo latino- Unbekannter Titel.mp3` | 3.051 | 2.698 | 3.664 | 3.278 |
| `02 -variado desconocido.mp3` | 2.696 | 2.498 | 2.759 | 2.346 |
| `bachata especial.mp3` | 3.214 | 3.021 | 3.030 | 2.715 |
| `especia bachats.mp3` | 3.823 | 3.728 | 4.349 | 3.806 |

All **24 feature loads passed**, maximum **4.102 s**. The reference had one
timeout exceeding five seconds for `especia bachats.mp3`, deck B, in round 1;
it is recorded as a failure, not an exact five-second observation. Its three-run
median is still determined by the other two measured values below five seconds.
The aggregate median changed from 3.136 s to 2.928 s. No added loading delay was
observed in these cases; these measurements do not establish a universal speedup.

An earlier diagnostic run of the default **unoptimized Test profile** (app level
0) failed the same real-file deadline. Its phase trace showed 3.532 s for detail
waveform/colors and 4.958 s total before Controller completion on the first file.
This result is retained, not hidden by changing the assertion or permanent
profiles. Interactive-app timing therefore explicitly uses `--profile dev`.

Local evidence: shared target's `cover-workspace-tests.log`,
`cover-deck-load-benchmark.log` (unoptimized failure), `cover-deck-load-dev.log`,
`cover-deck-load-baseline.log`, `cover-comparison-{before,after}-{1,2,3}.log`
and `cover-comparison-summary.json`.

Native visual acceptance remains separate and is performed by the user.
Concurrent disk/CPU activity can affect wall-clock timing; no universal
five-second guarantee for all media and hardware is implied.
