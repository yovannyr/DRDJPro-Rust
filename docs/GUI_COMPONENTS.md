# GUI component inventory and reference baseline


## User-provided UI references

Use `Softwareübersicht.md` and its numbered `Softwareübersicht.png` as the main screen inventory/layout reference, and `DRDJ-UserManual-DRDJ.pdf` for interaction semantics. The screenshot currently selects **Extended** mode. These are user-provided design/behavior references, not evidence that a feature exists in Rust or that the Slint spike has passed visual acceptance.

The PDF has 110 image-only pages. Focused visual reading for this checkpoint covered the contents, page 21, pages 25–30, pages 39–41 and pages 53–58. Page numbers below are the printed/manual page numbers, matching PDF page numbers. This is a focused UI review, not a complete audit of all 110 pages. Input PDF/Markdown/PNG sources were left unchanged.

The current tall deck cards and central mixer are a functional prototype. Next UI implementation should first align the **Extended workspace structure** with these references: global toolbar; compact deck/track areas and per-track overviews; a shared full-width area containing the two detailed waveforms; a substantial library area with navigation/search; and a separate status area. Practice-mode mixer controls have their own reference on manual page 29. Keep supported controls functional during layout changes.

### Numbered overview coverage and gaps

Every overview number 1–59 appears exactly once in this grouped inventory. “Open” means a requirement/reference, not an implemented control.

| Group | Overview numbers | Source detail | Rust status / implication |
| --- | --- | --- | --- |
| Workspace/display/deck selection | 4, 5, 29 | Manual pp. 21, 26, 29 | Open: Vertical, Horizontal, Extended, Stack and Library; active deck selection. Implement one usable reference layout before adding other modes. |
| Global tools/panels | 2, 3, 6, 7, 8, 9, 10, 20, 21, 22, 23 | Overview numbered sections; manual p. 21 for Quantize | Open: Link, Quantize, Flip, recording, FX, sampler, video, MIDI, account/help/setup. Product-specific identity and capability gates need separate scopes. |
| Master/system status | 19, 26, 27, 28 | Overview numbered sections | Open: master output, actual dropout warning, clock and CPU telemetry; never animate invented measurements. |
| Playback mode | 1 | Overview section 1 | Only internal software playback exists. REL/ABS/THRU/changeover depend on actual DVS/input routing/hardware and must not appear as working modes yet. |
| Track metadata/BPM/key | 11, 24, 25, 59 | Manual pp. 25–26 | Filename and experimental BPM candidate exist. Artist/key tags, distinct original/current BPM and pitch-derived display remain open; absent metadata stays absent. |
| Virtual deck/platter | 12, 16 | Manual p. 25 | Open: platter position, outer track-progress ring, end warning; these are distinct values/indicators. |
| Transport/eject | 17, 55 | Manual pp. 40–41 | Play/Pause and simplified Cue exist. Eject, previous/next library track and reference shortcut behavior remain open. |
| Tempo/pitch/nudge | 30, 32, 57 | Manual pp. 25, 29, 41 | Coupled pitch/ranges/fine and momentary nudge implemented with offline/headless evidence; native usability, transition quality and physical latency remain open. Editing grid BPM is not playback speed control. |
| Slip/keylock/sync/censor | 13, 15, 18, 44 | Overview numbered sections | Open; require engine behavior and explicit state before active UI controls. |
| Track gain/output meters | 14, 43 | Overview sections 14, 43; practice controls manual p. 29 | Channel gain sliders exist; persistent per-track gain and measured output meters do not. |
| Time/overview playhead | 41, 42 | Manual pp. 25–27 | Overview/seek exist. Current display is elapsed/total; reference asks elapsed/remaining and metadata time. Add clear labels and fractional-time precision in a separate slice. |
| Detailed waveform/beat matching | 34, 35 | Manual p. 28 | Colored following envelope/zoom/click-seek exist. Shared placement, hold/drag scrubbing, keyboard +/- zoom, beat-matching transient strip and pitch scaling remain open. |
| Grid editing | 46 | Manual pp. 53–58 | Partial: dedicated constant-BPM Edit Grid drafts, Save/Cancel, decimal BPM, numbered bars/red first downbeat, anchor and ±10 ms slip tools, conflict protection and persistence exist. Variable tempo/warp markers, meter changes and native visual/musical acceptance remain open. |
| Repeat/autoplay | 47, 50 | Manual pp. 26, 28 | Open: repeat the same track versus advance to the next crate track. These are different policies, neither implied by EOF/restart-on-Play. |
| Cue-point panel | 48, 58 | Overview sections 48, 58 | Partial data foundation: eight fixed per-track positions/names/colors persist in SQLite schema 5. Eight pads per deck now set empty slots and recall occupied slots without changing Play/Pause; explicit slot deletion exists. Name/RGB Save/Cancel editing and named/numbered overview/detail markers now exist. Quantization and controller feedback remain open. Existing Cue-to-zero is not hot-cue parity. |
| Flip/loop panels | 31, 33, 56 | Overview numbered sections | Open: flip sequences, auto-loop UI and saved loops; require separate engine and persistence slices. |
| Library actions/search | 36, 37, 38, 39, 40 | Manual p. 30 | Open: Search, History, Prepare, Browse, Files. Native file picker is not a library browser. |
| Library table/view | 45, 49 | Manual pp. 39–40; overview screenshot | Open: large track table, sort headers, selection and list/art view. Current library footer is a status placeholder. |
| Crate navigation | 51, 52, 53, 54 | Overview numbered sections | Partial: flat persistent crate navigation/create/rename/delete, set membership and active-crate import are implemented. Subcrates/smart crates and iTunes/Serato-format compatibility remain open. |

### Concrete behavior to preserve from the manual

- **Display modes (p. 21):** Library mode toggles back to the previous performance view with Space. Keyboard handling must respect focused text/search fields. Opening constrained panels is reflected in status. This is a future acceptance contract, not current behavior.
- **Track/deck information (pp. 25–26):** distinguish original/current BPM, pitch percentage/range, elapsed/remaining time, platter position and track progress. Missing artist/key/BPM tags must not be fabricated. Reference track-end warning begins 20 seconds before EOF and is configurable.
- **Overview (p. 27):** complete-track frequency colors, seek on click and minute/five-minute divisions. Current colors and seek cover only part of this contract.
- **Detail waveform (p. 28):** close-up around current position, press/hold scrubbing, +/- zoom and waveform scaling with playback pitch. Existing click seeking is not the documented drag interaction.
- **Library (pp. 30, 39–40):** search as typed; Ctrl+F focuses search; Escape/X clears it; sort through column headers; load a selection into A/B by drag/drop or Shift+Left/Right. Library empty/loading/error states and load protection need explicit Rust contracts.
- **Grid editor (pp. 53–58):** first downbeat is red/bar 1; other bars are numbered; beat divisions and warp markers are separate. Edits have Save/Cancel, Escape cancels and Enter saves. Adjust changes spacing; Slip shifts positions without changing BPM. Existing immediate GRID HERE/OFF is a spike, not that editor.

### Reference ambiguities and capability boundaries

- The Markdown and screenshots retain Serato names, account/activation and hardware-specific sections alongside DRDJ branding. Treat interaction/layout as the reference; account flows, device certification, DVS and library-format interoperability require explicit product/backend decisions. Manual p. 39's compatibility description is not a Rust interoperability claim.
- Overview section 49 calls the album-art mode a grid; manual p. 39 describes an album-art list. Start with the simple track table; keep the alternate layout unresolved until its own slice.
- The original numbered screenshot shows Extended mode; manual p. 21 distinguishes it from Horizontal mode. Do not label the same layout as both.

### Next bounded UI slice

**Scope:** reorganize the working two-deck prototype into the Extended reference regions. Preserve real load/transport/mix/seek/zoom callbacks and make space for library navigation/table without inventing library data or enabling unimplemented hardware/features. Keep detailed grid/analysis editing out of the normal transport surface where practical; a full transactional editor is a later slice.

**Expected files:** `ui/app.slint` (split into focused Slint components if needed), `src/controller.rs` and headless UI tests, plus this inventory and the validation log. Audio/DSP changes are not needed for the initial layout slice.

**Acceptance:** shared waveform region and distinct library/status areas; correct deck identity after reorganization; existing controls still affect the intended engine deck; absent data/unsupported capabilities remain explicit; geometry/event tests at supported sizes and keyboard focus checks where bindings are introduced. Run fmt, strict Clippy, tests and release build. Source-image inspection is not visual acceptance of the application; rendering/visual and Windows/macOS evaluation gates remain open.

Classify each item as built-in, styled built-in, or custom only after a Slint prototype and API review.

| Component | Expected approach | Key concerns |
| --- | --- | --- |
| Transport play/pause/cue/sync buttons | styled/custom control | keyboard, MIDI state, active/disabled feedback |
| Deck status and track metadata | styled controls | responsive layout, truncation |
| Multicolor scrolling waveform | custom renderer | smooth rendering, zoom, beat grid, cue markers, low CPU/GPU usage |
| Overview waveform | custom renderer | seek, progress and markers |
| Jog wheel / platter | custom control | touch/mouse drag, angular feedback, latency |
| Pitch fader | custom/styled control | precise mapping, fine adjustments |
| Volume fader and crossfader | custom/styled control | accurate input and visual state |
| EQ/filter/gain knobs | custom control | rotary input, value scales, reset gestures |
| Level/VU meters | custom control | animation, clipping indication |
| Hot cue pads | custom/styled control | color/state mapping, controller feedback |
| Loop controls | styled/custom control | loop length and active state |
| Mixer channel strips | composite control | reuse, layout and signal indicators |
| Browser, crates and track table | built-in/styled with custom behavior | virtualization, sorting, keyboard navigation |
| Audio device and settings panels | mostly built-in/styled | accessibility and validation |
| Notifications, tooltips and dialogs | mostly built-in/styled | consistency and focus management |

## Definition of done per control
- State model and input events specified
- Mouse, keyboard and controller behavior defined
- Theme and visual reference approved
- Performance tested where animated
- Automated tests and visual review completed

Reference visuals must be used for design comparison, not copied proprietary assets.

## Local overview checkpoint (2026-10-24)

The overview now uses custom Slint rectangles and a TouchArea, backed by up to 256 real PCM peak bins per deck. Headless tests cover peaks, pointer/time mapping, accessible percentage input, marker geometry at two widths and independent load/reload state. The detailed scrolling/multicolor waveform remains open. This does not finalize the framework or satisfy the visual/performance definition of done above; no visual review or rendering profile has been completed.

## Detail-view checkpoint (2026-10-24)

A second Slint peak-envelope view follows each deck's playhead and exposes 2–32-second zoom buttons. Its local click range and playhead are headless-tested; unchanged viewports reuse their model. The full-track overview remains separate. Frequency colors/beatgrid and rendering/visual qualification remain open; this is not a completed framework-evaluation gate.

## Frequency-color checkpoint (2026-10-24)

Overview/detail bars now receive colors from prepared low/mid/high band-energy data. The shared palette and legend use bass red, mids green and highs blue. Automated tests verify data classification, model correspondence and viewport/deck behavior. The colors have not undergone visual/design approval; no rendering performance or cross-platform acceptance gate is closed by these tests.

## Manual-grid checkpoint (2026-10-24)

The detail view now supports an explicitly applied manual 40–240 BPM grid and clear action per deck. Strong lines mark a user-anchored 4/4 downbeat; all lines are limited to the visible range. Headless tests cover accessible controls, geometry at two widths, zoom and track lifetime. Automatic BPM/meter detection and visual/framework acceptance remain open.

## Tempo-suggestion checkpoint (2026-10-24)

The BPM row now optionally offers USE ~ to copy an experimental loading-time tempo candidate. With no candidate it is disabled. It does not silently set a grid, start playback or alter tempo. Actual headless load/use/apply, error/cancel and reload tests cover the flow. Visual/design approval and reliable automatic musical analysis remain open.

## Extended workspace / library checkpoint (2026-10-24)

The current layout now has compact deck heads and overviews, shared full-width stacked detail waveforms, the real channel/crossfader strip, a substantial session-library table and separate application status. `CompactDeck`, `WaveformStrip`, `DetailWaveform` and `TrackLibrary` encapsulate reusable UI in app.slint. Headless layout assertions cover 850x740 and 1280x860; this is structural regression evidence, not visual approval.

Library controls now import real selected MP3/WAV paths in the background, search filename/path, sort filename/type/path, select using stable IDs and load into either paused deck. Arrow navigation is wired locally to the table focus. Busy/error/cancel/empty/no-match states and playing-deck protection are represented. Duration/tag/BPM columns, folder scans, persistence, crates/history, drag loading, additional display modes and the transactional grid editor remain open. Existing inventory statuses should be read with this dated checkpoint; no reference item is declared fully accepted by this change.

## Durable catalog checkpoint (2026-10-24)

Persistence is now implemented for track IDs/paths. The table footer shows restoration/saving/saved or persistent storage-error status, with Retry Save after a write failure. Missing entries retain identity/path and show MISSING. Startup failures visibly enter session-only behavior without overwriting the original data. Headless tests exercise save/restart/restore/load and error/retry; this does not establish native visual acceptance. Tags/crates/history, relocation, folder scans and session/deck state restoration remain open. LIBRARY_STORAGE.md defines the storage boundary.

## SQLite storage checkpoint (2026-10-24)

The same library controls now use SQLite through the background worker; no new display mode or speculative metadata/crate controls were added. The footer distinguishes new/restored/migrated/recovered SQLite catalogs and retains visible save-error/Retry Save behavior. A headless JSON-to-SQLite controller/engine restart test loads the same stable ID into B and then A, paused, while preserving original JSON bytes. Existing Extended geometry, selection, filtering, mixer and waveform regressions remain applicable. Native visual acceptance remains open.

## Metadata table checkpoint (2026-10-24)

The table now displays title (filename fallback), artist, album, duration, type/missing state and full location. Artist/album/duration headers sort the existing stable-ID model; search includes tags and filename/path. Unknown fields display an em dash, header-derived time has a tilde, and a full load replaces it with decoded length. Metadata inspection errors mark TYPE with `?` and add an accessible row description; they do not imply a successful decode. New headless tests exercise tags, artist/album search, numeric duration sorting, retained selection, persistence and paused A/B loading after a fresh engine/controller restart. Existing two-size structural checks remain; no visual/native acceptance is claimed. Crate and tag-editing controls remain open.

## Flat-crate navigation checkpoint (2026-10-24)

A reusable left CratePanel now shows All tracks, sorted crate names/member counts, New/Rename/Delete actions and an independent add-target dropdown. Name editing has Save/Cancel, and deletion has Confirm/Cancel; navigation cancels stale confirmation. The table composes crate membership with tag/path search and sorting, clearing hidden selections and showing scope totals. Import targets the crate captured at start; Add/Remove act on stable track IDs. Headless actions verify duplicate adds, invalid names, rename sorting with target identity retained, cancellation, deletion/file retention, continued A playback and restarted B loading. Bounds checks at 850x740 and 1280x860 remain structural, not visual acceptance. This partially implements overview 51/54; subcrates, smart crates (53), external integration (52), manual order and autoplay (50) stay open.

## Transactional constant-grid editor checkpoint (2026-10-24)

Each detail toolbar now opens Edit Grid rather than immediately changing its deck. Editor controls replace that toolbar while the waveform remains available: decimal BPM, suggestion-to-draft, Set 1 at the current playhead, ±10 ms slip, zoom, Clear, Save and Cancel. GRID OFF also opens a staged removal. DRAFT captions distinguish preview from committed state; first-downbeat/red and numbered-bar data are derived from the anchor. Initial whole-BPM seed controls are disabled for an existing grid, whose decimal value is edited in the dedicated row. Accessible Save/Cancel tests verify invalid input, no active-grid change before Save, no transport/fader changes, same-track conflict protection, stale-draft cancellation and durable restart/reload/deletion. Existing two-size structural/geometry tests stay green; native visual/interaction approval is not inferred. This is constant 4/4 editing only; warp markers, variable tempo/meter and musical acceptance remain open.

## Hot-cue deck controls checkpoint (2026-10-25)

Overview 48/58 now has eight numbered pads per deck, occupied-state color strips, accessible cue name/source-time descriptions and a separate 1–8 delete-slot selector/Delete action. Empty pads store the current decoded source position; occupied pads seek while preserving Play/Pause. No temporary hold-to-play behavior is implied. Slot edits enter the existing background catalog-save/Retry Save flow. Same-path decks show one shared cue set; different tracks remain independent. The deck region increases from 166 to 230 px while retaining the library minimum and shared waveform layout. Headless geometry is checked at 850x740 and 1280x860; native visual/usability review remains open.

Loading/restore/import blocks cue actions. A saved cue at/beyond the currently decoded EOF is rejected visibly without moving playback; deletion permits repair after changed media. Setting at EOF is rejected. Reload/failure/restart tests verify cache ownership. This is partial cue parity: name/color editing, waveform markers, keyboard shortcuts, quantization, hold-to-play, physical latency and controller feedback remain separate requirements.

## Cue metadata editor and markers checkpoint (2026-10-25)

The existing slot selector now supports Edit for an occupied slot. Its row becomes name/RGB inputs and Save/Cancel, without expanding the deck height. Colors accept RRGGBB or #RRGGBB; invalid fields keep the draft open and committed data unchanged. Cancel does not save. Shared-track concurrent edits/deletion are detected on Save. Track replacement discards the associated draft; failed loads retain it. Cue position/transport/mixer are unaffected by metadata edits.

Overview/detail strips render bounded cue lines and numbered, named labels in each saved RGB color. Overview uses full-track percentages; detail transforms them into the visible start/end range and hides outside positions. Invalid cached positions at/beyond decoded EOF remain editable/deletable but have no marker. Marker lines/labels are display-only; the existing pad and waveform seek controls retain their interactions. Eight-slot bound and model reuse limit overlay data, not measured rendering cost. Tests verify geometry at 850x740 and 1280x860 and visibility across seek/zoom. Overlapping labels, contrast, native input/focus and visual usability require later acceptance; headless geometry does not settle these.

## Manual loop engine checkpoint (2026-10-25)

Overview 31/33/56 remains open at the GUI boundary. A frame-aligned manual loop engine now supports In/Out, activate/exit and wrapped position reporting, with offline audio tests; no loop buttons or active-state display exist yet. Seek/Cue disables looping while retaining boundaries, and a successful track installation resets them. The next slice must expose the real engine state and test actual controls; saved loops, auto-loop lengths, quantization and native/hardware acceptance remain separate.

## Manual loop GUI checkpoint (2026-10-25)

Overview 31/33/56 now has partial manual-loop GUI behavior: IN, OUT, LOOP/ACTIVE and EXIT per deck with actual engine bounds/active captions. OUT requires an In point, activation requires valid bounds, and EXIT requires an active loop. Loaded/source-operation guards apply, while engine validation handles invalid/reversed/EOF bounds. In/Out only set boundaries; activation is explicit. Seek/hot-cue recall/Cue exits without clearing stored session boundaries; track replacement clears them. These are per-deck transient loops, not durable saved-loop slots.

The deck region increases from 230 to 260 px. Headless geometry confirms all loop buttons remain within window width and above shared waveforms at 850x740 and 1280x860, with the library minimum/status layout retained. Actual accessible actions test the engine wiring and state isolation. This does not establish native visual/usability acceptance. Saved-loop slots, beat-length auto-loops, quantization, loop waveform overlays and controller feedback remain open.

## Saved-loop data foundation checkpoint (2026-10-25)

SQLite schema 6 and the library snapshot now retain eight fixed named In/Out slots per track. This is data-layer work only: existing manual-loop buttons remain transient, with no saved-slot Save/Recall/Delete controls yet. Slots do not auto-activate on load. The following GUI slice must validate stored boundaries against actual decoded duration/sample frames before installation, preserve the previous deck state on failure, and expose durability through the existing footer. Names/slots are internal DRDJ data; external saved-loop compatibility is not established.

## Saved-loop GUI integration checkpoint (2026-10-25)

The per-deck SLOTS switch reuses the existing cue-pad/editor region for eight-slot selection, an optional name, Save, Recall, Delete and stored metadata description. The manual loop row remains accessible in either panel; deck height remains 260 px. Switching back retains any cue draft; successful source replacement clears the saved-loop name input. Empty names receive Loop N. Save refuses occupied slots; replacement requires explicit Delete. Recall loads inactive boundaries and explicitly instructs LOOP activation; deleting a saved slot does not exit an independently active deck loop. Guards and the existing save/footer report failures/durability.

Headless tests validate controls/geometry in both panel modes at 850x740 and 1280x860. These checks caught initial overflow, resolved by panel reuse and bounded input width. No visual/usability acceptance is implied. Auto-loop lengths, snapping/quantization, loop waveform overlays, saved-slot name/replace editing and hardware feedback remain open.

## Auto-loop and resize checkpoint (2026-10-25)

The tools selector cycles CUES → SLOTS → AUTO → CUES (button text names the next panel; stable accessibility label is Deck A/B deck tools panel). All three panels share the prior deck allocation. AUTO offers 1/8–32 beat lengths, explicit AUTO LOOP activation, half and double. A committed manual grid is required to create an auto-loop; a pending grid draft cannot change its timing. Half/double operate on existing manual or auto boundaries, retaining In and active/inactive status. A caption explicitly identifies start-at-playhead and no snapping. Missing grid, loading and missing boundaries disable appropriate controls; source-end/size errors report status and preserve state.

Three-panel headless geometry at 850x740/1280x860 and actual button workflows remain required evidence, not native visual approval. Auto-loops may start off beat; quantization, loop-range overlays, native focus/rendering and hardware feedback remain open.

## Loop-range overlays checkpoint (2026-10-25)

Overview/detail strips now draw a subtle loop area behind PCM bars and In/Out lines/labels above the beat overlay, with active green/inactive gray mapping. The playhead/cue overlays and seek TouchArea keep their existing behavior; the loop display itself is not draggable/clickable. Accessible loop-area descriptions include state and source In/Out times. Only actual complete deck boundaries are shown, including inactive recalled ranges; saved library slots do not activate a display until recall.

Clipping and endpoint visibility are computed from precise source/view durations before f32 conversion; only viewport-local coordinates reach Slint. This preserves final-frame loop visibility on long tracks whose absolute percentages collapse to the same float. Subpixel fills are represented by one pixel within bounds, including at EOF; overlapping endpoint labels remain a visual-usability limitation. Exact Out-to-viewport-start and In-to-viewport-end contacts show no shaded interval. Headless tests cover 850x740/1280x860, full/partial/offscreen ranges, zoom, state/source lifetime, and a synthetic600-second48kHz final-frame case without allocating its PCM. Color assignment/geometry are implementation evidence, not a native visual/contrast/rendering-performance acceptance result.

## Optional Auto Loop beat alignment checkpoint (2026-10-25)

AUTO now includes a per-deck BEAT SNAP checkbox (off by default) plus a caption identifying nearest manual-grid beat or playhead behavior. Checkbox changes alone have no engine side effects. New Auto Loop creation uses committed BPM/anchor, nearest nonnegative whole beat with later ties and source-frame rounding. It can move the resulting loop start earlier/later than the current playhead; this is immediate start alignment, not deferred/scheduled quantized triggering. Invalid full regions fail without changing existing playback/bounds. The checkbox is disabled without a committed grid or while loading; the controller still validates direct callbacks.

The second AUTO row grows to26px within the existing260px deck allocation. Headless geometry includes the checkbox at850x740/1280x860 alongside all three panels; native focus/contrast/layout review remains open. Manual boundaries, cue recall, half/double and saved slots do not snap, and the setting is not persisted across application restarts. Tempo/keylock/sync and global quantization remain open.

Beat boundary/tie decisions use the exact binary rational value of committed BPM with checked integer arithmetic, avoiding float floor/ceil errors around zero and midpoint boundaries. ±1 ns around a midpoint is covered by regression tests; this numerical correctness does not establish musical grid accuracy.

## Playback speed/pitch engine checkpoint (2026-10-25)

Overview30/32/57 remains open at the GUI boundary. The engine now supports validated0.5–2.0 coupled playback speed/pitch with original-source position and loop semantics; there is no pitch fader/range/reset or original/current BPM display yet. This engine-first slice does not expose an unverified GUI control. New tracks reset to1x, failed loads preserve the installed factor, and seek/Cue/loops retain it. GUI wiring must present requested/effective speed honestly, retain source-time displays and verify actual controls. Smooth live-fader updates/resampling quality and physical latency need separate qualification; keylock/Sync are not implemented.

## Pitch GUI checkpoint (2026-10-25)

The deck tools cycle CUES → SLOTS → AUTO → PITCH → CUES. PITCH reuses the existing panel height for a-50..+100% slider,1x reset, requested/effective percent and source/current BPM text. BPM is labeled Manual for committed grid data, Estimate with~ for the existing experimental candidate, or unavailable. Effective speed, not the requested pre-rounding factor, scales current BPM. Source time/loop overlays keep original-track coordinates; pitch does not edit the grid or tags.

Events are latest-wins per deck and apply on the next GUI tick; source replacement/loading guards discard stale queued changes. The source-bound queue prevents a gesture for an old track affecting a new one. Successful new loads reset1x and reflect that thumb position; failed loads retain it. No UI sample-rate field, live-audio quality claim or silent persistence is introduced. Four-panel headless geometry at850x740/1280x860 passes; native focus/usability/contrast and physical audio remain unqualified. Selected pitch ranges, fine gestures, smoothing/live transition qualification, keylock and Sync remain open.

## Pitch ranges and fine control checkpoint (2026-10-25)

PITCH now includes±8%,±16%,±50% and Wide(-50..+100%) range selection and± fine buttons for0.01 percentage-point steps, within the existing shared panel. The chosen range limits new GUI gestures; it does not clamp applied audio. Outside the selected range, actual requested/effective values and an Outside range state remain visible, the fader is absent and fine controls disable; widening or1x reset restores fader operation. A valid range change cancels pending gestures without seeking or changing rate. The range is per-deck/transient; successful new loads retain its selection but reset speed1x.

Slider requests round to0.01pp and fine steps use integer units against the latest valid pending target. Multiple clicks before a33ms tick accumulate precisely, while a boundary click cannot discard the last valid queued target. Native fine-button enabled state follows applied values; pending-edge validation is still enforced. Two-size headless layout tests include range/fine controls; all existing source-time/mixer/transport guards remain. Native keyboard/focus/usability, physical/audible transition quality, nudge, keylock and Sync remain open.

## Temporary nudge engine checkpoint (2026-10-25)

Overview30/32/57 now has an engine-only temporary relative±2% nudge state separated from the base fader. Existing captions can identify active nudge and actual effective BPM while the fader/range uses permanent base pitch. No hold-to-nudge control exists in this slice. The next GUI slice must never latch a temporary rate after release, pointer cancel, focus/panel change, loading or shutdown. Engine pause/Cue/seek/new-track cancellation and idempotent neutral release already exist. No native hold/release/focus acceptance or artifact-free transition claim is made here.

## Momentary nudge GUI checkpoint (2026-10-25)

Tools now cycle CUES/SLOTS/AUTO/PITCH/NUDGE. NUDGE reuses the existing panel height for HOLD -2%, HOLD +2%, Cancel and actual pitch/BPM captions. Hold means a relative multiplier against the current permanent base, not an additive fader edit. Pointer down focuses the dedicated keyboard scope; Space/Return have matched press/release, with autorepeat ignored. Direct focus loss, panel change, pointer cancel/release, transport, loading, source replacement, EOF and shutdown neutralize. Instant accessibility actions cannot leave a held rate. Focus has a border; native appearance/usability remains unqualified.

Source-bound controller sessions keep independent deck inputs; opposing holds yield neutral, invalid remaining directions cancel all, and release bypasses busy guards. Tick cancellation is bounded by the existing33ms GUI timer rather than a real-time audio guarantee. Two-size headless geometry/event/state checks include the fifth panel. No listening, visual, physical latency, live transition quality, Sync or keylock acceptance is implied.

Review follow-up: successful occupied hot-cue recall and all successful GUI loop commands/saved-boundary recall now clear held ownership, including neutral opposing holds; rejected actions preserve it. A dedicated regression covers hot-cue/loop Exit/In/saved recall and invalid cue/loop actions. Each NudgeHold uses explicit first-input ownership: the initiating left pointer, Space or Return owns the gesture until its release; additional simultaneous inputs are ignored and require a fresh press. Unrelated key/right-button releases cannot cancel the owner. Mixed-input tests cover this policy. This avoids both premature neutralization and stale aggregate releases after transport cancellation.

## Opus1: independent deck busy states (2026-10-25)

Deck controls/grid editors use left-busy/right-busy, derived from restore readiness and the matching load owner. Loading Deck B or merging an import does not disable pitch/nudge/loops/Hot-Cues/grid editing on Deck A, and conversely. LOAD and library/crate workflows retain single-job admission. Controls for the loading deck remain blocked. Direct callbacks retain the same guards and existing source/editor conflict checks. Focus/panel/transport cancellation of momentary input is unchanged; this is headless workflow evidence, not native dialog/focus or listening qualification.

## Opus2: renderer-backed pitch and playhead (2026-10-25)

Pitch/nudge now publish rate commands to a stable renderer without seeking or replacing its Sink. The original-source playhead is published after complete output frames rather than reconstructed from the5ms Sink timer. Requested/effective pitch and BPM captions keep the same contract. Same-track seek/Cue/loop/pause use frame-boundary commands; explicit seek target is visible until acknowledged. Device output format is injected from the actual stream configuration. Existing focus/input/restore/source guards remain. Fifty short-flank ramp regressions measure rendered continuity at supported fixture rates; no native visual/listening/physical latency or DSP anti-alias quality acceptance is inferred.

## Opus7: background-job cancellation (2026-10-26)

The library footer shows CANCEL while a load/import job owns the single slot. Requesting cancellation is idempotent and keeps that slot busy until completion acknowledgement; it does not launch overlapping replacement jobs or cancel storage commits. The worker checks cooperatively and the controller rejects late ready results before deck/library mutation. Closing the controller requests cancellation too. Native file pickers/OS reads cannot be force-closed by this token; no native interaction acceptance is claimed. Headless tests verify active-button geometry at850x740/1280x860, preserved deck/gain/library state and recovery with a fresh job token.


## Opus4: distinct LOOP and RELOOP controls (2026-10-26)

The original user manual printed50–51 separates Loop On/Off and Slot Reloop. LOOP toggles activation, preserving source phase inside the interval; activation from outside starts at In. RELOOP replaces the redundant EXIT button and always activates/restarts at In. Both remain available for valid boundaries while active, preserve transport, and use existing per-deck busy/restore guards. The programmatic Exit callback remains available. OUT still captures the boundary; no automatic start is inferred. Saved-loop recall continues to install inactive boundaries. Headless control/ramp/geometry tests do not establish native visuals, focus or listening acceptance. Fine editing, auto-loop toggle/resize parity and direct slot controls remain open.


## Manual-grid tempo MATCH (2026-10-26)

A MATCH button beside pitch reset performs one-shot tempo matching to the other deck, retaining source phase rather than seeking a beat. Both confirmed manual grids and loaded/control-ready decks are required. It uses reference effective playback BPM, rejects transient nudge/held ownership, pending pitch and unsaved grid edit sessions, and respects the target pitch range. Direct callbacks repeat these guards. The button is disabled when unavailable; a direct rejected callback supplies the reason in status. No latched Sync/master mode, phase alignment, estimated-BPM fallback or keylock is implied. Current requested/effective pitch/BPM captions remain authoritative. Two-size headless geometry includes MATCH in both decks; native visual/focus/listening acceptance remains separate.
