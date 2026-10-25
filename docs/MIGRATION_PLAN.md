# DrDJ Pro product and migration plan

## Product goal and release gate

The target is a complete professional DJ application. Spikes in the historical checkpoints below describe evaluation work, not the intended finished product. Current builds are development checkpoints; production readiness is not established by compilation or headless tests alone.

Release requires a traceable disposition and acceptance evidence for every FEATURE_CATALOG capability and all 59 numbered UI reference items. Core acceptance includes durable library/session data, reliable transport and cue/loop workflows, tempo/sync, mixer DSP, device/output/headphone routing, hardware integration, recovery and measured performance. Optional/provider-dependent features need explicit supported scope rather than hidden stubs. Windows/macOS builds, sustained physical audio/device tests, visual/usability review and representative music-corpus analysis remain release gates.

Deliver in reviewed, bounded slices with working behavior and regression evidence. Keep the existing executable name during development for CI/command compatibility; its historical name does not define the product scope.

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
| 3 — representative deck UI | Extended workspace, dual transport/mixer/seek, peak/color detail views and constant-grid draft editor | Visual review, rendering/interaction measurements and Windows/macOS framework qualification |
| 4 — incremental audio | MP3/WAV and deck/mixer/seek behavior headless-tested | Reference parity, DSP/routing/device behavior and physical real-time tests |
| 5 — hardware/library/persistence | Searchable local catalog with title/artist/album/duration, durable SQLite storage, flat crates, manual grids and backup recovery | Subcrates/smart crates/history, relocation, session restore and hardware slices |
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

## User-provided UI reference baseline

`Softwareübersicht.md`, its numbered `Softwareübersicht.png`, and `DRDJ-UserManual-DRDJ.pdf` now provide the primary layout/interaction references for upcoming UI slices. GUI_COMPONENTS.md maps all 59 overview items, records focused manual page references, and separates current Rust behavior from reference requirements.

Priority within phase 3 is now to align the working prototype with the Extended reference workspace: compact deck metadata/overview areas, shared detailed waveforms, a substantial library region and separate status. Preserve engine wiring and regression tests while changing layout. Add additional display modes and full grid/library editors only in separately scoped steps. More analysis features alone do not close the current layout gap.

The manual is a behavior reference, not proof of Rust feature parity or a decision to implement Serato-specific account/licensing/hardware/library compatibility. Image-only reference pages were inspected; the application has not thereby passed visual acceptance. Phase 3 remains partial.

## Extended workspace and session library checkpoint (2026-10-24)

Implemented compact deck heads/overviews, shared stacked detail waveforms, retained engine-connected mixer controls, a resizable library region and separate status. Reusable Slint components retain transport, seek, zoom and grid behavior. Real MP3/WAV file imports run through a background worker; the session table supports case-insensitive search, name/type/path sorting, stable selection, arrow navigation and paused loading into either deck. Direct successful loads also enter the table. Replacing a playing deck is blocked, including a second check after background decoding.

This closes the scoped layout structure and session-browser foundation, not phase 3 or the complete library. Headless geometry tests do not establish visual acceptance. Import is limited to 5,000 selected paths per operation and 100,000 session entries; these limits are safeguards, not performance qualification. Extension/existence inspection occurs at import and actual decode validation at load. Failed loads retain the installed track.

Next proposed slice, after review: durable local library storage with atomic save/recovery, stable identity across restarts, missing-file handling and restart tests. Then metadata/crates/history and a transactional grid editor, followed by prioritized cue/loop, tempo/sync, mixer/routing and hardware slices with reference acceptance. Native-dialog, visual, physical-audio and platform qualification must accompany the appropriate release gates.

## Durable catalog checkpoint (2026-10-24)

The Extended workspace/session-library slice and the preceding user UI/reference commit are now published on main as 45285cf (parent f50fba4). Local and remote heads were verified equal with a clean worktree before starting this slice. Both published trees were checked against their original local commit trees; the CLI's wrong-account credentials were not changed.

The current bounded slice adds a versioned, user-local library catalog with stable track IDs and paths, background startup/save, same-directory atomic file replacement, previous-good backup recovery and a process-owned OS file lock. Missing files remain listed and marked; errors preserve installed deck audio. Unrecoverable or unknown-version files are preserved and the application explicitly switches to session-only library behavior. Write failures stay visible with Retry Save. Normal shutdown flushes the latest catalog on the storage worker; a final save failure returns an error exit.

See LIBRARY_STORAGE.md for format, paths, recovery boundaries and acceptance evidence. Next proposed slice after review: track metadata and durable crates, beginning with schema evolution and reference-backed acceptance. Session/deck restoration, relocation tools, full grid editor, visual/audio qualification and the remaining professional feature gates stay open.

## SQLite storage checkpoint (2026-10-24)

The JSON persistence slice is published as 9f75010. At the user's request the current bounded slice replaces its writer with bundled SQLite through rusqlite, keeping the worker/controller boundary and paused A/B loading intact. An application-tagged version-1 schema stores stable IDs/paths. A one-time transaction migrates validated JSON primary/backup data without modifying either source. Initialized SQLite always wins over retained JSON. Unknown/foreign databases and unrecoverable files are preserved with the established visible session-only failure behavior.

Saves update changed rows transactionally, preserve a previous-good SQLite online backup, and flush the latest snapshot on close. Regression acceptance includes migration/restart, ID/path preservation, legacy/current locks, write contention/retry, transaction rollback, backup recovery and forced child-process termination followed by SQLite journal recovery/integrity check. See LIBRARY_STORAGE.md and LOCAL_VALIDATION.md for exact evidence and limits. Full metadata/crates work is the next proposed slice after review; this change does not implement those tables or expand product acceptance claims.

## Metadata follow-up checkpoint (2026-10-24)

After the SQLite slice passed 57 tests and release compilation, the user authorized continuing. The next local slice adds worker-based MP3/WAV header/tag inspection, cached title/artist/album/duration, search/sort/table columns and schema 1-to-2 migration without renumbering tracks. Header durations are marked approximate; fully loaded tracks replace them with decoded PCM duration. Missing tags fall back honestly and failed metadata inspection does not discard the library path or replace an installed deck. No new dependency was required.

Acceptance covers generated UTF-8 ID3v2.4 and WAV INFO tags, untagged/invalid input, text/duration bounds, successful/failed schema upgrade, retained cached metadata for missing files and headless table search/numeric sort/restart/paused A/B loading. Existing SQLite transaction/crash-recovery and audio/waveform regressions remain required. Next proposed reviewed slice: durable crates and track membership; tag editing, artwork, watchers, paging and full professional release gates remain open. SQLite and metadata changes are still local, not yet published.

## Flat-crate checkpoint (2026-10-24)

The user authorized continuing from the green 64-test SQLite/metadata checkpoint. This bounded slice adds pure crate/set-membership logic, stable durable crate IDs, schema-3 migration, left navigation, create/rename Save/Cancel, confirmed deletion, target-based add/remove, scoped search/counts and captured-crate file imports. Crate operations leave global tracks, files and loaded/playing decks intact. Source overview controls 51 and 54 now have partial concrete implementation; nested/smart crates, external compatibility, member ordering and autoplay are still open.

Acceptance includes name/identity/reference validation, filter-selection behavior, persistence and ID non-reuse after deletion/restart, failed relationship-save rollback, schema-2 upgrade/failure with metadata preservation, and actual headless GUI workflow through import/assignment/rename/delete/cancel/restart/paused B loading while A remains playing. No new dependency or audio callback change was needed. See LOCAL_VALIDATION.md for command evidence. Next proposed bounded slice after review: the transactional Beatgrid editor; advanced crate/library behavior and all professional release gates remain tracked. All current SQLite/metadata/crate changes remain local, not yet published.

## Transactional constant-grid editor checkpoint (2026-10-24)

The user authorized continuing from the green 72-test crate checkpoint. This slice replaces immediate GUI grid changes with isolated per-deck drafts, decimal BPM, first-downbeat placement, ±10 ms grid slip, staged Clear and explicit Save/Cancel. Zoom/seek are retained; bar labels and the first-downbeat marker are derived from the manual constant 4/4 anchor. Preview cannot alter the active grid or audio until Save, and even Save only changes grid metadata, not tempo/pitch or PCM. Simultaneous drafts detect saved-grid conflicts; successful track replacement discards stale drafts while failed/cancelled loading preserves them.

SQLite schema 4 stores optional grids by stable track ID, preserving fractional BPM/nanosecond anchors and restoring them through application loads. Upgrades from schemas 1–3 and grid writes preserve metadata/crates transactionally. Same-track loaded decks share compatible committed grid data. Variable-tempo/warp markers, meter detection, native visual acceptance and musical correctness remain open; overview 46 is partial rather than complete parity. Commands and headless/engine/store evidence are in LOCAL_VALIDATION.md. No dependency or audio callback change was required. Next proposed bounded slice after review: persistent hot cues (overview 48/58); advanced grid requirements stay tracked. All accumulated work remains local and uncommitted.
