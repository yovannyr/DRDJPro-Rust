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
| Tempo/pitch/nudge | 30, 32, 57 | Manual pp. 25, 29, 41 | Open: audio tempo/pitch, temporary nudge and selectable ranges. Editing grid BPM is not playback speed control. |
| Slip/keylock/sync/censor | 13, 15, 18, 44 | Overview numbered sections | Open; require engine behavior and explicit state before active UI controls. |
| Track gain/output meters | 14, 43 | Overview sections 14, 43; practice controls manual p. 29 | Channel gain sliders exist; persistent per-track gain and measured output meters do not. |
| Time/overview playhead | 41, 42 | Manual pp. 25–27 | Overview/seek exist. Current display is elapsed/total; reference asks elapsed/remaining and metadata time. Add clear labels and fractional-time precision in a separate slice. |
| Detailed waveform/beat matching | 34, 35 | Manual p. 28 | Colored following envelope/zoom/click-seek exist. Shared placement, hold/drag scrubbing, keyboard +/- zoom, beat-matching transient strip and pitch scaling remain open. |
| Grid editing | 46 | Manual pp. 53–58 | Simple manual constant-BPM anchor exists. Dedicated Edit Grid mode, Save/Cancel, numbered bars, red first downbeat, adjust/slip tools and warp markers remain open. |
| Repeat/autoplay | 47, 50 | Manual pp. 26, 28 | Open: repeat the same track versus advance to the next crate track. These are different policies, neither implied by EOF/restart-on-Play. |
| Cue-point panel | 48, 58 | Overview sections 48, 58 | Open: up to eight stored cue points, names/colors and triggers. Existing Cue-to-zero is not hot-cue parity. |
| Flip/loop panels | 31, 33, 56 | Overview numbered sections | Open: flip sequences, auto-loop UI and saved loops; require separate engine and persistence slices. |
| Library actions/search | 36, 37, 38, 39, 40 | Manual p. 30 | Open: Search, History, Prepare, Browse, Files. Native file picker is not a library browser. |
| Library table/view | 45, 49 | Manual pp. 39–40; overview screenshot | Open: large track table, sort headers, selection and list/art view. Current library footer is a status placeholder. |
| Crate navigation | 51, 52, 53, 54 | Overview numbered sections | Open: crates, subcrates/smart crates, import and create actions. No iTunes/Serato-format compatibility is implemented. |

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
