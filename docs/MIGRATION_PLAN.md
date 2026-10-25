# DrDJ Pro product and migration plan

## Product goal and release gate

The target is a complete professional DJ application. Spikes in the historical checkpoints below describe evaluation work, not the intended finished product. Current builds are development checkpoints; production readiness is not established by compilation or headless tests alone.

Release requires a traceable disposition and acceptance evidence for every FEATURE_CATALOG capability and all 59 numbered UI reference items. Core acceptance includes durable library/session data, reliable transport and cue/loop workflows, tempo/sync, mixer DSP, device/output/headphone routing, hardware integration, recovery and measured performance. Optional/provider-dependent features need explicit supported scope rather than hidden stubs. Windows/macOS builds, sustained physical audio/device tests, visual/usability review and representative music-corpus analysis remain release gates.

Deliver in reviewed, bounded slices with working behavior and regression evidence. Keep the existing executable name during development for CI/command compatibility; its historical name does not define the product scope.

## Fortschritt auf einen Blick (2026-10-25)

✅ = im beschriebenen Umfang implementiert und lokal geprüft. ☐ = noch offen. Die Häkchen kennzeichnen einzelne Arbeitspakete; keine der sechs Gesamtphasen ist bereits vollständig abgenommen. Historische Checkpoints weiter unten dokumentieren den damaligen Stand.

### Erledigte Arbeitspakete

- [x] ✅ Alle 59 nummerierten UI-Referenzpunkte im Komponenten-Inventar erfasst; vollständiger Code-/Referenztest-Abgleich bleibt offen.
- [x] ✅ Reale MP3/WAV-Auswahl und Hintergrund-Loading für Deck A/B.
- [x] ✅ Play/Pause, Cue-to-zero, Positionsanzeige und Seek; Schutz vor Ersetzen eines spielenden Decks.
- [x] ✅ Channel-Fader und Crossfader mit der Rodio-Engine verbunden.
- [x] ✅ Extended-Workspace-Struktur mit gemeinsamen Detail-Waveforms und Bibliotheksbereich.
- [x] ✅ PCM-Waveform-Übersicht, folgende Detailansicht, Zoom und approximative Frequenzfarben.
- [x] ✅ Experimenteller BPM-Vorschlag mit begrenzter Offline-Analyse; keine automatische musikalische Ausrichtung.
- [x] ✅ Manueller konstanter 4/4-Beatgrid-Editor mit Vorschau, Save/Cancel, Anker, Slip und SQLite-Speicherung.
- [x] ✅ SQLite-Bibliothek mit stabilen Track-IDs, JSON-Migration, Transaktionen, Backup/Recovery und Speichern beim Beenden.
- [x] ✅ Metadaten für Titel/Artist/Album/Dauer sowie Suche und Sortierung.
- [x] ✅ Flache Crates erstellen, umbenennen, löschen und Tracks zuordnen.
- [x] ✅ Acht persistente Hot-Cue-Slots je Track mit Setzen, Anspringen und Löschen.
- [x] ✅ Hot-Cue-Namen/RGB-Farben mit Save/Cancel und Konfliktschutz bearbeiten.
- [x] ✅ Nummerierte, benannte und farbige Cue-Marker in Übersicht und Detail-Waveform.
- [x] ✅ Aktuellen lokalen Stand geprüft: 101 Tests, `cargo fmt --check`, striktes Clippy und Release-Build grün; siehe [Prüfnachweise](LOCAL_VALIDATION.md).

Veröffentlichungsstand: Saved-Loop-Datenbasis und Slot-GUI sind nach unabhängigem GPT-6-Astra-Mittel-Review als `7778eec` gepusht. Auto-Loops und Halbieren/Verdoppeln sind lokal implementiert; Review und Veröffentlichung folgen dem festgelegten Ablauf.

### Nächste und offene Arbeitspakete

- [x] ✅ Manuelle Loop-Engine mit framegenauem In/Out, Aktivieren/Verlassen und deterministischen Offline-Tests.
- [x] ✅ Loop-In/Out, Aktivieren/Verlassen und echte Loop-Zustandsanzeige in beiden Decks.
- [x] ✅ Datenbasis für acht persistente Saved-Loop-Slots je Track mit validierten Grenzen/Namen und SQLite-Migration.
- [x] ✅ Saved-Loop-Slots in der GUI speichern, abrufen und löschen, mit Quellvalidierung und Neustart-Tests.
- [x] ✅ Beatbasierte Auto-Loops (⅛–32 Beats) aus bestätigter manueller BPM und Halbieren/Verdoppeln bestehender Loop-Grenzen.
- [ ] ☐ **Als Nächstes:** Loop-Bereichsmarkierungen in Übersicht und Detail-Waveform.
- [ ] ☐ Tempo/Pitch, Nudge, Sync, Keylock und Quantisierung.
- [ ] ☐ Mixer-DSP: EQ, Filter, Gain/Meter und Effekte.
- [ ] ☐ Audio-Gerätewahl, Ausgangs-/Kopfhörer-Routing und Vorhören.
- [ ] ☐ MIDI/HID-Controller und Hardware-Feedback.
- [ ] ☐ Erweiterte Bibliothek: Sub-/Smart-Crates, History, Relocation und Session-Wiederherstellung.
- [ ] ☐ Erweiterte Grid-Funktionen: variable Tempi, Warp-Marker und Taktartwechsel.
- [ ] ☐ Cue-Tastatur-/Hold-Verhalten und Prüfung überlappender Marker.
- [ ] ☐ Vollständiger .NET-Code-/Referenztest-Abgleich und Feature-Paritätsnachweis.
- [ ] ☐ Unterstützten Umfang optionaler Streaming-/Video-Integrationen festlegen und umsetzen.
- [ ] ☐ Slint-/Audio-Backend-Auswahl durch Rendering- und Echtzeitmessungen abschließen.
- [ ] ☐ Native visuelle/Usability-Abnahme, Musik-Korpusprüfung und reale Audio-/Latenz-/Langzeittests.
- [ ] ☐ Windows/macOS-Plattformqualifikation und abschließende Produktabnahme.

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

## Hot-cue persistence foundation checkpoint (2026-10-25)

The accumulated SQLite/metadata/crate/grid work is published on main as 6412d46; local/remote equality and a clean worktree were verified. The next authorized bounded slice adds eight fixed per-track cue slots with validated position/name/color metadata, library snapshot preservation and transactional SQLite schema 5. It deliberately ends at the data boundary. Migration/restart, sparse-slot identity, explicit removal, invalid/orphaned rows, rollback and retry are automated acceptance targets. No dependency, GUI or audio callback change is needed.

Next proposed slice after review: connect deck hot-cue pads to set/jump/delete operations using actual decoded source positions and duration checks, retaining transport and other-deck isolation, with headless tests. Names/colors editing, waveform cue markers and hardware feedback require explicit acceptance in those UI slices. Overview 48/58 remains partial; a stored model alone does not establish professional cue behavior.

## Hot-cue deck integration checkpoint (2026-10-25)

Hot-cue storage was committed/pushed as 29b52f8 before this user-authorized continuation. This bounded slice connects eight per-deck pads to set/recall and explicit slot deletion. Absolute source-time seeks preserve Play/Pause, grid and sink volumes. Cached cues are shared by canonical track identity and restored with the library. Invalid slot/EOF/out-of-duration actions fail without changing playback; changes persist through the existing worker. No schema/dependency/audio-callback change was required.

Acceptance uses actual headless controls, engine tests and temporary-profile restart/deletion tests, retaining all prior audio/storage regressions. Native visuals/listening/physical latency remain unqualified. Next proposed slice after review: cue name/color editing and waveform cue markers, with reference-backed interaction and headless tests. Quantization, controller feedback and the full professional release gates remain open.

## Cue metadata editor and waveform markers checkpoint (2026-10-25)

The user authorized the next bounded slice after the 86-test deck-pad checkpoint. Per-deck Edit now opens isolated name/RGB fields with explicit Save/Cancel, validated six-digit hex colors and existing Unicode name bounds. Saved positions are immutable in this editor. Source identity and original cue values prevent stale drafts overwriting same-track edits/deletion. Successful source replacement discards a draft; failed replacement retains it. Committed updates propagate to both same-path decks and persist through existing schema 5.

Numbered, named/color-coded markers appear in overview and visible detail ranges; cues outside loaded duration or viewport are hidden. Marker placement follows zoom/seek, and unchanged cue models are reused. Headless workflow/geometry/restart coverage is recorded in LOCAL_VALIDATION.md. No schema/dependency/audio-callback change was needed. The preceding deck integration and this editor/marker slice remain local and uncommitted; 29b52f8 is the latest published checkpoint.

Next proposed bounded slice after review: manual loop-in/out, activate/exit and deterministic offline engine behavior, then GUI controls and persistent saved loops in separately reviewed slices. Cue keyboard/hold behavior, quantization, controller feedback, marker overlap/usability and all native visual/physical-audio gates remain open.

## Manual loop engine checkpoint (2026-10-25)

The user authorized the bounded engine-first loop slice. A shared immutable PCM source replaces SamplesBuffer and uses integer frame addressing; active loops repeat [In, Out) without allocation/locks in its next() implementation. The engine can set In/Out, activate from In preserving Play/Pause, and exit at the current wrapped source position. Seek/hot-cue recall and Cue-to-zero deactivate looping while retaining bounds; selecting a new In clears Out; successful track installation resets bounds and failed preparation preserves them. Out may equal EOF; empty/reversed/out-of-source and subframe-empty regions are rejected. Existing mixer/grid/source ownership remains intact. No dependency/schema/GUI was added; the staged loop-control API awaits the next reviewed GUI slice.

Acceptance includes deterministic stereo output over many wraps, one-frame loops, integer frame-time round trips, allocation counting of one million prepared-source samples, real Rodio offline engine looping/exit/EOF, pause, failed/successful replacement and existing regressions. These do not qualify the surrounding Rodio pipeline as lock-free or prove physical real-time/latency or click-free musical transitions. GUI position reporting still uses Rodio's control-plane position timing with wrapped loop phase. Next proposed bounded slice after review: GUI In/Out/activate/exit controls and displayed loop state, with headless workflows; saved-loop persistence/quantization follow separately. All continuation work after 29b52f8 remains local/uncommitted.

## Manual loop GUI checkpoint (2026-10-25)

The user requested commit/push and continuation. All preceding cue/UI/engine/documentation work was committed and pushed as 9a1c7b7; remote main was verified at that SHA. This bounded follow-up adds four manual-loop controls per deck and a caption derived from actual engine In/Out/active state. In/Out capture the current source position; activation preserves Play/Pause, and Exit preserves current loop phase. Controls/callbacks are guarded during loading/restore/import, with errors reported through status. No schema, dependency or audio-source callback behavior changed.

A headless actual-control test covers unloaded/invalid boundaries, paused/playing activation, offline wrap, pause/exit, seek/Cue deactivation with retained bounds, failed/successful replacement, loading guards and other-deck/Sink-volume isolation. Two-size layout regression includes all loop controls. Native usability/visuals, listening and physical latency remain unqualified. Next proposed bounded slice after review: durable per-track saved loop slots and schema migration/restart/rollback tests, followed by saved-loop GUI integration. Auto-loop lengths/beat quantization and hardware feedback remain open. Current GUI changes remain local and uncommitted.

## Saved-loop data foundation checkpoint (2026-10-25)

The user requested commit/push and continuation. The manual loop GUI was published as 1fb3e91 and remote main verified at that SHA. This bounded slice adds eight fixed saved-loop slots per stable track identity, each with nanosecond In/Out and a Unicode name. The eight-slot limit is the explicit bounded implementation choice for this slice, not proof of external format compatibility. Schema 6 stores these independently of transient per-deck active-loop state. No playback activation, GUI, audio callback, dependency or tag-write behavior changed.

Model validation requires In < Out within seven days, names up to 128 Unicode scalar characters without controls and slots 0–7. Schema 1–5 upgrades, restart/clear, malformed/orphaned/duplicate rows and atomic rollback/retry alongside metadata/cues/grid/crates are tested. Stored duration bounds are intentionally independent of sample rate; loading into the engine in the following slice must revalidate actual decoded duration and reject regions that collapse to zero frames. Next proposed reviewed slice: GUI saved-slot save/recall/delete and headless restart/source-duration tests. Auto-loop lengths, quantization, waveform overlays and hardware/visual/physical-audio acceptance remain open. Current data changes remain local/uncommitted.

## Saved-loop GUI integration checkpoint (2026-10-25)

The user authorized continuing after the 96-test data foundation. Per-deck SLOTS switches the cue-pad area to saved-loop slot/name/Save/Recall/Delete controls and a stored-name/bounds description, keeping the deck at 260 px. Save requires valid current manual boundaries and an empty target; occupied slots reject replacement until explicit deletion. Recall validates loaded duration and nonempty complete frames atomically, then loads inactive boundaries without seeking or starting playback; LOOP remains the activation command. Delete changes only persistent slot metadata, including while a deck loop remains active. Same-path decks share saved data; different tracks retain independent sets. No schema/dependency/audio callback change was needed.

EOF bounds round-trip through snapshots without exceeding the decoded duration by fractional-nanosecond rounding. Tests cover source-invalid/subframe intervals retaining active loop/playback state, playing/paused recall, Unicode names, empty/occupied/invalid/loading cases, shared-track updates, fresh controller/engine/storage restart and deletion, and both panel layouts at 850x740 and 1280x860. Early layout tests caught overflow from adding rows; the switchable panel resolved it. Native visuals/listening/physical latency remain unqualified. Saved-loop data and GUI work remain local/uncommitted after published 1fb3e91.

Next proposed reviewed slice: beat-based auto-loop lengths and half/double operations based on the explicitly defined manual grid, with integer-frame bounds and offline/headless tests. Quantization/beat snapping, loop waveform overlays, saved-slot rename/replace drafts, musical grid correctness and hardware feedback remain separate open requirements.

## Auto-loop and resize checkpoint (2026-10-25)

Following the new standing workflow, GPT-6 Astra (gpt-6-astra, medium) independently reviewed the saved-loop data/GUI slice without blocking findings; independent and final author tests passed before commit/push 7778eec. This next bounded slice adds 1/8, 1/4, 1/2, 1, 2, 4, 8, 16 and 32 beat auto-loops using the committed manual grid BPM. Start is the current decoded frame, not a snapped beat. Beat duration is rounded once to whole frames; the immutable source repeats that region without cumulative boundary drift. Creation activates at In preserving Play/Pause. Invalid/missing-grid/out-of-track requests leave playback and bounds unchanged.

Half/double keeps In fixed and scales the existing integer-frame span. Halving uses integer floor (up to one frame discarded); a one-frame loop cannot be halved. Active resizing maps current source phase modulo the new span and preserves Play/Pause; inactive resizing only changes boundaries without seeking or activation. These operations also support manually defined loops and do not require a new grid. Saved slots are not rewritten; explicit Save can capture the resulting region.

The deck tools switch now cycles CUES/SLOTS/AUTO within the same 260 px deck height. AUTO contains beat length, Auto Loop and half/double controls with actual committed-grid/loop/loading guards. Domain/offline/headless and three-panel two-size regressions verify behavior. No schema/dependency/audio callback change was needed. Native visuals/listening, beat snapping/quantization, musical grid correctness and physical timing remain unqualified. Next proposed bounded slice is visible loop ranges in overview/detail waveforms; name/replace drafts and controller feedback remain open. Publication requires the standing independent-review/final-check gate.
