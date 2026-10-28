# DrDJ Pro product and migration plan

## Product goal and release gate

The target is a complete professional DJ application. Spikes in the historical checkpoints below describe evaluation work, not the intended finished product. Current builds are development checkpoints; production readiness is not established by compilation or headless tests alone.

Release requires a traceable disposition and acceptance evidence for every FEATURE_CATALOG capability and all 59 numbered UI reference items. Core acceptance includes durable library/session data, reliable transport and cue/loop workflows, tempo/sync, mixer DSP, device/output/headphone routing, hardware integration, recovery and measured performance. Optional/provider-dependent features need explicit supported scope rather than hidden stubs. Windows/macOS builds, sustained physical audio/device tests, visual/usability review and representative music-corpus analysis remain release gates.

Deliver in reviewed, bounded slices with working behavior and regression evidence. Keep the existing executable name during development for CI/command compatibility; its historical name does not define the product scope.

## Fortschritt auf einen Blick (2026-10-27)

✅ = im beschriebenen Umfang implementiert und lokal geprüft. ☐ = noch offen. Die Häkchen kennzeichnen einzelne Arbeitspakete; keine der sechs Gesamtphasen ist bereits vollständig abgenommen. Historische Checkpoints weiter unten dokumentieren den damaligen Stand.

### Erledigte Arbeitspakete

- [x] ✅ Alle 59 nummerierten UI-Referenzpunkte im Komponenten-Inventar erfasst; vollständiger Code-/Referenztest-Abgleich bleibt offen.
- [x] ✅ Reale MP3/WAV-Auswahl und Hintergrund-Loading für Deck A/B.
- [x] ✅ M4A (AAC-LC/ALAC), FLAC, AIFF/AIF und Ogg/Vorbis durch Dateiauswahl, Bibliothek und beide Decks; Weitere Codecs bleiben offen.
- [x] ✅ Ogg/Opus (Mono und Stereo) durch Dateiauswahl, Bibliothek und beide Decks; verkettete Streams und Mehrkanal bleiben offen.
- [x] ✅ Play/Pause, Cue-to-zero, Positionsanzeige und Seek; Schutz vor Ersetzen eines spielenden Decks.
- [x] ✅ Channel-Fader und Crossfader mit der Rodio-Engine verbunden.
- [x] ✅ Beschriftete verstrichene Zeit, Restzeit und Gesamtdauer in Original-Trackzeit mit Millisekunden für beide Decks; native Lesbarkeitsprüfung bleibt offen.
- [x] ✅ Extended-Workspace-Struktur mit gemeinsamen Detail-Waveforms und Bibliotheksbereich.
- [x] ✅ PCM-Waveform-Übersicht, folgende Detailansicht, Zoom und approximative Frequenzfarben.
- [x] ✅ Fokusgebundener Plus-/Minus-Tastaturzoom der Detail-Waveforms mit Grenzen2–32s und Schutz fokussierter Texteingaben; native Fokusabnahme offen.
- [x] ✅ Experimenteller BPM-Vorschlag mit begrenzter Offline-Analyse; keine automatische musikalische Ausrichtung.
- [x] ✅ Manueller konstanter 4/4-Beatgrid-Editor mit Vorschau, Save/Cancel, Anker, Slip und SQLite-Speicherung.
- [x] ✅ SQLite-Bibliothek mit stabilen Track-IDs, JSON-Migration, Transaktionen, Backup/Recovery und Speichern beim Beenden.
- [x] ✅ Metadaten für Titel/Artist/Album/Dauer sowie Suche und Sortierung.
- [x] ✅ Flache Crates erstellen, umbenennen, löschen und Tracks zuordnen.
- [x] ✅ Acht persistente Hot-Cue-Slots je Track mit Setzen, Anspringen und Löschen.
- [x] ✅ Hot-Cue-Namen/RGB-Farben mit Save/Cancel und Konfliktschutz bearbeiten.
- [x] ✅ Nummerierte, benannte und farbige Cue-Marker in Übersicht und Detail-Waveform.
- [x] ✅ Aktuellen lokalen Stand geprüft: 201 Tests, `cargo fmt --check`, striktes Clippy und Release-Build grün; siehe [Prüfnachweise](LOCAL_VALIDATION.md).

Veröffentlichungsstand: Die sieben bewerteten Opus-Befunde wurden in separat geprüften Schritten bis0819327 abgearbeitet und gepusht. Tempo-Matching ist als b143276 gepusht. Der anschließende einmalige Beat-Phasen-Sync hat148 grüne Tests, einen erfolgreichen Release-Build und ein akzeptiertes unabhängiges Astra-Mittel-Follow-up nach Korrektur eines Rundungsfehlers. Der gemeinsame Render-Bus ist als4b3009f gepusht. Kohärente Phasen-Veröffentlichung ist als7a43187 gepusht. Frischeprüfung ist als d45d31b gepusht. Der begrenzte kontinuierliche SYNC-Follower ist als f053a30 gepusht. Beat-/4/4-Takt-Auswahl ist als1a6f771 gepusht. Fortlaufende Taktphasenregelung ist als9e27c8c gepusht. Der anschließende QWEN-Schritt zur Quantisierung neuer Cues ist mit171 Tests, Release-Build und akzeptiertem GPT-6.1-Sol-Mittel-Review geprüft. Präzise Quellzeitanzeige ist als3e545b5 gepusht. Der anschließende fokusgebundene Tastaturzoom ist mit175 Tests, Release-Build und akzeptiertem Sol-Mittel-Review geprüft. Der gemessene Bus-Sample-Peak ist anschließend mit180 Tests, Release-Build und akzeptiertem Sol-Mittel-Review geprüft. Commit-Zuordnung und Prüfnachweise siehe Git-Historie und LOCAL_VALIDATION.md.

### Nächste und offene Arbeitspakete

- [x] ✅ Manuelle Loop-Engine mit framegenauem In/Out, Aktivieren/Verlassen und deterministischen Offline-Tests.
- [x] ✅ Loop-In/Out, Aktivieren/Verlassen und echte Loop-Zustandsanzeige in beiden Decks.
- [x] ✅ Datenbasis für acht persistente Saved-Loop-Slots je Track mit validierten Grenzen/Namen und SQLite-Migration.
- [x] ✅ Saved-Loop-Slots in der GUI speichern, abrufen und löschen, mit Quellvalidierung und Neustart-Tests.
- [x] ✅ Beatbasierte Auto-Loops (⅛–32 Beats) aus bestätigter manueller BPM und Halbieren/Verdoppeln bestehender Loop-Grenzen.
- [x] ✅ Loop-Bereiche mit In/Out-Markierungen in Übersicht/Detail, präzisem Clipping und Anzeige von Aktiv/Inaktiv.
- [x] ✅ Optionale Beat-Ausrichtung neu erzeugter Auto-Loops anhand des bestätigten manuellen Grids.
- [x] ✅ Engine-Grundlage für gekoppelte Playback-Geschwindigkeit/Pitch 0,5–2,0×, mit Quellzeit- und Offline-Signaltests.
- [x] ✅ Pitch-Fader/Reset mit gebündelten, quellgebundenen Anfragen und Anzeige von gewünschtem/effektivem Pitch sowie Quell-/aktuellem BPM.
- [x] ✅ Pitch-Bereiche ±8/±16/±50/Wide und driftfreie 0,01-Prozentpunkt-Feinschritte ohne stillen Rate-Reset beim Bereichswechsel.
- [x] ✅ Engine-Zustand für temporäres relatives ±2%-Nudge, Basis-Pitch-Erhalt und sicheren neutralen Release/Transport-Abbruch.
- [x] ✅ Momentane Nudge-GUI mit Pointer-/Tasten-Release, Fokus-/Panel-/Lade-/Transport-/EOF-/Shutdown-Abbruch und Headless-Ereignistests.
- [x] ✅ Opus1: Performance-Steuerung deckweise von Laden/Import entkoppelt; Restore-/Quellschutz und persistente Cue-Änderungen getestet.
- [x] ✅ Opus3: alte PCM-Besitzer bis zum tatsächlichen Queue-Retirement halten; Freigabe im Kontrollthread mit separatem Pull-Thread getestet.
- [x] ✅ Opus2: feste Ausgabeformate, frameweise Render-Position und Pitch/Nudge ohne Sink-Neustart;50 kurze Flanken gegen PCM-Rampe geprüft.
- [x] ✅ Opus6: lesbaren Katalog bei Metadaten-Backfill-Schreibfehler erhalten; Retry und Neustart mit injiziertem Schreibfehler getestet.
- [x] ✅ Opus5: revisionsgebundene Track-Deltas, begrenzte Zeilenänderungen, periodische geprüfte Backups und Retry/Close; Snapshot bei100000 Tracks vermessen.
- [x] ✅ Opus7: PCM-Puffer ohne Sample-Kopie teilen, fallibel begrenzt reservieren und Lade-/Import-Worker kooperativ abbrechen; späte Ergebnisse verworfen.
- [x] ✅ Crate-Mitgliedschaften revisionsgebunden einzeln speichern; Rollback, parallele Änderungen und Neustart geprüft.
- [x] ✅ Crate-Strukturänderungen revisionsgebunden einzeln speichern, einschließlich Namenswechsel, ID-Fortschreibung und Rollback.
- [x] ✅ Bibliothekszeilen im Slint-ListView bedarfsweise erzeugen; 20.000 Tracks headless auf begrenzte Zeilenanforderungen geprüft.
- [x] ✅ Unveränderte Bibliotheksansichten wiederverwenden; bei Änderungen an Suche, Sortierung, Tracks oder Crate-Mitgliedschaften neu berechnen.
- [x] ✅ Neue Bibliotheksansichten mit vorbereiteten Unicode-Such- und Sortierwerten aufbauen; bisherige Ergebnisse gegen Referenztests geprüft.
- [x] ✅ Trackauswahl und Pfadauflösung über den ID-Index; Sichtbarkeit bleibt an Suche und aktives Crate gebunden.
- [x] ✅ Headless-Bibliotheksmodell mit 100.000 synthetischen Tracks, Suche, Sortierung und Auswahl geprüft.
- [x] ✅ Bereits sortierte Treffer bei Verfeinerung der Suche weiterverwenden; Erweiterungen und Katalogänderungen bauen die Sicht neu auf.
- [x] ✅ Anzahl fehlender Dateien bei unverändertem Bibliotheksbereich zwischenspeichern und bei Änderungen neu bestimmen.
- [x] ✅ Begrenzter In-Memory-Trigramm-Kandidatenindex mit exakter Nachprüfung und linearem Fallback; Aufbau und Ansicht bleiben synchron.
- [x] ✅ Bibliothekssuche und Sortierung für alle Bibliotheksgrößen im Hintergrund mit revisionsgebundener Ergebnisprüfung; Projektionsaufbau bleibt synchron.
- [x] ✅ Bibliotheksseiten mit höchstens 200 Tracks, global sortierten Treffern, Seitensteuerung und Tastaturwechsel; Seitenantworten stammen vom Hintergrundworker.
- [ ] ☐ Weitere Bibliotheksskalierung: indexierte/paginierte Suche und große Ansichten unter realistischen Bedingungen qualifizieren.
  - Teilfortschritt: Der Trigrammindex des Such-Snapshots wird bei Bedarf im Suchworker aufgebaut statt im UI-Thread kopiert. Der Snapshot der Track-Daten entsteht weiterhin synchron; inkrementelle Synchronisierung, Lastmessung und unabhängiges Review stehen noch aus.
  - Reale USB-Musiksammlung auf Windows geprüft: 3.478 unterstützte Pfade, 60 Sekunden Zwei-Deck-Lauf mit auf null gesetzten GUI-Kanalfadern und Seiten-/Suchaktionen; Erststart mit ungelesenen Tags und 59 Metadatenfehler bleiben offene Befunde; bei fünf getesteten Dateien mit ID3-Textkodierungsdiagnose dekodierte FFmpeg jeweils eine Sekunde Audio erfolgreich; DrDJs `--analyze` schlug fehl. Details und Grenzen siehe LOCAL_VALIDATION.md.
- [x] ✅ Opus4: Handbuchabgleich: LOOP erhält die Position innerhalb der Grenzen, RELOOP springt zu In; OUT setzt weiterhin die Grenze. Transportzustand bleibt erhalten.
- [x] ✅ Einmaliges Tempo-Matching aus zwei bestätigten manuellen Grids in beiden Decks, mit Pitch-Bereichs-/Transientenschutz und Headless-Tests.
- [x] ✅ Einmaliger Beat-Phasen-Sync mit Tempo-Anpassung, bestätigten Grids, Track-/Transientenschutz und PCM-/Headless-Tests.
- [x] ✅ Gemeinsamer interner Deck-Render-Bus und Zähler vollständiger Ausgabeframes; Lade-/Pause-/EOF-/Kanalgrenzen und Warm-Allokationen geprüft.
- [x] ✅ Kohärente Quellphase/Bus-Frame-Paare mit begrenztem Lesen, Befehls-/Wechsel-Validierung und parallelen Tests; einmaliger Sync nutzt passende Paare.
- [x] ✅ Frischeprüfung für gleiche Render-Frames mit Alters-/Lesefenster, beobachtetem Bus-Fortschritt, Stillstandsablauf und Wiederanlauf-Tests.
- [x] ✅ Begrenzter kontinuierlicher SYNC-Follower für bestätigte konstante Grids: getrennte Korrektur, Frische-/Eingabeschutz und Render-Deadline, beide Deck-Richtungen getestet.
- [x] ✅ Beat-/4/4-Takt-Auswahl für PHASE und initiale SYNC-Ausrichtung, mit exakten Halbperioden-Entscheidungen und Modus-/Abbruchtests.
- [x] ✅ Fortlaufende 4/4-Taktphasenregelung mit erfasstem Sitzungsmodus, Ganz-Beat-Fehlererkennung und unveränderten Sicherheitsgrenzen.
- [x] ✅ Opt-in Beat-Quantisierung neuer HotCue-Slots mit Grid-/Draft-/Endschutz, exaktem Recall und SQLite-Neustarttests.
- [ ] ☐ Musikalische/Hardware-Langzeitqualifikation von Sync, hörbare Übergänge, variable Taktarten, Keylock und geplante quantisierte Trigger.
- [ ] ☐ Tempo/Pitch, Nudge, Sync, Keylock und Quantisierung.
- [x] ✅ Gemessener Post-Fader-Bus-Sample-Peak aus vollständigen50-ms-Renderfenstern mit dBFS-Anzeige; kein True-Peak-/Lautheits-/Hardware-Nachweis.
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

## Next library-search architecture slice (2026-10-27)

For every library size, the view worker filters and sorts an immutable in-memory projection with a bounded trigram index. Creating a fresh projection on catalog or scope changes still copies records and the index on the UI thread; search completion is asynchronous and uses generation/revision/query/sort/scope validation. A persisted SQLite index alone cannot represent unsaved or failed-save edits. The next search slice should introduce a separately synchronized background search projection with its own catalog generation, search revision and request identity, preserve current substring/sort/crate semantics, discard stale results and keep worker communication bounded. Search changes must not consume persistence acknowledgements; later index work can build on that projection. Validate delayed results, unsaved edits, worker failure, selection and 100,000-track headless behavior before marking indexed/paged search complete. This is a proposal, not an implemented worker or performance acceptance.

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

## Loop-range waveform overlays checkpoint (2026-10-25)

This bounded slice adds display-only loop shading and In/Out lines in both overview/detail waveforms from actual per-deck engine snapshots. Complete validated bounds are required. Active and inactive state use distinct colors and accessible source-time descriptions; incomplete In-only state has no interval marker. Overview and detail receive separate viewport-local models. Native Duration comparisons decide overlap and endpoint visibility before f32 conversion; viewport times use the same canonical frame conversion as engine boundaries. Clipped shading stays within the waveform, uses a one-pixel minimum even at EOF, and hides exactly touching non-overlaps. Saved slots alone do not create markers until recalled into deck bounds.

Independent GPT-6 Astra / medium review found an absolute-f32-coordinate defect for final-frame loops on a supported 600-second track. The implementation was corrected before publication, with a synthetic long-track headless regression (no huge PCM fixture). Existing workflow tests verify full/left/right/outside ranges, two sizes, zoom/seek, active/exit state, one-frame EOF, failed/successful load and other-deck/volume isolation. No audio engine/callback, schema or dependency changed; waveform exposes only a read-only sample-rate getter for precise view timing. Visual rendering/contrast/overlap usability remain unqualified. Next proposed bounded slice is explicit optional beat alignment when creating Auto Loops; cue-trigger quantization, tempo/sync and hardware timing remain separate requirements. Publication is gated by follow-up review and final green checks.

## Optional Auto Loop beat alignment checkpoint (2026-10-25)

The reviewed loop-range slice was published as fa710ac before this bounded continuation. Each deck now has a runtime BEAT SNAP checkbox in AUTO, default off. With it off, creation retains the existing playhead-frame start. With it on, only a newly triggered Auto Loop starts at the nearest nonnegative whole beat from the committed grid BPM/anchor, then activates under the same Play/Pause/complete-region rules. Equal-distance ties choose the later beat; if the mathematically nearest beat is negative, the first nonnegative grid beat is used. Beat selection and first-nonnegative/tie decisions use checked integer rational arithmetic for the stored f64 BPM, then round directly to a source frame once; no floating floor/ceil or intermediate nanosecond floor is used. A snapped start/end outside the source fails before any mutation, without fallback to unsnapped/clamped audio.

Changing the checkbox alone never moves or resizes playback. The preference is independent per deck and transient; it is not a stored track/grid setting. Pending grid edits, saved cue/loop data, manual In/Out, half/double and cue recall remain unaffected. No trigger scheduling, global quantization, keylock or sync is implemented. Domain, engine and actual headless checkbox/auto actions cover tie/anchor/preroll/rounding behavior, failures retaining active state, other-deck isolation and draft-versus-committed timing. Existing two-size AUTO geometry includes the new checkbox. No schema/dependency/sample callback code changed.

Next proposed reviewed slice: playback speed/pitch control-plane and PCM source foundation with deterministic offline tests; GUI, keylock and sync remain separate acceptance scopes. Native visual/listening/physical latency, musical grid correctness, cue trigger scheduling and platform qualification remain open. Publication requires the standing independent GPT-6 Astra medium review and final green checks.

## Coupled playback speed/pitch engine checkpoint (2026-10-25)

This bounded engine-first slice adds validated requested0.5–2.0 speed factors and rounded effective source sample rates. A prepared immutable PCM clone receives a fixed playback rate on the control plane; DeckOutput reports that exact format for its lifetime, and the existing Rodio resampling path produces coupled pitch/time changes. Sink::set_speed is not used because DeckOutput intentionally fixes metadata per installed source. Source sample iteration remains unchanged and allocation/lock-free in its own next(); Rodio queues/resampler/control callbacks are not claimed lock-free or hard-real-time qualified.

Speed changes replace the source at current original-track position, retaining Play/Pause, active loop/bounds, grid and mixer. Sink playback elapsed time is mapped by effective rate/base rate to original source time before EOF/clipping/loop wrapping. Seek/Cue, loop activation/exit/resize and pause preserve the deck factor. Successful new-track preparation starts at1x; failed preparation preserves state. Invalid factors/rate overflow fail before mutation. Requested/effective values remain distinct because rates are whole Hz. The public engine setter is staged with a narrow dead-code allowance pending GUI wiring; no GUI/persistence/new dependency was added.

Tests measure rendered pitch frequency, signal duration and stereo antisymmetry at0.5/1/1.25/2x, source-time control resolution, effective-rate rounding, loop/speed lifecycle and failures/other-deck/gain isolation. Queue-leading silence and the existing5ms position publication tick are measured/accounted separately from signal duration. This does not establish artifact-free live fader changes, anti-alias/resampling quality, hardware latency or musical acceptance. Next proposed reviewed slice is GUI pitch control and requested/effective speed plus original/current BPM display; native rendering/listening, smoothing/coalescing, keylock and Sync remain separate acceptance gates. Publication requires independent Astra-medium review and final green checks.

Review follow-up coverage also renders44.1kHz at requested1.333333/effective4/3 and directly asserts steady-state loop-region samples over repeated changed-rate wraps, resizing and another rate change. These checks deliberately exclude the control-plane settling transition; no click-free claim is made.

## Coalesced GUI pitch controls checkpoint (2026-10-25)

The engine-first slice passed independent Astra-medium review and final112-test checks before publication376a174. This follow-up adds a fourth PITCH tools panel with a fixed-50..+100% fader and1x reset, requested/effective pitch captions, and manual-grid or explicitly approximate estimated source/current BPM. Current BPM uses the effective rate ratio; absent analysis stays unavailable. No keylock/Sync, tag-BPM claim or selected-range control is implied.

GUI fader/reset events are validated and coalesced into one pending request per deck, retaining only the latest. The existing33ms application timer applies at most one pending setting per deck per tick. Requests capture the installed prepared-source identity and are discarded if loading/restore/import starts or the source is replaced before application. Invalid events clear that deck's pending request and leave the engine unchanged. Play/Pause, position, loop/bounds, grid and mixer are retained by the reviewed engine. GUI bindings display applied engine values after the tick; coalescing does not qualify audible live transitions.

Actual headless slider/reset tests cover bursts, default/unloaded states, final request ownership, manual/current BPM, invalid/loading/stale-source rejection, failure retention and deck isolation. Two-size layout now covers all four tool panels within the existing260px deck. No schema/dependency/sample-source code changed. Next proposed reviewed slice is selectable pitch ranges and fine adjustments. Native visuals/focus/listening, interpolation/anti-alias/transition quality, keylock/Sync, controller hardware and platform gates remain open. Publication requires the standing review/final-check gate.

## Pitch ranges and fine control checkpoint (2026-10-25)

The reviewed GUI baseline is published as c366c79. This bounded slice adds per-deck±8%,±16%,±50% and Wide(-50..+100%) control ranges; Wide remains the default for compatibility. Range selection is a view/control constraint, never an engine-rate command. Changing it cancels that deck's pending pitch gesture. If the applied factor lies outside a new range, the fader is hidden rather than silently clamped; actual requested/effective values remain visible and reset/wider range permit recovery. Range selection persists only for the current deck session and remains on successful track replacement while the speed itself resets1x.

GUI requests quantize to0.01 percentage points before the existing effective-Hz mapping. Fine± actions use integer hundredths of a percent, building from the source-bound pending target if present, so bursts coalesce without accumulated float drift. Range-edge rejection preserves the last valid pending fine target. Reset and all rate changes continue through the reviewed bounded queue. Audio source, engine speed algorithm, storage and dependencies are unchanged.

Domain and actual headless tests cover all bounds/quantization,100-step forward/backward stability, out-of-range current-state display, range-only no-seek/no-rate change, boundaries retaining queued steps, invalid indices, independent decks, new-track reset/session range retention and two-size geometry. Native usability/focus, audible transitions, resampling quality and physical latency remain unqualified. Next proposed reviewed slice is temporary nudge with safe release/cancellation; keylock and Sync remain separate. Publication follows independent Astra-medium review and final green checks.

## Temporary nudge engine checkpoint (2026-10-25)

The reviewed range/fine baseline is published as a1d3b3b. This bounded engine-first slice separates permanent base pitch from the temporary nudge direction(-1/0/+1) and actual rate. Nudge multiplies the current base by0.98/1/1.02; it is not an additive percentage-point edit. A valid permanent pitch change during nudge updates the base; release derives neutral rate from that latest base rather than restoring a captured stale value. Unsupported base/combined-rate/direction requests fail before mutation. Nonneutral activation requires a playing loaded deck; neutral release is idempotent and succeeds unloaded/after EOF without starting playback.

Pause freezes the current original-source phase and cancels nudge; successful explicit seek/Cue cancel it after preparation, while failed operations retain state. Successful new tracks reset base1x/neutral; failed preparation retains the active state. Loop phase/bounds/grid/mixer and other-deck ownership remain intact. Playback snapshots report base requested pitch and actual effective pitch separately, with a nudge-state caption. PlaybackSpeed now stores only validated rate data; the requested permanent factor has a single owner in Deck. No dependency/schema/sample iterator change was needed. The staged engine nudge setter awaits GUI momentary input in the next reviewed slice.

Offline tests cover rate composition, failed limits, paused/unloaded guards, current-base release, transport/new-track/EOF release and steady stereo tone frequency after direction changes. These do not qualify live transitions, native input cancellation, physical latency or musical beat alignment. The following slice must implement robust pointer/key release, focus/panel/loading/shutdown cancellation and headless lifecycle coverage. Publication follows independent Astra-medium review and final green checks.

## Momentary nudge GUI checkpoint (2026-10-25)

The reviewed engine baseline is published as e1371a8. A fifth NUDGE deck-tools panel provides relative hold-to-nudge -2%/+2% and Cancel. Pointer down explicitly focuses a zero-width keyboard scope; pointer up/cancel and held Space/Return releases neutralize. The direct focus-lost callback cancels immediately on focus transfer/window deactivation; repeated key events cannot reactivate canceled input. Instant accessibility actions perform press then release synchronously, never a toggle. Existing source/current BPM and actual nudge captions remain visible. Panel switching cancels before removing controls; disabled controls and controller guards prevent activation while paused/loading.

Controller sessions bind held-direction flags to prepared-source identity. Repeated presses are idempotent, opposing held directions yield neutral, releasing one retains the other if valid. Failed remaining-direction rate validation cancels all rather than preserving already released flags. Neutral cancellation bypasses busy guards. Existing33ms tick discards stale/loading/not-playing/EOF sessions and transport callbacks clear sessions on pause/Cue/successful seek. Shutdown neutralizes both decks before storage flush. Current permanent base, source phase/loops and engine limits remain governed by the reviewed engine. No dependency/schema/sample iterator change was made.

Headless event/state regressions cover actual pointer down/up, Space/Return/repeat, window activation loss, panel removal, busy/paused/source/EOF/shutdown cancellation, conflicting inputs and rate boundaries. Five-panel two-size geometry is checked within existing260px decks. These are headless tests, not native visual/focus, listening/live transition or physical latency acceptance. Next proposed slice is validated manual-grid tempo matching; phase Sync, keylock, smoothing/resampling quality, hardware and full product qualification remain open. Publication requires independent Astra-medium review and final green checks.

Review follow-up: successful occupied hot-cue recall and all successful GUI loop commands/saved-boundary recall now clear held ownership, including neutral opposing holds; rejected actions preserve it. A dedicated regression covers hot-cue/loop Exit/In/saved recall and invalid cue/loop actions. Each NudgeHold uses explicit first-input ownership: the initiating left pointer, Space or Return owns the gesture until its release; additional simultaneous inputs are ignored and require a fresh press. Unrelated key/right-button releases cannot cancel the owner. Mixed-input tests cover this policy. This avoids both premature neutralization and stale aggregate releases after transport cancellation.

## Opus1: independent deck control checkpoint (2026-10-25)

The user authorized implementing the assessed Opus recommendations. Controller can_control_deck(id) now gates only startup restore and that deck's pending load. Pitch/fine/nudge, loops/saved loops, Hot-Cue editing and grid editing use this ownership check. Library import merges do not replace catalog ownership; global crate mutations and admission of load/import jobs retain their existing guards. Shared-track draft conflict and prepared-source checks remain. Both CompactDecks and grid editors receive per-deck busy state; LOAD admission still reflects the single worker. Native focus loss still cancels nudge independently of any busy state.

A symmetric headless regression holds the other deck's real picker worker via channels, applies a previously queued pitch, retains logical nudge, rejects commands to the loading deck, exercises loops and durable Hot-Cues, then merges a controlled import result and verifies SQLite restart. Restore remains exclusive. No audio/sample-source/schema/dependency change was made. Native dialog/focus/listening/latency acceptance remains open. Review and final checks are recorded in LOCAL_VALIDATION.

## Opus6: optional metadata backfill failure checkpoint (2026-10-25)

After reviewed3034f92, Store::open retains successfully restored/enriched catalog data when its optional backfill save fails. Worker LoadedLibrary carries that warning together with the library and startup note in one event, so no fake save completion can race command accounting. Controller publishes the catalog, retains the worker, marks dirty/save_failed and exposes the existing Retry Save path. Fatal inspect/schema/migration failures retain their existing preservation/error behavior. No schema/dependency/audio/UI-source change.

A headless regression seeds a real unscanned WAV record, injects a SQLite metadata INSERT abort trigger, verifies the visible enriched title and stable ID with DB scanned still0, loads the track, removes the trigger, retries, checks scanned1 and restarts successfully. This proves this optional write-failure path; it is not a new physical power-loss/platform qualification. Final review and checks are recorded in LOCAL_VALIDATION.

## Opus3: control-plane PCM retirement checkpoint (2026-10-25)

After reviewed f4a505d, each custom DeckOutput owns a retirement marker declared after its queue. Rust field-drop order destroys the queued PCM owner before a Release store publishes retirement. Source replacement transfers the old PreparedTrack into a control-plane retained list before dropping the old Sink. GUI ticks and subsequent replacements Acquire-check acknowledgements and reclaim old track buffers only on that control plane; sink.empty or elapsed time is not used as proof. Engine Drop signals sinks and drops its owned stream before track-buffer fields; the local Windows CPAL implementation joins its render thread on stream drop. Injected renderers on another thread must be stopped/joined by their owner before engine drop.

A real offline mixer on a separate thread verifies the old PCM remains alive after queue/mixer destruction with exactly the control owner left, then its last strong owner is released only after collect_retired on the test/control thread. The Weak observer retains the Arc allocation, so this is not an allocator-level free measurement. This establishes old PCM ownership reclamation in the tested lifecycle, not hard-real-time qualification: Rodio queue/resampler locks and small wrapper/marker allocations/frees remain. A stalled renderer retains buffers until acknowledged rather than prematurely freeing them. Native shutdown, device latency, listening and cross-platform qualification remain open. Delayed playhead/source transitions are a separate Opus2 slice.

## Opus2: stable output-format deck renderer checkpoint (2026-10-25)

After reviewed0812e8f, deck_render.rs renders PCM directly into the explicit mixer/device channel count and sample rate. Whole-Hz effective source rates drive integer source-phase/output-rate accumulation with linear interpolation; channel mapping matches the previously used Rodio channel conversion (discard extra inputs, mono-to-stereo duplicate, zero-fill other added outputs). This is coupled rate/pitch, not time stretching or anti-alias qualification. The mixer sees an unchanging matching format, so its external converter has no rate lookahead to confuse the published playhead.

A single control-plane writer publishes atomic scalar settings with a sequentially-consistent version/payload/version order. The renderer checks at the beginning of complete output frames; a busy/changed writer yields one bounded failed read and the prior coherent settings for that frame, never a spin. Phase and source nanoseconds publish at complete output-frame boundaries. Explicit pending seeks display their target until audio acknowledges the seek serial; rate-only updates leave phase untouched. Pitch/nudge no longer clone/reseek a source or replace Sink. Same-track seek/Cue/loop/pause update the existing renderer; actual track replacement stops the old graph at a frame boundary and retains the reviewed retirement ownership. EOF remains silent/installed and explicit restart/seek reuses the graph, avoiding queue-exhaustion races. Injected mixers supply their actual format explicitly; the production composition root reads OutputStream config.

All existing124 tests passed after initial integration. Three added regressions check fifty sub5ms rate/nudge flanks with exact rational ramp positions at source44.1/48/96kHz and output44.1/48kHz, stereo antisymmetry and unchanged renderer/retirement count; mid-stereo-frame commands, pause, bounded busy-writer behavior and zero counted next() allocations; and concurrent command payload coherence. No audible transition/artifact-free, anti-alias, device-latency compensation, CPU deadline or cross-platform claim. Rodio still owns mixer/Sink locks/wrappers and coarse gain/pause control propagation; this renderer has no callback locks or allocation. Full application real-time acceptance remains open. Review/final commands in LOCAL_VALIDATION.

Opus2 review correction (2026-10-26): initial127-test review found double nanosecond/frame-flooring at non-integral frame boundaries. Commands now carry explicit frame coordinates and the renderer/control publication retains exact integer source-phase units, not a rounded nanosecond clock. Canonical display timestamps use ceil only at the Duration boundary. Loop resize/exit/pause preserve fractional phase; a wrap-changing loop command composes a pending exact rational phase so back-to-back half/double without an intervening render cannot undo its mapping. New regressions directly assert frame1/2/7/8 seeks at44.1/48/96kHz, actual48kHz loop-resize samples at frame2, manual In/Out boundaries, composed pending half/double and44.1→48kHz fractional phase through resize/exit/pause. The new two tests passed(.00s). This is a coordinate/phase fix, not merely a changed ramp expectation. Final full gates and follow-up review apply to this corrected state.

Opus2 EOF follow-up (2026-10-26): independent129-test review accepted the frame-coordinate correction but found CLI wait_for_end still waiting on finite Sink queues. It now polls renderer source completion on the control thread instead. An offline two-deck regression keeps the output queues alive at EOF until that wait returns, checks the longer deck is not cut off, and acknowledges completion via a bounded channel timeout. This does not prove physical device-buffer drain; producer EOF and hardware output completion remain distinct. No live hearing/device test was performed.

## Opus5: revision-bound track delta persistence checkpoint (2026-10-26)

After reviewed e91319e, Library marks changed track indexes and crate state with monotonic revisions. Regular saves/close clone only changed SavedTracks plus a crate snapshot only when crates changed. Controller records the in-flight revision and acknowledges only that revision after success; newer edits and failures remain pending. Optional metadata backfill acknowledgements clear already persisted enrichment. Cue/grid/saved-loop-only edits to existing records no longer rebuild unchanged metadata table rows in the GUI. Track vector indexes remain stable; global deletion/relocation is not introduced.

Worker applies deltas in one Immediate SQLite transaction with lightweight application/schema checks, stable identity/path checks, bounded per-track cue/loop/grid writes and conditional metadata upserts. Full writes retain migration/recovery/test responsibilities. Integrity-checked online backups occur before the first delta, after an empty backup can be superseded, on dirty60s checkpoint opportunities (including idle timeout), and on clean close. Normal cue saves do not inspect/backup/rewrite the whole catalog. Failed checkpoints use bounded retry delay and a distinct warning event, leaving save accounting intact; an empty delta can explicitly retry backup. Backups can lag recent commits until checkpoint/clean close, and I/O failures can extend that lag. SQLite synchronous=EXTRA transactions remain; this is not a new physical power-loss guarantee. Crate mutation still carries/replaces its relationship snapshot; finer crate deltas and large view virtualization remain separate optimization work.

Measured debug-test snapshot for100000 records: a single-cue delta used3 allocations/~20.1us versus100002/~162.25ms for full catalog cloning. SQL tests at1000/10000 tracks changed2 rows for one cue (~3.98/~4.50ms in the focused run), preserved unrelated rows, rolled back an injected trigger failure and retried latest data. Additional tests cover revision acknowledgement/newer edits, failed delta retry, due-backup fault/retry and an idle timed checkpoint. These are local sample measurements, not production latency bounds. Review/final evidence in LOCAL_VALIDATION.

## Opus7: PCM ownership and cancellable jobs checkpoint (2026-10-26)

After reviewed bb06a94, PcmSource privately shares an Arc<Vec<f32>> that takes the original Vec allocation without copying samples into an Arc slice. This also means test Weak observers retain the small Arc owner block rather than the Vec sample allocation after its last strong owner drops; prior Opus3 evidence remains a historical last-owner proof. Decode uses fallible try_reserve_exact with an8MiB speculative hint cap, bounded growth and the existing256MiB retained PCM sample limit. A controlled1M-sample decode/ownership test recorded2 allocator requests,0 reallocations and4000040 live/peak requested bytes; a pointer identity test independently verifies transfer. This is isolated allocator bookkeeping, not process RSS/decoder/analysis/device peak qualification. Large unknown-length reallocations, analysis buffers and multiple loaded decks still need whole-process budgets.

A small Cancellation token is checked before I/O, every4096 decoded samples and between analysis stages; imports check between metadata files and before returning. Controller retains one global job slot until worker acknowledgement, checks cancellation again before applying even already queued results, and cancels on close/drop. The footer CANCEL action targets load/import only, not a database commit/restore. Native pickers and synchronous OS I/O cannot be forcibly interrupted; cancellation is cooperative and stage boundaries are not a hardware deadline. A completed cancelled load/import preserves old deck/library data, and the next job receives a fresh token.

Headless tests block real picker callbacks through channels, cancel twice, retain admission until acknowledgement, verify successful subsequent loading, cancel import, reject a ready late result, cancel on close and check two-size button geometry. Synthetic decoder cancellation occurs mid-iterator rather than only at EOF; pre-cancelled paths/imports do not inspect files. No schema/dependency change or native visual/dialog/listening acceptance. Full gates/review in LOCAL_VALIDATION.


## Opus4: reference-backed LOOP and RELOOP checkpoint (2026-10-26)

The unchanged DRDJ-UserManual-DRDJ.pdf, printed pages50–51, distinguishes Loop On/Off from Slot Reloop. LOOP now toggles the current interval, retaining exact renderer phase when inside it; outside it activation starts at In. RELOOP explicitly activates/restarts at In, including while already active. Both preserve Play/Pause, including EOF remaining stopped. OUT remains boundary capture; the reference does not explicitly require automatic activation on OUT. The deck row exposes IN/OUT/LOOP/RELOOP, with the existing programmatic exit callback retained.

Offline ramp samples verify the distinct start positions; headless controls verify both decks, paused/playing state, readiness guards and geometry. This resolves the assessed Opus4 ambiguity, not full loop parity: active In/Out fine editing, auto-loop same-length toggle/current-loop resize semantics, direct slot controls, media-file loop persistence, quantization and native/hardware acceptance remain separate work. No schema/dependency change. Publication requires independent Astra-medium review and final checks; results are recorded in LOCAL_VALIDATION.md.


## One-shot manual-grid tempo matching checkpoint (2026-10-26)

MATCH in each deck's PITCH panel sets that deck's coupled playback speed to (other confirmed grid BPM * other effective rounded playback factor) / own confirmed grid BPM. It is one-shot: later reference pitch changes do not follow automatically. Estimated/tag BPM, beat-phase alignment, automatic master selection, octave/double-tempo selection and keylock are outside this slice. Exact requested factor is passed to the existing whole-Hz renderer mapping without GUI hundredth-percent quantization; current requested/effective captions expose rounding.

Both tracks need saved manual grids, both must be available for control, and target factor must fit engine0.5–2x and selected GUI pitch range. Active nudge/held nudge ownership, pending pitch gestures and grid edits reject without consuming or changing them. MATCH applies on one control-thread turn; imports need not block it, while restore or loading either deck does. Position/fractional phase, loop boundaries/activation, Play/Pause, manual grids, mixer and other deck remain intact. No new dependency/schema/storage/audio-callback work. Publication requires Astra-medium independent review and final checks recorded in LOCAL_VALIDATION.md. Phase Sync, keylock, quantization and physical/musical qualification remain open.


## One-shot beat-phase sync checkpoint (2026-10-26)

PHASE in the NUDGE panel performs one immediate tempo-and-nearest-beat-phase alignment from two confirmed manual grids. It is distinct from MATCH (rate only). Both decks must be playing, with active loops off on both. Existing range/loading/restore/pending-pitch/grid-editor/held-nudge guards apply before mutation. No estimated BPM, auto-master, latched regulation, downbeat/bar alignment, keylock or quantization mode is introduced.

Both grid phases use exact binary-BPM remainders scaled to a shared integer denominator. The shortest signed beat displacement is chosen exactly before final floating-point nearest-frame projection onto the target grid. Nearest half-beat ties choose later; if that nearest rounded source frame is outside the decoded track it rejects rather than making a larger jump or clamping to EOF. Positions beyond seven days reject. The control plane publishes rate and source-frame seek coherently to the installed renderer. Inactive loop boundaries, manual grids, gains, reference deck and storage remain unchanged. Positions are separately sampled published render coordinates, not a simultaneous physical-output clock: scheduling delay, device buffers, fractional-frame rounding and unequal device paths remain qualification limits. No click-free/musical/physical synchronization claim.

Focused domain, actual-ramp engine and both-deck accessible-action tests pass; final gates and Astra-medium review are recorded in LOCAL_VALIDATION.md. Continuous shared-clock sync and native/hardware/listening qualification remain open. No dependency/schema/audio-iterator changes.


## Shared deck render bus foundation (2026-10-26)

Both deck queues now feed a private fixed-format Rodio mixer, wrapped once by RenderBus before entering the device/injected mixer. Its shared AtomicU64 counts completed interleaved output frames, including silence while unloaded/paused/EOF. The bus remains installed and continues polling its mixer so late source admission and replacement work after an empty result. One engine clock handle is retained for the next sync-control integration; continuous sync does not consume it yet. Existing MATCH/PHASE semantics remain unchanged and the continuous-sync checkbox stays open.

This is a render-frame clock, not hardware/device/drain time. It establishes a common point at which both deck sample pulls finish; timestamped coherent phase publication, matching controls to that clock and bounded continuous correction still require subsequent reviewed slices. No new GUI, dependency/schema or DeckRenderer iterator change. Rodio inner/outer mixer source admission still uses its own locks/allocations; only the wrapper and warmed steady pull are tested for no added allocations. Hardware deadlines/listening/latency and platform acceptance remain open.


## Coherent source-phase / bus-frame publication (2026-10-26)

Each clock-attached deck renderer publishes exact source-phase units and the corresponding completed-frame boundary together, only after a complete interleaved frame. A separate single-writer SC sequence protects the atomic payload. Readers make one bounded attempt and reject overlapping publication, stopped sources and a changed/unapplied command sequence. The stamp records a produced-frame boundary, not hardware-played/drained time. Paused Sink sources can retain an old, truthful stamp while the shared bus advances; no freshness is inferred from mere presence.

One-shot PHASE uses two valid stamps only when their frame identifiers match and the bus has completed that boundary. Otherwise it retains its existing control-position fallback, including explicit pending seeks. That fallback is not suitable for a future continuous regulator: timestamp freshness, shared-clock correction policy and lifecycle cancellation still need further slices. Existing UI/MATCH/transport behavior remains unchanged. No schema/dependency change. Tests cover stereo half frames, command invalidation, pause/EOF/restart/replacement, concurrent readers and isolated allocation-free publication. Review/final evidence: LOCAL_VALIDATION.md.


## Fresh render-pair eligibility foundation (2026-10-26)

Same-frame phase pairs now pass a strict control-plane freshness helper before being used as rendered one-shot inputs. At the final sampled bus counter, the stamp must be at most ceil(output_rate*5ms) frames old, and the bus must not have advanced by more than that budget during pair sampling. Future frames, counter discontinuities, invalid stamps and mismatched frames reject. A separate control-plane Instant observer acquires liveness only after progress between timely observations; it expires after100ms without observed progress, rejects observation gaps over100ms and reacquires after new progress. The pair read also revalidates at its end and must take no more than5ms wall time. Boundary limits are inclusive. No Instant calls, new allocations or locks are added to audio callbacks.

This is observed producer liveness, not a device clock/deadline guarantee or proof of current hardware output. A qualifying pair is observed at validation; it cannot promise that rendering continues afterward. Paused/EOF/deck/grid/loop/nudge ownership eligibility still belongs to surrounding engine/controller guards. One-shot PHASE keeps its previous control-position fallback when fresh rendered inputs are absent. Future continuous regulation must require the strict helper and must not silently use that fallback. Continuous correction/lifecycle ownership remains open. Review and deterministic clock/boundary/lifecycle evidence are recorded in LOCAL_VALIDATION.md.


## Bounded continuous Sync follower (2026-10-26)

SYNC in the NUDGE panel enables one follower at a time; the other deck is its fixed reference. Acquisition requires both playing with confirmed manual grids, loops off, admitted pitch range and a strict fresh observation. It performs the existing one-shot tempo/phase alignment once, then only rate corrections: no recurrent seeking. A second follower is rejected to prevent cycles; the active button toggles off even if readiness later fails.

Sync correction is separate from requested base pitch and manual nudge. The controller targets reference effective BPM plus proportional beat-phase feedback, caps the multiplier0.98–1.02 and slews at most0.02 multiplier units/second (two percentage points per second). The existing global0.5–2x engine limits still apply; an unsupported combined rate stops following and restores neutral correction. Acquisition respects the selected base pitch range; temporary correction, like nudge, can extend beyond that fader range. UI shows base requested/effective rate plus active Sync/correction.

Each control tick checks both prepared-source identities, saved grids, renderer command generations, playing/loop/loading/restore state and pending pitch/grid/held-nudge ownership. Render-affecting manual input/master pitch/seek changes invalidate the session; source replacement/pause/rate paths neutralize transient state. Missing fresh observations immediately neutralize correction while waiting up to100ms for valid samples, then stop. Long control/render intervals also stop. Ordinary fixed-frame correction publishes rate, neutral rate and a100ms-produced-frame deadline coherently. If the control loop stops renewing, the renderer returns to neutral at that deadline without a wall-clock call/allocation/lock. A stopped renderer consumes no new frames; the watchdog is not a100ms wall/device guarantee. Snapshot captions reflect renderer-reported expiry. Close/drop and explicit Sync off neutralize.

This is a bounded initial continuous-following feature for constant manually confirmed grids, not completed Sync parity or physical/musical acceptance. Synthetic convergence, actual offline PCM/deadline behavior, both-deck multi-tick controls with an injected control-only clock, and cancellation tests are recorded in LOCAL_VALIDATION.md. Native focus/listening, audible transition quality, real devices/deadlines, music corpus, downbeat/bar Sync, controller mappings and broader feature parity remain open. No dependency/schema change.


## Beat / 4-beat bar acquisition choice (2026-10-26)

Each deck now has Beat(default) and Bar4/4 acquisition modes in NUDGE. PHASE and initial SYNC acquisition use the selected period; ongoing bounded correction remains beat-phase based. Bar4 aligns the nearest matching position within the four-beat period defined by each confirmed grid's downbeat anchor. It is immediate alignment, not scheduling a future action on the next downbeat. It can move up to half a bar; if the nearest rounded frame lies outside the decoded track it rejects without clamping or choosing a farther bar.

Both periods use exact binary-BPM phase remainders/common-denominator arithmetic for the discrete decision; exact half-period ties choose later. Different manual anchors, pre-anchor beat indices and fractional BPM remain supported within the existing seven-day bound. Defaults retain previous Beat behavior. The per-deck choice lasts for the current app session and survives track replacement, but is not stored in SQLite/media tags. A changed target choice stops its active follower and neutralizes correction without seeking; changing only the reference deck's view choice does not change the follower. Invalid selections preserve the current state and restore the authoritative displayed index. Valid selection alone never seeks or starts playback.

Tests cover half-bar±1ns, distinct whole-beat ambiguity, manual anchors/fractional tempos, endpoint rejection, actual PCM after Bar4 seek, both deck controls, invalid selection and active-follower release. The existing freshness, loop, loading, grid and nudge guards apply. Full bar-phase regulation, variable meters, quantization, artifact-free transitions, music/hardware/native qualification and full reference parity remain open. No dependency/schema change. Final review/checks are recorded in LOCAL_VALIDATION.md.


## Continuous Bar4 feedback (2026-10-26)

The session's selected Beat/Bar4 period now governs ongoing feedback as well as acquisition. A SyncSession captures its target's alignment at activation; engine observation computes signed phase displacement in source beats for that period. Beat uses a half-period0.5-beat error bound, Bar4 uses2 beats. The existing proportional gain,0.98–1.02 multiplier cap,0.02 multiplier-units/second slew,100ms control interval, strict fresh-pair checks and producer deadline remain unchanged. No recurrent seek or schema/dependency/UI-layout change. A whole-beat offset is now visible to Bar4 rather than being discarded by beat-period wrapping.

Runtime implementation was generated by the configured QWEN API model (reported qwen-3.6-35b-sovereign), inspected/applied by Codex and independently reviewed with GPT-6.1 Sol / medium. Codex also added independent engine/controller verification. This does not qualify physical output phase, musical quality, audible transitions, variable meter or device longevity. Evidence is in LOCAL_VALIDATION.md.


## Quantized new HotCue placement (2026-10-26)

Each deck's CUE panel now has opt-in QUANT for setting empty pads. It uses the confirmed manual grid's existing exact nearest-beat calculation (later beat on ties) and stores the canonical source-frame timestamp. No grid/draft/endpoint fallback is guessed: missing grid, open grid editor, nearest frame at/after decoded EOF or existing busy/restore guard rejects before catalog insertion or mutation. Placement preserves source position, Play/Pause, pitch/nudge/loop state and gains. Existing occupied recall remains exact to the saved cue; deletion remains available without a grid.

The toggle defaults off, is per-deck session state, survives track replacement and is not stored in SQLite or media tags. Enabling from the UI requires a loaded/control-ready track with a confirmed grid; an already enabled toggle can always be turned off, including after a gridless replacement. Cue data uses the existing SQLite transaction/revision/flush protocol. This is nearest-beat placement of new cues, not scheduled/quantized triggering, hold-Cue semantics, bar snapping or full quantization parity. Tests cover both decks, actual accessible actions, later ties, no-mutation rejection, canonical-path SQLite restart and44.1kHz fractional-BPM recall. QWEN supplied runtime fragments; Codex adapted existing identifiers/binding placement, added verification and ran checks. Independent review is GPT-6.1 Sol / medium. LOCAL_VALIDATION.md records checks and remaining native/hardware/listening gates. No dependency/schema/audio-renderer change.


## Precise original-source time display (2026-10-27)

The next bounded UI/reference item replaces elapsed/total whole seconds with a two-line, explicitly labeled ELAPSED/LEFT/TOTAL millisecond caption for both decks. Integer Duration arithmetic clamps elapsed to duration and saturates remaining; pitch does not change source-time coordinates. Sub-millisecond fractions truncate, minutes can exceed59, and the complete caption is available as an accessible description when elided. Tests include arithmetic boundaries and both real controller deck bindings, transport/seek/pitch/EOF and two-size headless geometry. No DSP, scheduled triggering, persistence or native acceptance is added. Final command/review/publication evidence is recorded in LOCAL_VALIDATION.md.


## Focus-scoped detail keyboard zoom (2026-10-27)

Bounded UI-reference follow-up: per-deck detail waveforms route focused + and - key presses/repeats to the existing zoom path, with2–32s bounds and no global shortcut. Click-to-focus retains existing click-to-seek. Ctrl/Alt/Meta combinations pass through, Shift is allowed, text fields retain input, and overview keyboard zoom remains disabled. Two headless tests exercise actual pointer/keyboard WindowEvents, both deck focus paths/two supported sizes, modifier/repeat/boundary behavior and preserved audio state after zoom. Native focus/readability/screen-reader acceptance, scrubbing and pitch-scaled waveform reference parity remain open. See LOCAL_VALIDATION.md for final review/check/publication evidence.


## Post-fader bus sample-peak meter (2026-10-27)

Bounded mixer foundation: a focused PeakAccumulator computes aggregate absolute sample maxima across all output channels in ceil(rate/20)-frame windows. RenderBus integrates it without changing returned samples or existing frame-clock counting. One packed AtomicU64 store per completed window publishes value plus validity sequence; control reads once and formats dBFS. Non-finite samples invalidate a whole window. GUI adds BUS PEAK beside the existing faders with full-scale warning and accessible scope description. Producer callbacks gain no loops, locks, allocation, syscalls or logarithms from the meter; existing Rodio internals retain their own limitations.

Acceptance scope is offline sample/atomic/window/gain regression and headless GUI bounds. The latest window can remain after rendering stops or overwrite a peak before a delayed GUI poll. This is not true peak, loudness, physical output timing, a limiter, clip latch or native visual/listening acceptance. Individual channel meters, persistent track gain, EQ/filter/effects and routing/hardware remain open. Actual checks/review/publication evidence is in LOCAL_VALIDATION.md.


## Incremental crate membership changes (2026-10-27)

First bounded part of the earliest open scaling item: membership additions/removals use revision-tracked pair deltas and transactional per-pair SQLite writes, without cloning or rewriting unchanged crate relationships during membership-only saves. At this checkpoint structural crate create/rename/delete still used the full snapshot; the subsequent checkpoint removes it. Library-view virtualization has not yet been started. In-flight acknowledgement, batched additions, atomic failure/retry, track-plus-membership saves and restart are covered by regression tests. Periodic validated backups remain full-catalog work. Final check/review/publication evidence is in LOCAL_VALIDATION.md. The remaining parts of this plan item retain priority before later open items.


## Incremental crate structure (2026-10-27)

The next part of the same first open scaling item replaces structural crate snapshots with revision-bound per-ID create/rename/delete changes and a monotonic high-water mark. The worker applies structural and membership changes atomically, including multi-rename name exchanges. Deleting a crate cascades its own members; other members are not rewritten. Tests cover injected rollback, unchanged memberships, in-flight create/delete and restart. Periodic backups and deletion of a crate with many members remain proportional to their data. Library-view virtualization is the next open part of this plan item. See LOCAL_VALIDATION.md for review and final checks.


## Lazy library row materialization (2026-10-27)

The next part of the first open scaling item keeps the existing Slint ListView and replaces eager construction of every LibraryRow with an immutable ordered ID model. A validated ID-to-index lookup retrieves and formats only requested rows. Weak references avoid a UI/controller ownership cycle, and a catalog generation prevents retained old models from returning rows from a replacement catalog. Filtering, sorting, selection and counts retain their existing behavior. Search/sort still traverse and sort the in-memory catalog and the ordered ID vector is rebuilt on publish; this is not SQL paging or constant-time filtering.

A 20,000-track headless fixture requested two model rows for its visible viewport; sorting, filtering, borrow contention and stale models after restore were checked. In the same local test, preparing IDs from an already sorted visible list used one allocation versus200,001 for eagerly materializing all row objects. The comparison excludes filtering, sorting, index construction and rendering; it is not a whole-refresh or maximum-scale latency measurement. Native scroll/render performance and reduced catalogue-wide filter/sort work remain open, along with the next plan acceptance gates. Review and full-check evidence are recorded in LOCAL_VALIDATION.md.


## Reuse unchanged library views (2026-10-27)

The next bounded step caches the current ordered filtered track indices. Repeated reads with the same normalized query, sort and crate scope reuse that result, avoiding rebuilding sort keys and sorting. Track changes, actual membership edits, active-crate changes, changed normalized queries and sort actions invalidate the cache; the next read rebuilds using the existing filter/sort semantics. Structural edits that do not change scope/members do not affect track order. Cached reads still traverse the result and allocate a reference vector; new queries and changed views still filter/sort the full in-memory catalog, and track mutations conservatively invalidate even when they change only cues/grids. No SQL paging or native responsiveness claim is made.

In a local 10,000-track unit fixture the first view build used10,016 allocations and a repeated cached view used1. Tests cover normalization, sorting, metadata, active crate/membership, deletion and new imports. This is a comparison of view construction only, not an end-to-end GUI, native scroll or maximum-scale latency measurement. Remaining cold-search/sort optimization and real-environment qualification stay at this point in the plan. Review and final checks are in LOCAL_VALIDATION.md.


## Prepared text for cold library views (2026-10-27)

For the next bounded part of library scaling each track retains five normalized text fields for title, artist, album, filename and path. They are created on insertion and rebuilt on metadata updates/restoration. Cold search reuses these values, and cold sorting compares borrowed precomputed keys instead of allocating one normalized key per track. Filtering and sorting still scan/sort the in-memory catalog; ordered result/reference/ID vectors remain. The additional Strings consume memory per track and must be recomputed on metadata changes; no total-memory or wall-time improvement has been established.

A local 10,000-track fixture measured16 allocations for cold view construction and1 for a cached view, compared with10016 and1 before this step in the same fixture. Reference-equivalence tests use the earlier expressions for all six sort fields, both directions, Unicode/missing values, crate scope, restore and metadata replacement. This is unit-test evidence, not SQL paging, native scroll or maximum-scale responsiveness. Indexed/paged search and real-environment qualification remain open at this point in the plan. See LOCAL_VALIDATION.md for actual review/full checks.


## Indexed library selection (2026-10-27)

A further bounded part of the first open scaling item resolves a selected track through the existing ID index, checks the same normalized query and active-crate predicate as the visible view, and resolves its path by ID. Validating an existing selection after a query change no longer constructs or sorts the entire visible view. Navigation by previous/next still uses the ordered visible list. Crate lookup and matching the selected track's strings still require work; this is not a worst-case constant-time, paginated or indexed full-text search claim.

A local 100,000-track fixture selected an ID with zero measured allocations before any view was built, then compared selection against the actual visible set across missing IDs, queries and crate scopes. No time or native UI guarantee follows. Indexed/paged new search and large-view qualification remain open. Final check/review evidence is in LOCAL_VALIDATION.md.


## 100,000-track headless qualification (2026-10-27)

At the existing catalog limit a headless test inserts100,000 synthetic track paths, publishes the real library model, checks the visible viewport, searches for one and many tracks, selects the result and reverses sort order while retaining selection. The corrected viewport counter observed2 model-row requests before any explicit first/last row reads. A standalone diagnostic run measured insertion796.55ms, initial publication52.52ms, selective search72.81ms and broad search102.67ms. Search timings include accessibility-control lookup and callbacks; these are test-backend observations in one run, not native render/scroll timings, actual file/metadata I/O, peak-memory bounds or guarantees. The test uses no timing threshold.

The next open scaling work remains indexed/paged search and real-environment qualification. SQLite FTS5 trigram supports substring matching but full-text queries shorter than three Unicode characters do not match, and wildcard/escaping behavior needs explicit handling and correctness tests; see [SQLite FTS5 documentation](https://www.sqlite.org/fts5.html). No FTS integration or semantic equivalence is claimed by this qualification slice.
