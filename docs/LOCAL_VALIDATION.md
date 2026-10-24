# Local validation — 2026-10-24

Host: Windows x64, rustc 1.99.1. Starting revision: c8e0b70, branch main. Initial tracked working tree was clean. The referenced chat exposed message references only, so its claimed build results could not be recovered; the checkout was verified directly.

## Baseline

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Initial `cargo test`: 12 passed, 1 failed because Slint accessibility debug metadata was missing.
- Fixed `build.rs` to enable that metadata without a CI environment variable. Repeated fmt, Clippy and tests: all passed (13 tests).
- `cargo build --release`: passed before implementing loading/mixer changes.

## Final source verification

All commands below completed with exit code 0 after the final source change:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed, no warnings |
| `cargo test` | 15 passed, 0 failed, 0 ignored; 0.75 s test execution |
| `cargo build --release` | Passed; final rebuild 7.56 s |
| `target/release/drdjpro-ui-spike.exe --analyze tests/fixtures/tone-9s.mp3` | 44100 Hz, stereo, 398592 frames, 9.038 s, peak 0.0883, RMS 0.0592 |
| `git diff --check` | Passed |

An earlier diagnostic `cargo test -- --nocapture` measured the five offline 50 ms mix renders at approximately 0.89–1.64 ms in the debug test binary. These observations include test work and are not real-time callback benchmarks or latency guarantees.

Build artifact: `target/release/drdjpro-ui-spike.exe`. Changes remain local and uncommitted. No visual/native-dialog interaction test, sound-device playback/listening acceptance, cross-platform build or .NET reference parity verification was performed. See AUDIO_PLAYBACK.md and FEATURE_CATALOG.md for scope and remaining limits.

## Seeking follow-up — latest verification (2026-10-24)

The previous loading/mixer slice was preserved. Before editing, ordinary fmt, strict Clippy and all 15 existing tests passed again. This follow-up adds per-deck PCM seeking through the position sliders, preserving transport, gains and deck isolation. Seeking to the end stops; Play restarts; backward seeks after EOF remain paused. Invalid positions are rejected; out-of-range finite values are clamped.

The new stereo render regression initially failed: the Rodio queue reported mono/44.1 kHz before activating a stereo source. A fixed-format output adapter for each replacement sink corrected the channel/rate interpretation. The regression now passes and the adapter is also used for load/Cue. No dependencies were added in this follow-up.

After the final source change, these commands all completed with exit code 0:

| Command | Latest result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed, no warnings |
| `cargo test` | 18 passed, 0 failed, 0 ignored; 0.56 s test execution |
| `cargo build --release` | Passed; 6.70 s |
| `target/release/drdjpro-ui-spike.exe --analyze tests/fixtures/tone-9s.mp3` | 44100 Hz, stereo, 398592 frames, 9.038 s, peak 0.0883, RMS 0.0592 |
| `git diff --check` | Passed |

An earlier diagnostic run measured a control-plane seek at 13.2 microseconds with no audio consumer running. This is an observation, not a latency limit or callback benchmark. The test verifies source sample identity/channel order and position offsets after forward/backward seeking, including paused position freeze. GUI headless tests invoke actual accessible position slider actions for both decks.

Remaining limits: seeking replaces a source abruptly, can overlap the old sink during Rodio control propagation, and has no click suppression or device-latency compensation. PCM stays capped at 256 MiB/track. Rodio is not lock-free. No visual test, native dialog interaction, physical listening acceptance or cross-platform build was performed. The new release binary is at `target/release/drdjpro-ui-spike.exe`; all work remains local and uncommitted, ready for slice review.

## Full-track peak overview — latest verification (2026-10-24)

The existing local loading/mixer/seeking changes were preserved. Before editing, formatting, strict Clippy and all 18 previous tests passed. The bounded follow-up adds `src/waveform.rs`, immutable shared peak data on prepared tracks, separate cached GUI models for each deck, a full-track overview/playhead and click-to-seek. Synthetic bars are now restricted to explicit demo mode. No dependencies were added.

After the final source change, all commands completed with exit code 0:

| Command | Latest result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed, no warnings |
| `cargo test` | 23 passed, 0 failed, 0 ignored; 0.86 s test execution |
| `cargo build --release` | Passed; 7.23 s |
| `target/release/drdjpro-ui-spike.exe --analyze tests/fixtures/tone-9s.mp3` | 44100 Hz, stereo, 398592 frames, 9.038 s, peak 0.0883, RMS 0.0592 |
| `git diff --check` | Passed |

Coverage includes exact peaks for mono, opposite-phase stereo, silence, short tracks, uneven bin boundaries and the last frame; decoded WAV/MP3 peak checks; per-deck model identity across timer, Cue, seek, reload, decode error and cancellation; real headless pointer events at two window widths and accessible overview value actions; marker geometry including the track endpoints. Test windows use the testing backend only: these are event/layout checks, not visual acceptance.

An earlier diagnostic run analyzed 30 seconds of stereo PCM into 256 bins in 212.0467 ms in the debug test binary. This is an observation, not a GUI frame-time, device-latency or real-time callback benchmark. Peak calculation runs during track preparation, outside playback and GUI timer processing; each retained peak overview uses at most 1024 bytes of sample data (excluding allocation metadata/GUI representation).

Updated MIGRATION_PLAN.md, GUI_COMPONENTS.md, FEATURE_CATALOG.md, UI_SPIKE.md and AUDIO_PLAYBACK.md to describe the current implementation and remaining phase gates. Phase 3 remains partial: detailed waveform/zoom/beat analysis, visual review and Windows/macOS rendering evaluation are still open. No screenshot/visual, native-dialog, physical listening, cross-platform build or hard real-time acceptance was performed. The release EXE was rebuilt; changes remain local and uncommitted, ready for slice review.
