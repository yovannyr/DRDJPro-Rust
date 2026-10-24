# Migration plan (draft)

## Reference
.NET repository: https://github.com/yovannyr/DRDJ

Known documented modules: Desktop; Audio.Abstractions; Audio.Engine; Audio.Windows; Audio.Mac; Hardware; Streaming.Abstractions/Fake; Video.Abstractions/Fake. Reference docs include docs/arc42/arc42.md, docs/adrs/, docs/mvp.md, docs/next.md and phase documentation. Verify details before mapping.

## Phases
1. Inventory every requirement, feature, ADR, UI control, audio capability, controller mapping and test. Record source paths and implementation status.
2. Establish target architecture and evaluate Rust libraries and platform requirements; use a Cargo workspace with crates selected by bounded responsibility, not a mechanical project-to-crate conversion.
3. Build a single visual DJ deck spike with waveform, transport, cue and responsive controls; benchmark rendering and inspect visuals on Windows/macOS.
4. Port audio functionality incrementally using behavior and tests from .NET as the reference; validate real-time performance.
5. Integrate hardware, library, persistence and optional streaming/video modules in independently testable slices.
6. Run cross-platform performance, stability and usability tests before feature parity claims.

## Proposed architecture (subject to review)
- app: composition root and application lifecycle
- domain: deck/mixer/library state and commands
- audio: engine, decoding, timing and backend abstractions
- ui: GUI composition and custom controls
- hardware: MIDI/HID/controller adapters
- integrations: optional streaming/video providers

## Decisions to validate
- Slint versus other rendering/UI options, especially custom waveform performance.
- Audio I/O backends and supported sample formats.
- Controller protocol, starting with the Denon controller.
- macOS minimum supported version and older Intel Mac feasibility.
- Feature-by-feature traceability from .NET.

## Budget discipline
One bounded task at a time; reserve capacity for tests, visual review and rework. Do not use a single broad agent prompt for migration.

## Local checkpoint (2026-10-24)

This is an implementation checkpoint, not phase completion or a feature-parity claim. Evidence and exact local commands are recorded in LOCAL_VALIDATION.md; acceptance IDs are in FEATURE_CATALOG.md.

| Phase | Current status | Remaining acceptance |
| --- | --- | --- |
| 1 — inventory | Partial feature/control inventory | Code-level .NET reference and reference-test audit |
| 2 — architecture/evaluation | Cargo workspace and focused modules; Slint/Rodio prototype | Framework choice, audio backend and real-time qualification |
| 3 — representative deck UI | Two decks, real track loading/transport/mixer/seeking, real full-track peak overview and playhead | Detailed waveform evaluation, visual review and rendering/interaction measurements on Windows/macOS |
| 4 — incremental audio | MP3/WAV and deck/mixer/seek behavior headless-tested | Reference parity, DSP/routing/device behavior and physical real-time tests |
| 5 — hardware/library/persistence | File selection foundation only | Library, persistence and controller slices |
| 6 — cross-platform acceptance | Open | Platform builds, long-run stability, performance and usability |

The latest bounded slice replaces synthetic audio-mode bars with real PCM peaks and connects overview clicks to seeking. It intentionally does not add zoom, BPM/beatgrid, hardware or new dependencies. Phase 3 stays partial. Subsequent scope requires review.

## Detail-view follow-up (2026-10-24)

The preceding loading/mixer/seeking/overview slices are published on main as e06dc33. The next local slice adds a following peak-envelope view with independent 2–32-second zoom and click-to-seek in the visible range. It uses precomputed data, not repeated PCM analysis during playback. Phase 3 remains partial: frequency-color/beat visualization, visual review, rendering/frame-time measurements and Windows/macOS evaluation are still open. Do not infer sample-level editing, professional real-time readiness or cross-platform acceptance from the headless tests.

## Frequency-color checkpoint (2026-10-24)

The detail/zoom slice is published on main as 10e3de8. The next local slice adds approximate low/mid/high energy colors to overview and detail views, using analysis during loading and bounded re-binning during UI updates. It changes no audio samples and adds no dependencies. Phase 3 remains partial: beatgrid/BPM, visual approval, rendering/frame-time evidence and Windows/macOS evaluation remain open. The palette communicates approximate frequency energy, not calibrated spectral measurements.

## Manual-grid checkpoint (2026-10-24)

The frequency-color slice is published on main as 54b5ba9. The next local slice adds a user-defined BPM/anchor and 4/4 beat lines to the detail view. It establishes grid timing and interaction without claiming automatic BPM estimation. Phase 3 remains partial: automatic analysis, visual approval, rendering/frame-time evidence and cross-platform evaluation remain open. Grid persistence and tempo/sync behavior require separate slices.

## Tempo-suggestion checkpoint (2026-10-24)

The manual-grid slice is published on main as 999f9b7. The next local slice adds an experimental BPM suggestion during bounded offline track preparation. It is adopted only into the pending BPM field; grid/anchor application remains manual. Deterministic pulse and headless workflow tests establish regression behavior, not music-corpus accuracy or automatic beat alignment. Phase 3 remains partial: production BPM/downbeat work, visual approval, rendering measurements and Windows/macOS evaluation remain open.
