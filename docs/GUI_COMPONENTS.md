# GUI component inventory and reference baseline

## MC4000 S10: Split Cue and contextual hardware meters

MON now has **SPLIT CUE**. Channel16 Note28 is an absolute switch request:
NoteOn>0 enables, NoteOn0/NoteOff disables; repeated states are idempotent.
On devices with at least four output channels, OUT3/4 carries averaged mono
Cue on the left and averaged mono master on the right. Headphone gain applies
to both; the existing linear mix knob balances `(1-mix)*Cue` against
`mix*Master`. At the center each side has half weight; at either endpoint the
other side is silent. Normal stereo cue/master mixing returns when Split is
off. The mode change crossfades over5ms with the existing gain/mix ramp.
Master routing/audio remain independent of the headphone controls.

Split alone reserves OUT3/4, even with no selected cues and mix0. Enabling it
fails without four output channels, with master already on3/4, or with native
multichannel tracks loaded. Such tracks cannot be installed while Split is
active. Errors restore authoritative GUI state. Profiles store Split alongside
gain/mix/cues under the existing endpoint/backend/channel-shape identity.
Older monitor JSON defaults Split off; malformed values remain rejected and
preserved through the existing profile error path. No SQLite schema change.
Tests simulate profile reopen4→2→4; this is not physical device acceptance.

Hardware meters keep manufacturer CC68/69 and the existing seven segment
thresholds. With both channel faders closed, left/right show their respective
sounding deck's pre-fader peak (paused decks are dark). Once either fader is
open, both show the summed post-fader/FX stereo master, except a selected cue
deck with its own fader closed and audio playing/scratching shows its pre-fader
peak on its side. A paused cue side returns to master. Master is measured from
logical channels1/2 before routing, excluding headphone/auxiliary channels and
the physical MASTER knob. GUI deck PRE/PRE LAST and clip holds are unchanged;
hardware clipping follows the latest complete50ms peak window, not those holds.

A separate control-plane producer observer acquires on progress and darkens
hardware meters after over100ms without observed progress or on a long poll
gap/output failure. It needs new progress to reacquire. This is not a hardware
watchdog: a stopped UI/MIDI loop cannot send a clearing frame until it resumes.
These are producer sample peaks, not calibrated device/true-peak meters.

Mode/gain/mix use one coherent atomic word (the validated nonnegative gain's
unused sign bit stores Split). The router caches only the current cue pair
when splitting/ramping so it can average L/R without allocation or locks;
this can finish a logical producer frame at the left cue sample, rather than
at the last physical channel. Sync observation remains a producer-clock
contract; per-channel phase-pair regressions cover this lookahead. The new
master accumulator does bounded stereo work and publishes every50ms. Release
timing is checked separately with a100ms offline block; native headphone
orientation, sound, LED response and sustained latency remain open.

## MC4000 S9: fast navigation and loop jog editing

SHIFT+Jog seeks by 50ms of source time per relative MIDI unit (ten times the
ordinary paused jog step). Shifted search notes32/33 hold backward/forward
search while global SHIFT is held: each timely control tick adds 8 seconds of
source displacement per elapsed second to the current software playhead.
Normal playback advances between these discrete seeks; this is not continuous
reverse DSP. Opposing held buttons cancel displacement. Pause remains paused;
forward search at EOF pauses, and both boundaries clamp and end the hold.

Release, a real SHIFT change, focus loss, MIDI recovery/overflow/disconnect,
loading, changed source/renderer/transport/loop bounds, or a control gap over
100ms cancels the held session. The 100ms boundary is inclusive; long/backward
clock intervals cause no catch-up jump. This is a control-thread observation,
not a device-latency deadline. Identical repeated SHIFT states are idempotent.
Manual navigation cancels Sync, queued quantized cues and jog bend, and exits
active loops while retaining their bounds. Pending tempo/device work, editors,
cue/bend holds and scratch ownership reject navigation. SHIFT changes release
hardware scratch on both decks, preserving play/pause intent; mouse ownership
is retained. A touch ignored during navigation must release before ordinary
touch-jog can resume. Explicit new inputs can acquire a new session.

The AUTO tools panel's **JOG OFF / JOG IN / JOG OUT** button selects the loop
boundary. Hardware Loop In/Out (channel5/6 notes56/57) toggle the respective
edit mode while a loop is active; on inactive loops they retain their existing
set-point behavior. The ordinary GUI IN/OUT buttons retain set-point semantics.
Loop-edit Jog takes priority over SHIFT. Starting held search exits loop edit.
The mode appears in the loop caption, and leaving AUTO cancels it immediately.

Each loop-jog unit moves the selected edge by 1ms, rounded to source frames
(at least one frame per movement). In stays before Out, Out stays within EOF,
and the minimum loop is one source frame. An active playhead excluded by new
bounds returns to In; other active positions and play/pause intent are retained.
Inactive loops only update bounds. Save to a persistent loop slot explicitly;
edit modes themselves are transient. Source/renderer identity, command version
and bounds guard the session even when shared PCM is reused and command
counters restart. No new DSP, render locks or audio-thread allocations; the
existing renderer receives coherent seek/loop commands. Sensitivity and search
rate are DrDJ defaults, not manufacturer-specified measurements. Native sound,
end-to-end timing and hardware feel remain open.

## MC4000 S8: curated Prep Queue

The separate **Prep Queue** tab holds at most 5,000 deduplicated library track
IDs in curated order. **To Prep** appends the selected track once; **Remove**,
**Up** and **Down** edit the queue without deleting files or library tracks.
Normal searches filter the queue while preserving that order. Column sorting
is disabled in this view. Moving a filtered selection swaps it with its adjacent
entry in the complete queue, which may be hidden by the current filter.

MC4000 channel16 Note27 (LOAD PREP) appends the settled selected track; Note16
(OPEN PREP) opens this tab, clears the search and selects controller track
browsing. Panel cycles through six tabs; the crate target also includes the
queue. Text editors/search focus and in-progress library/loading operations
are protected. Queue actions never initiate analysis or start playback.
Loading a queued track uses existing deck protection and retains the entry.
Remove it explicitly when finished. The original **Prepare** tab remains the
analysis view with its original Analyze Files workflow.

SQLite schema17 stores ordered track IDs with unique membership and foreign
keys. Migration from schema16 initializes an empty queue. Queue deltas use the
existing transaction, acknowledgement, retry and shutdown flush paths; older
save acknowledgements cannot erase newer queue edits. Missing files remain
queued, and relocation preserves membership by track ID. Queue edits can
rewrite at most 5,000 rows on the storage worker. No audio/render path changes.
Automated tests cover persistence/failure recovery and GUI geometry at 850x740
and 1280x860; native controller/visual acceptance remains open.

## MC4000 S7: Instant Doubles

Two consecutive LOAD presses on the same target within 350 ms (inclusive,
measured at control-thread dispatch using `engine.control_now`, not hardware
event timestamps)
copy the opposite deck. The first press must be an accepted normal library
LOAD and still starts that operation immediately; GUI LOAD remains unchanged.
A successful second press supersedes only its own first decode job. A failed
clone leaves that normal job intact. Other controller commands, alternating
LOAD targets, recovery, unrelated loader changes and source replacement break
the gesture. A completed first load can still be followed by the second press.

The target must be paused. The clone shares immutable PCM, waveform and prepared
Keylock data, captures the source's current software position, base pitch, grid,
Keylock and play/pause state, and adopts its pitch range with fader pickup reset.
It clears target loop bounds and temporary cue state. Persisted hot cues/saved
loops are looked up for the cloned track; no foreign track's cue state is copied.
Target mixer/EQ/FX and headphone routing remain independent. Missing/EOF source,
scratch, loop, nudge, cue holds/pending quantized cues, active Sync and pending
tempo/device/library operations reject the clone. Search/edit focus protection
continues to apply. A successful pair is consumed; a third LOAD starts a new pair.

This is a control-plane software snapshot, not sample-exact alignment at the
physical outputs. Replacement can allocate on the UI thread but does not decode
or copy PCM. Render-trigger completion adds one atomic acknowledgement per
executed quantized cue; no new audio-thread locks or allocations. Native timing,
audible transition quality and controller usability remain to be qualified.

## MC4000 S6: library navigation and LOAD

Global encoder CC0 moves through the complete filtered/sorted library;
SHIFT CC1 applies ten rows per relative step. Cross-page targets retain their
row offset, and bounded pending deltas wait for the existing search worker.
The selected row is revealed without taking keyboard focus. Ordering revisions,
pane changes and recovery cancel stale queued navigation instead of redirecting
a later LOAD. No full-library search or filesystem work is added to MIDI/audio
callbacks.

Encoder press switches the controller target between tracks and ordered crates
(including All tracks/History/Prep Queue); from focused search it explicitly returns to
tracks. Back selects the crate target. SHIFT+Back cycles existing Library,
Files, Crates, History, Prepare and Prep Queue tabs.
Open crate/grid/cue editors, focused loop-name fields and focused search are
protected. LOAD A/B validates the settled selected ID, availability and busy
state, then reuses paused-target loading. Missing/playing/busy targets report
errors and do not replace the other deck. Double-press cloning is described in S7.

## MC4000 S5: temporary transport CUE

CUE is now a source-bound temporary point, separate from persisted hot cues.
While paused away from the point, pressing CUE sets it without starting audio.
At the point, holding CUE previews; the last input release pauses and returns.
During ordinary playback CUE pauses and returns. PLAY during preview promotes
it to normal playback. Initial point is zero; replacement clears it. EOF,
loading, output failure, window deactivation and controller recovery release
unpromoted previews. GUI and hardware have separate hold bits.

GUI CUE is a pointer/Space/Return hold control. START retains the previous
cue-to-zero action (also SHIFT+CUE, note5); it does not overwrite the temporary
point. STUTTER (SHIFT+PLAY, note4) restarts playback from the temporary point.
Seek, platter and loop operations release temporary holds first; overlapping
hot-cue holds are explicitly rejected. Existing source-time seek/play APIs
run on the control plane; no new DSP, render-thread locks or allocation.
Native onset/release latency and listening remain unqualified.

## MC4000 S4: hardware hot cues

Pad channels 5/6 notes20-23 reuse the eight-slot persisted hot-cue workflow
for slots1-4 only; notes28-31 delete. Both NoteOff and zero-velocity NoteOn
release previews, including shifted releases. Hardware uses its own input bit:
disconnect/overflow releases it without clearing GUI holds. Paused occupied
pads preview until release; PLAY promotes to normal playback. Empty pads set
a cue without starting playback. LEDs use the closest supported blue/red/purple
palette color; arbitrary RGB remains intact in the library and GUI.

## MC4000 S3: sync and loops

Deck notes 2/3 explicitly enable/disable that deck's Sync follower; repeated
commands are idempotent and cannot cancel the other deck's follower.
Loop-channel notes 50/51/52/53/56/57 reuse Auto toggle, Reloop, half/double,
In/Out. Note63 (SHIFT+AUTO) creates four musical bars, not four beats.
Its start follows the existing quantize/snap preference; its end preserves
the fractional position within the bar four bars later, evaluating committed
meter, tempo and warp changes. Out-of-track regions fail without replacing
valid boundaries. Existing control-plane region publication is reused: no
new render-thread locks, allocation or DSP; native timing/listening remains open.

## MC4000 S2: modifier and pitch

Global SHIFT, pitch range cycling, paired 14-bit pitch CC41/9 and held
pitch-bend buttons reuse the control-plane tempo/nudge APIs. Pickup arms only
near or across the current target; GUI pitch, range, tempo match and track
replacement disarm it. Incomplete MIDI pairs survive polling without producing
partial tempo jumps. Hardware lifecycle release preserves GUI-owned nudges.
Held bend buttons take priority over transient untouched jog bends.
No new DSP or audio-callback work; physical fader calibration and listening
remain open in the [hardware test plan](MC4000_HARDWARE_TEST_PLAN.md).

## MC4000 S1: channel trim, EQ and filter

Deck MIDI channels 1/2, CC22-26 now control Trim, High, Mid, Low and Filter
through the existing `eq-changed` callback and validated engine settings.
Trim maps to -12..+12 dB, EQ bands to -24..+12 dB and Filter to -1..+1
(low-pass/off/high-pass). Both central values 63/64 are exactly neutral;
each side is linearly mapped to its endpoint. This is an application mapping,
not a measured hardware potentiometer calibration.

Ten atomic latest-value slots coalesce the absolute knobs before GUI dispatch;
they do not consume the bounded gesture queue. No synthetic knob state is
applied on connection. Existing GUI reset remains functional; the next physical
knob movement takes control directly (no pickup in this slice).
The existing five-millisecond DSP ramps, frame-coherent settings and pre-fader
cue signal path are unchanged. No new audio-thread allocation, lock or DSP
algorithm is introduced. Tests cover decode/channel isolation, coalescing,
neutral/endpoints, actual rendered trim level and unchanged transport/faders.
Native A/B regulator direction, detents and MASTER/headphone listening remain
to be qualified; automated tests do not assert physical latency or sound quality.


## User-provided UI references

### Analyze Files and Prepare

The compact tab strip includes Analyze Files and Prepare. Analyze Files queues available library tracks without a successful stored analysis, independent of selection, search or active crate, and analyzes them sequentially in the existing cancellable background-worker lane. New, failed and corrupt-flagged tracks are retried; healthy results are skipped, including after restart. Unknown BPM/key in an otherwise healthy result is not a failure. Each finished result updates the library and queues a save immediately; a processed/total/failure counter shows progress. Errors are reported with track ID and the last error, and do not stop the remaining queue. Corrupt-flagged results remain visible but count as failures and stay retryable. Cancellation stops the current file and remaining queue while keeping already completed results.

Prepare displays a compact table with ID, Song, Artist, estimated BPM, Camelot Key, Album and Length (hundredths of a second), plus recommended Gain dB and analysis status. Unknown values remain unavailable; absent tags are not invented. The selected track's recommendation/decode summary and monochrome peak overview remain above the table. At most 256 peak bars are rendered from the stored bounded overview, and unchanged results are not rebuilt every timer tick. Search/sort/paging and tab navigation remain usable during analysis; changing selection or tab does not redirect results or force Prepare again.

All track-table headers are sortable: #, Song/Title, Artist, BPM, Key, Album, Length, Gain dB, Analysis, Path and Type. Clicking again reverses direction; the active header shows an arrow. Sorting applies to the complete filtered library, not only the current page, including explicit sorting in History (its initial order remains most-recent-first). BPM, duration and gain use stored numeric values, Camelot keys use number then A/B, and analysis uses the displayed Analyzed/Corrupt/Missing/Pending order. Unknown numeric/key values stay last in both directions; equal values use stable track IDs as a deterministic tie-breaker. Analysis-based sorts update as results arrive while reusing the text-search index.

The footer's **Columns** panel provides a checkbox for each column and left/right buttons to change its position. At least one column must stay visible. Header and cell order/width share one model in all library tabs; footer arrow buttons scroll horizontally to otherwise off-screen columns without interfering with track drag-to-deck. Visibility/order are saved in SQLite `track_columns_v1` via the existing background transaction/Retry Save/close-flush lane. Invalid saved preferences produce a warning and session defaults; the invalid data is preserved until an explicit column change. Column resizing and drag-reordering are not implemented in this slice.

Completed results persist in SQLite schema 16 and survive restart and ordinary metadata refresh. Relocation invalidates them. Cancellation or analysis failure retains the previous result and reports the error in the status area. Selection changes do not redirect the result to another track. Analysis never changes beatgrids, deck gain or transport. Automatic gain/grid application, a separate manually curated Prepare queue and musical-corpus/native quality acceptance remain separate work; this is not full Serato Prepare-queue parity.

Analyze Files requires at least one available new/failed track and an idle library/load worker, not a selection or enabled audio output. It explicitly starts analysis of that retryable-library snapshot when clicked, not automatically on startup. Missing tracks and healthy cached results are excluded. Existing imports/loads/relocation remain serialized with the analyzer; regular playback is unaffected. Mono/stereo Ogg Opus uses the same strict pre-skip/end-trim decoder as playback, streaming packets into the shared loudness/BPM/key/waveform pipeline without retaining full-track PCM. Other supported formats retain the Symphonia path; the one-hour analysis limit and cooperative cancellation apply to both. Empty/tag-only files and unsupported/chained/multichannel Opus remain explicit errors, not fabricated successful analyses.

The `analyzer_supports_library_formats` regression covers bundled MP3, AAC, ALAC, FLAC, AIFF, Vorbis and mono/stereo Opus. Optionally set `DRDJ_ANALYSIS_TEST_FILE` to include a real audio file in that same read-only test; results are not written into the user's library. Fixture/headless checks do not replace musical-corpus quality or native acceptance.

### Compact library tabs (2026-10-31)

The library layout now follows the user-provided compact dark table screenshot and the Files/History panel organization on manual page 15. Library, Files, Crates and History form a compact tab strip above the table. Files exposes the existing native file/folder import actions and explains the target crate and reference-only storage. Crates exposes create/subcrate/smart/rename/delete and membership actions in the left navigation. History selects the real persisted history scope; leaving it restores the previously selected library/crate scope. Search, deck loading, page navigation, missing-track relocation, cancellation and Retry Save stay available.

Library buttons use 11px text with intrinsic label width plus 14px padding and 22px height, visible keyboard focus, Return/Space activation and accessible enabled state. Track/crate rows are 22px; alternating dark table rows and a blue selection follow the screenshot. Metadata can still elide when its column is too narrow; complete values remain in the accessible row description. No missing BPM/key metadata is fabricated to imitate the screenshot. Geometry tests cover all tabs and compact action bounds at 850x740 and 1280x860, plus scope restoration and existing import/crate/load flows.

This is a bounded layout slice, not full Serato parity or native readability acceptance. Files still uses the existing pickers, not the manual's multi-column disk browser. Folder dragging to create crates, a Prepare queue, metadata Browse filters, external-drive-local catalogs and alternate artwork views remain separate work. Deck/mixer controls and audio processing are unchanged.

Use `Softwareübersicht.md` and its numbered `Softwareübersicht.png` as the main screen inventory/layout reference, and `DRDJ-UserManual-DRDJ.pdf` for interaction semantics. The screenshot currently selects **Extended** mode. These are user-provided design/behavior references, not evidence that a feature exists in Rust or that the Slint spike has passed visual acceptance.

The PDF has 110 image-only pages. Focused visual reading for this checkpoint covered the contents, page 21, pages 25–30, pages 39–41 and pages 53–58. Page numbers below are the printed/manual page numbers, matching PDF page numbers. This is a focused UI review, not a complete audit of all 110 pages. Input PDF/Markdown/PNG sources were left unchanged.

The current tall deck cards and central mixer are a functional prototype. Next UI implementation should first align the **Extended workspace structure** with these references: global toolbar; compact deck/track areas and per-track overviews; a shared full-width area containing the two detailed waveforms; a substantial library area with navigation/search; and a separate status area. Practice-mode mixer controls have their own reference on manual page 29. Keep supported controls functional during layout changes.

### Numbered overview coverage and gaps

Every overview number 1–59 appears exactly once in this grouped inventory. “Open” means a requirement/reference, not an implemented control.

| Group | Overview numbers | Source detail | Rust status / implication |
| --- | --- | --- | --- |
| Workspace/display/deck selection | 4, 5, 29 | Manual pp. 21, 26, 29 | Open: Vertical, Horizontal, Extended, Stack and Library; active deck selection. Implement one usable reference layout before adding other modes. |
| Global tools/panels | 2, 3, 6, 7, 8, 9, 10, 20, 21, 22, 23 | Overview numbered sections; manual p. 21 for Quantize | Partial: global Quantize gates opted-in cue placement, scheduled cue triggers and Auto Loop snap on both decks. Link, Flip, recording, FX, sampler, video, MIDI, account/help/setup remain open. Full reference/native acceptance remains open. |
| Master/system status | 19, 26, 27, 28 | Overview numbered sections | Open: master output, actual dropout warning, clock and CPU telemetry; never animate invented measurements. |
| Playback mode | 1 | Overview section 1 | Only internal software playback exists. REL/ABS/THRU/changeover depend on actual DVS/input routing/hardware and must not appear as working modes yet. |
| Track metadata/BPM/key | 11, 24, 25, 59 | Manual pp. 25–26 | Filename and experimental BPM candidate exist. Artist/key tags, distinct original/current BPM and pitch-derived display remain open; absent metadata stays absent. |
| Virtual deck/platter | 12, 16 | Manual p. 25 | Round light-faced software decks show current BPM, pitch, elapsed/remaining source time and progress when MC4000 is connected. Physical platter position, outer progress ring and end warning remain open; these are distinct indicators. |
| Transport/eject | 17, 55 | Manual pp. 40–41 | Play/Pause and Cue-to-zero exist. Per-deck EJECT now stops/unloads that deck, clears its persisted startup assignment and retains its library track. Shift+Left/Right loads the selected Library track into A/B; headless focus, repeat, busy and playing-target tests do not establish native keyboard/visual/hardware acceptance. Previous/next library navigation remains open. |
| Tempo/pitch/nudge | 30, 32, 57 | Manual pp. 25, 29, 41 | Coupled pitch/ranges/fine and momentary nudge implemented with offline/headless evidence; native usability, transition quality and physical latency remain open. Editing grid BPM is not playback speed control. |
| Slip/keylock/sync/censor | 13, 15, 18, 44 | Overview numbered sections | Partial: software Keylock, one-shot matching/phase and bounded continuous Sync exist. A Keylock follower prepares its matched base asynchronously, shows a cancellable preparation state and becomes active only after fresh render frames. Nudge preserves pitch using bounded overlap/add. Musical/transient/hardware qualification remains open; Slip/Censor are open. |
| Track gain/output meters | 14, 43 | Overview sections 14, 43; practice controls manual p. 29 | Deck trim, channel gain sliders, independent pre-fader A/B sample peaks with manual clip hold and the separate post-fader bus peak exist. Persistent per-track gain, true-peak/loudness metering and physical output qualification remain open. |
| Time/overview playhead | 41, 42 | Manual pp. 25–27 | Overview/seek exist. Both decks display labeled elapsed/remaining/total original-source times with millisecond precision (truncated). Pitch does not convert these into wall-clock time-to-end. Native readability and metadata time parity remain open. |
| Detailed waveform/beat matching | 34, 35 | Manual p. 28 | Colored following envelope/zoom/click-seek exist. Focus-scoped keyboard +/- zoom is implemented; hold/drag scrubbing, beat-matching transient strip and pitch scaling remain open. |
| Grid editing | 46 | Manual pp. 53–58 | Partial: dedicated Edit Grid drafts support base-BPM editing, Save/Cancel, numbered bars/red first downbeat, anchor and ±10 ms slip tools, conflict protection and persistence. Up to sixteen bar-indexed meter changes and 64 beat-indexed BPM changes/warp markers can be set/replaced/removed with waveform preview. Numerators 1–16 and denominators 2/4/8/16 support fractional-quarter barlines; SQLite schemas 11–13 preserve meter denominators and warp markers while defaulting legacy grids appropriately. Warp-marker anchors remap the grid only; no audio time-stretching. Sync/tempo matching reject non-constant-tempo grids pending local-tempo support. Automatic meter-aware Sync and native visual/musical acceptance remain open. |
| Repeat/autoplay | 47, 50 | Manual pp. 26, 28 | Open: repeat the same track versus advance to the next crate track. These are different policies, neither implied by EOF/restart-on-Play. |
| Cue-point panel | 48, 58 | Overview sections 48, 58; manual pp. 45–46 | Eight persistent per-track positions/names/colors; set/recall/delete, name/RGB editing, quantization and numbered keyboard/held-preview semantics are implemented. Coincident waveform labels occupy distinct slots. Native visual/keyboard usability and physical controller feedback remain open. Existing Cue-to-zero is not hot-cue parity. |
| Flip/loop panels | 31, 33, 56 | Overview numbered sections | Open: flip sequences, auto-loop UI and saved loops; require separate engine and persistence slices. |
| Library actions/search | 36, 37, 38, 39, 40 | Manual p. 30; user Import Music/Prepare screenshots | Partial: compact Library/Files/Crates/History/Prepare tabs, indexed search and persisted history; Analyze Files processes all available library tracks progressively. Files uses native import pickers, not a disk browser. Metadata Browse filters and a separate curated Prepare queue remain open. Acceptance mappings: R-LIB-16/17 in [FEATURE_CATALOG.md](FEATURE_CATALOG.md). |
| Library table/view | 45, 49 | Manual pp. 39–40; overview and user Prepare screenshots | Partial: metadata table, selection and 200-row paging; Prepare adds BPM/Key/Gain/status and hundredths-length beside Song/Artist/Album. Analysis-only updates reuse the search index and current page. Worker search/sort still processes matching records; native maximum-scale analysis/playback performance and alternate list/art views remain open. R-LIB-18 in [FEATURE_CATALOG.md](FEATURE_CATALOG.md) maps exact row/geometry evidence; headless bounds are not native visual acceptance. |
| Crate navigation | 51, 52, 53, 54 | Overview numbered sections | Partial: persistent hierarchical static crates and saved-query smart crates, indented navigation, create/rename/delete, direct set membership and active-crate import are implemented. Smart filters search title, artist, album, filename and path, combined with the normal search. iTunes/Serato-format compatibility remains open. Parent crates show only their own direct memberships; child tracks are not implicitly aggregated. |

### Concrete behavior to preserve from the manual

- **Display modes (p. 21):** Library mode toggles back to the previous performance view with Space. Keyboard handling must respect focused text/search fields. Opening constrained panels is reflected in status. This is a future acceptance contract, not current behavior.
- **Track/deck information (pp. 25–26):** distinguish original/current BPM, pitch percentage/range, elapsed/remaining time, platter position and track progress. Missing artist/key/BPM tags must not be fabricated. Reference track-end warning begins 20 seconds before EOF and is configurable.
- **Overview (p. 27):** complete-track frequency colors, seek on click and minute/five-minute divisions. Current colors and seek cover only part of this contract.
- **Detail waveform (p. 28):** close-up around current position, press/hold scrubbing, +/- zoom and waveform scaling with playback pitch. Focus-scoped +/- zoom now uses the existing2–32s limits. Existing click seeking is not the documented drag interaction.
- **Library (pp. 30, 39–40):** search as typed; Ctrl+F focuses search; Escape/X clears it; sort through column headers; load a selection into A/B by drag/drop or Shift+Left/Right. The shortcut uses the existing load callbacks and leaves focused search fields and busy/playing targets protected. Library empty/loading/error states still need explicit Rust contracts.
- **Cue points (pp. 45–46):** keys 1–5 trigger left-deck cues and 6–0 right-deck cues; while a deck is paused, an occupied cue plays only while held, returning paused to the cue on release. PLAY/PAUSE while held keeps playback going. The two-deck mapping, focus-safe input and stacked coincident labels are implemented with headless tests; native keyboard, visual and hardware acceptance remain open.
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

The mixer now has a Show mixer EQ button and a collapsible pair of deck strips. Each strip exposes accessible dB sliders for Trim (-12..12), Low/Mid/High (-24..12), displayed values, a bipolar filter (-1..1) with LP/HP percent/OFF caption, a separate filter OFF button and a per-deck Reset for all controls. Controls work before loading and retain deck settings when tracks change; they are session-local. Sliders are the current functional input, not a completed rotary-gesture/reference-layout acceptance. Headless bounds are checked at 850x740 and 1280x860; native visual/listening acceptance, MIDI knob mapping and effects remain open. Independent channel meters are described below.

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

A reusable left CratePanel now shows All tracks, History, sorted crate names/member counts, New/Rename/Delete actions and an independent add-target dropdown. History contains one row per recently played library track, newest first; normal Play starts and promoted cue previews count, temporary cue previews do not. Name editing has Save/Cancel, and deletion has Confirm/Cancel; navigation cancels stale confirmation. The table composes scope membership with tag/path search, clearing hidden selections and showing scope totals. Import targets the crate captured at start; Add/Remove act on stable track IDs. Headless actions verify duplicate adds, invalid names, rename sorting with target identity retained, cancellation, deletion/file retention, continued A playback and restarted B loading. Bounds checks at 850x740 and 1280x860 remain structural, not visual acceptance. This partially implements overview 51/54; external integration (52), manual order and autoplay (50) stay open.

The library table offers **RELOCATE** only for the selected missing track. It inspects one replacement file on the existing background worker, retains that track's stable ID and associated library data, and rejects destination collisions or an old path still loaded on either deck. Headless UI/controller tests verify the worker flow and that relocation does not add a duplicate; native visual acceptance and directory-wide remapping remain open.

At startup, the controller restores saved A/B library tracks sequentially through the existing background loader. Both remain paused at source position zero; errors are reported without deleting the saved IDs. Explicit `--gui A B` tracks override this assignment. Headless restart, CLI-precedence, save/retry, close-flush and missing-media tests cover the flow; native startup and visual acceptance remain open.

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


## One-shot PHASE action (2026-10-26)

The NUDGE panel adds PHASE beside Cancel. It matches tempo and seeks the target to the nearest matching beat fraction of the other confirmed manual grid. Both decks must play with loops off, and MATCH prerequisites/range/editor/pending/held-nudge guards still apply. Direct rejected actions report why; neither action starts a paused deck. Existing inactive loop bounds remain. This is a single control-plane alignment from published source positions; subsequent seek/pitch/loop changes are not followed. It is not bar/downbeat synchronization or a latched Sync state. The audio command changes rate and seek together on an output-frame boundary. Headless geometry includes PHASE at850x740 and1280x860; native visuals, audible transitions and physical timing remain separate gates.


## Shared render bus integration (2026-10-26)

No control or latched Sync state is added. Existing transport, gains, source-time captions, MATCH and one-shot PHASE run through one private deck bus before the output mixer. The shared completed-frame clock is infrastructure for later timestamped phase publication/continuous correction, not a GUI timer or hardware-played position. Native visual/listening acceptance remains unverified.


## Timestamped one-shot phase inputs (2026-10-26)

No new control or continuous Sync mode. PHASE can use coherent source-phase pairs from the same completed bus frame; otherwise its previous explicit control-position behavior remains. Pending commands invalidate older rendered pairs. Paused/EOF sources may retain last rendered timestamps, which do not indicate current hardware time. Future continuous regulation must require eligible/fresh timestamps rather than the one-shot fallback. Native visual/listening/physical timing acceptance remains open.


## Freshness gate for rendered phase inputs (2026-10-26)

No new control or latched Sync mode. One-shot PHASE uses same-frame rendered inputs only when the5ms render-frame/read-window and100ms observed-progress policies pass; absent fresh inputs retain the existing explicit control-position behavior. Continuous regulation will require strict freshness rather than this fallback. Long control polling gaps force liveness reacquisition. These are producer-side checks, not proof of physical output timing or hardware continuity. Native visual/listening qualification remains open.


## Continuous SYNC follower control (2026-10-26)

A second row in NUDGE contains SYNC/SYNC ON with accessible action `DECK A/B continuous sync`. Only one follower is allowed; the other deck remains the reference. Initial activation requires a fresh eligible pair and performs one-shot tempo/phase alignment; continued operation changes rate only. Toggle off releases correction; another follower must wait until the current one stops. Temporary correction is distinct from base pitch and nudge and displayed with requested/effective values. The additional row remains within headless two-size layout bounds.

Pending/manual render commands, source/grid changes, active loops, not-playing/loading/restore, open grid edits and held nudge ownership stop following on the next control check. Transient missing fresh samples neutralize immediately and wait at most100ms before stopping. The audio-side100ms-produced-frame watchdog also neutralizes if the controller stops renewing. This is not proof of device timing, artifact-free transitions or musical Sync quality. Native usability/focus/listening/device acceptance remains open.


## Alignment period selector (2026-10-26)

The NUDGE Sync row adds an accessible `DECK A/B phase alignment mode` selector for Beat / Bar4/4. It controls PHASE, SYNC's initial acquisition and the active session's ongoing feedback period. Beat wraps at one beat; Bar4 retains position inside the four-beat period. Bar4 matches position inside the manual grid's four-beat period, rather than waiting for a future downbeat. Selection itself preserves transport/position. Changing a follower's target choice releases Sync/temporary correction; a reference deck's independent choice does not affect the other follower. Invalid values are rejected and reset to the stored session choice. Default is Beat, no persistence/schema change. Headless two-size geometry includes the selector; native visuals, usability and listening remain unqualified.


## Continuous alignment-period feedback (2026-10-26)

The existing selector now determines continuous SYNC feedback too. The target mode is captured at activation; the existing target mode-change cancellation/reacquisition behavior remains. No new controls or layout changes. Beat remains default. Error correction is still bounded and rate-only after initial acquisition; no claim of native visual/listening or physical synchronization acceptance follows from headless tests.


## QUANT for new cue placement (2026-10-26)

The non-editing CUE tools row contains QUANT (`DECK A/B quantize new cues`). It snaps only newly set empty slots to the nearest confirmed manual beat; an exact halfway position chooses later. Existing cues recall their exact saved timestamp even with QUANT enabled or the grid unavailable. A grid draft must be saved/cancelled before quantized placement. Endpoint/guard failures retain library and transport. The checkbox has session-only bidirectional bindings; enabling requires loaded/not-busy/grid-active, while turning an already checked option off is always allowed. This prevents a checked but unusable option becoming stuck after a gridless load. Headless geometry includes the checkbox at850x740/1280x860; no native visual or focus acceptance is inferred. Scheduled cue triggering is not implemented by this toggle.


## Precise original-source time display (2026-10-27)

Each deck now shows ELAPSED / LEFT on the first line and TOTAL on the second, in mm:ss.mmm with unbounded minute digits. Time is original-source time, including at non-1x playback rates; LEFT is not predicted wall-clock time until EOF. Milliseconds truncate smaller fractions, so less than1ms remaining displays00:00.000. Elapsed is clamped to duration and remaining saturates at zero. The full caption has a deck-specific accessible label/description. The former rounded percentage text is removed; the seek slider remains.

Headless tests cover unloaded zero values, independent deck seeking, paused/play-without-produced-frames behavior, different pitch factors, EOF, Cue-to-zero, arithmetic boundaries/long durations and two-line geometry at850x740/1280x860. Existing playback PCM tests exercise producer advancement. These are not native visual, accessibility-reader, physical latency or listening acceptance. Long captions may elide when constrained; the complete source-time caption remains the accessible description. No sample-accuracy claim is made for the millisecond display.


## Focus-scoped detail waveform keyboard zoom (2026-10-27)

The detail waveform in each deck can receive keyboard focus through a left pointer press or focus traversal. A pointer press still participates in the existing click-to-seek workflow; focusing by clicking is not a seek-free selection action. With that waveform focused, + halves its source-time window and - doubles it, bounded2–32seconds, using the existing zoom callback. Key repeats are allowed; at a boundary no zoom callback is emitted. Ctrl/Alt/Meta combinations and unrelated keys pass through; Shift is allowed to produce +. No new global shortcut is installed. Search, grid BPM and other text inputs retain their own input. Overview strips do not enable keyboard zoom. Existing controls' loaded/audio-enabled rules apply to detail zoom; no new load/restore guard is inferred.

A focus border and accessible waveform description identify the scope/keys; the native focus appearance and screen-reader behavior are not accepted by these checks. Two headless WindowEvent tests cover disabled/unloaded detail, both deck directions at850x740/1280x860,2–32s bounds, repeated keys, modifiers, refreshed viewport span, unchanged audio state after zoom, focus switches to search/grid text and zoom during grid edit. Source/display zoom does not change playback pitch or rendering and is not waveform drag/scrub parity.


## Post-fader bus sample peak (2026-10-27)

The mixer row now has BUS PEAK with dBFS text and a linear-amplitude bar. Measurement is the maximum absolute sample across all bus channels in the latest completed ceil(sample_rate/20)-frame window (50ms at ordinary supported rates). It is after both channel faders and the crossfader, before the outer/device mixer. No samples are changed. No completed window displays a dash; silence is -infinity dBFS; non-finite source samples invalidate their entire window and display BUS INVALID. At or above1.0 the bar is full and uses warning color; this is a digital full-scale warning, not a verified clipping detector. It does not implement a limiter, master gain, per-channel meters, true peak, RMS or loudness.

The accessible description identifies the latest completed producer window. With output stopped the previous value remains; there is no fresh-device-time claim, animated decay, peak hold or latched clip alarm. Only the latest window is retained, so GUI stalls can miss intervening peaks or invalid windows. The bar is linear amplitude, not a calibrated dB scale. Headless tests cover real post-fader sample values, silence/pause, invalid/full-scale captions and mixer bounds at850x740/1280x860. Native contrast/readability, screen-reader behavior and physical audio output remain open.


## Lazy library list rows (2026-10-27)

The existing ListView now obtains LibraryRow data on demand from an ordered stable-ID model instead of eagerly allocating all row strings on publication. Headless tests cover20,000 tracks, viewport requests, search/sort, catalog replacement and safe controller lifetime. Sorting/filtering still process the full in-memory catalog and the model rebuilds its ID list. Headless viewport and allocation evidence is not native scrolling/rendering, accessibility-reader, visual or platform acceptance.


## 100,000-track headless library workflow (2026-10-27)

A test at the existing 100,000-track limit exercises the library model and accessible search/sort/selection callbacks; two model-row requests were observed before explicit first/last lookups in one standalone run. Paths and metadata are synthetic. This does not constitute native visual/scrolling/screen-reader acceptance or establish responsiveness with real media and device loads. Indexed/paged search remains open.

## Cue quantization step checkpoint (2026-10-29)

The CUES tools panel adds a per-deck quantization selector for 1/8, 1/4, 1/2, 1, 2 or 4 beats. The existing KEYLOCK/trigger row remains compact; trigger text is Next grid and its accessible description explains scheduled recall. QUANT uses the selected nearest step for new slots; Next grid uses the next selected step for occupied slots and preserves their saved timestamp. Changing the bound step cancels a queued trigger. Default is one beat; no new global toggle or persistent preference is implied. Headless property/engine and two-size geometry evidence is separate from native dropdown/focus/usability acceptance.

## Persistent Quantize settings (2026-10-29)

Global QUANTIZE and the per-deck cue snap/Next grid/Auto Loop snap/step settings now restore from SQLite before preference controls become editable. Existing libraries with no settings keep the previous defaults. Controls remain available for session use if storage startup fails; save failures use the existing Retry indication. Restoring settings does not start playback, load tracks or restore an armed cue trigger. Headless restart/error/close tests cover this scope; native interaction and general device/performance preference parity remain open.

## Audio output pair selector (2026-10-29)

The global toolbar now exposes Audio output pair with the complete stereo pairs offered by the current stream. A mono/stereo-only output has one disabled option; a four-channel output can switch between OUT 1/2 and OUT 3/4. The selected destination receives the common post-fader mix. Selection is session-local and does not imply an independent deck cue/headphone mix or device-selection interface. Headless tests cover pair selection, invalid values, render-frame preservation, stereo samples and toolbar bounds at both supported sizes. Native listening evidence remains explicitly separate.

## Output profile restore/status (2026-10-29)

Audio output pair is restored from the current library database after guarded device matching. It remains disabled until preference restore finishes. An adjacent Audio output profile label keeps session-only or save/restore status visible independently of library status messages. The device binding uses unique backend/display-name/channel shape, not a hardware serial number. Full hardware-ID matching and device selection remain open. Restoring routes never resumes transport or loads tracks.

## MC4000 startup and running illumination (2026-10-29)

The five-round 5.2-second startup choreography now ends with the documented All LEDs On command. LEDs then stay illuminated for the connection lifetime; no periodic off or Keylock-state LED messages follow. Keylock controls and GUI state remain functional, but its LED no longer indicates enabled/disabled state in this illumination mode. Normal controller release/application shutdown requests All LEDs Off; process crashes or a failed/disconnected MIDI output cannot guarantee a final hardware reset. Reconnection creates a fresh startup sequence. Automated tests cover the emitted schedule and late polls; no new physical LED or unplug/replug acceptance is claimed.

## Deck meter row (2026-10-29)

An always-visible row beside the existing mixer area shows independent A/B post-DSP pre-fader sample peaks and per-deck CLIP/CLEAR buttons. The latest 50-ms producer window is displayed in dBFS; LAST explicitly marks inactive transport. CLIP remains through quiet windows and track/source changes until its own button is pressed. Reset does not alter mixer settings, transport or the other deck. Accessible labels identify both meter images and clear actions; headless bounds are checked at 850x740 and 1280x860. These are peak/invalid-sample warnings, not RMS/VU, true peak, physical-device meters or native visual acceptance.

The shared waveform region uses fixed min/preferred/max layout bounds, with detail canvases allowed to shrink from 54 to 36 px in constrained views. This leaves room for the meter row while retaining library/status/cancel controls at the supported small size, without a window-height-dependent binding loop.

## Delay FX overlay and MC4000 controls (implemented, tests pending 2026-10-29)

FX opens a closable floating A/B delay panel, leaving the underlying workspace layout dimensions unchanged. Each deck has Enabled/bypass, milliseconds, feedback percent, Dry/Wet percent and a 1-8 preset selector with explicit Load/Save. Presets share one bank; empty slots report an error instead of changing parameters. The header reports FX ready/changed/saved/not saved or session-only. Invalid values restore the prior valid settings. Native layout/input validation is deferred; compilation alone is not UI acceptance.

Manufacturer MIDI addresses come from MC4000-User-Guide-v1.2_00.pdf, MIDI Commands pp. 30-31 (https://cdn.inmusicbrands.com/denondj/MC4000/MC4000-User-Guide-v1.2_00.pdf). Zero-based channels 8/9 map to the two FX banks. Note 0 toggles the single delay; CC0/1/2 map to wet, feedback and milliseconds. CC3 is interpreted as signed relative movement and adjusts time in 10-ms increments. These software parameter assignments do not claim the manufacturer's three separate effect slots or Beat Sync/Tap behavior. Six absolute knob slots coalesce before GUI dispatch; relative steps/buttons retain the bounded queue and overflow report. Shift/release addresses are ignored. All-on controller illumination keeps priority over per-effect LED state feedback. Physical movement, relative magnitude interpretation and musical response await tomorrow's test.

## Paused in-process device change (2026-10-30)

AUDIO / APPLY NOW opens the selected device within the running application. Both decks must be paused, loading/tempo work finished and deck editors closed. Pending settings are flushed first; failed saves or device opens preserve the existing engine. Loaded prepared PCM is shared into the replacement engine, preserving source position, pitch/Keylock preparation, grid, loop markers/activation, faders, EQ and delay parameters. Transport remains paused. The target device restores its own matching output/monitor profile, or safe defaults with visible status. SAVE FOR NEXT START remains a separate action.

Opening occurs on the GUI/control thread and can temporarily stall interaction; this is not a seamless playing-device change. Delay histories, meters and render clocks restart. Timestamped MIDI button events queued during opening are discarded to prevent delayed transport actions. Name-based matching, default-backend enumeration, exclusive-driver opening failure, hotplug and physical/native listening acceptance remain limitations. The old stream is released after successful ownership replacement; failure keeps it available.

## Windows endpoint-bound output profiles (2026-10-30)

Output and monitor profile keys now optionally contain the opaque Windows IMMDevice endpoint ID, scoped by backend and channel shape. A display-name change alone does not change the matching endpoint profile; another endpoint with the same name cannot inherit it. This is an OS endpoint identity, not a hardware serial number or a guarantee across driver reinstall/USB topology changes. Implementation reference: https://learn.microsoft.com/en-us/windows/win32/api/mmdeviceapi/nf-mmdeviceapi-immdevice-getid.

CPAL 0.16 does not expose its endpoint ID publicly. A control-thread WASAPI adapter queries active render endpoints and maps CPAL's same FriendlyName property only when both enumerations are unambiguous. A failed/missing/ambiguous identity leaves WASAPI profiles session-only. The Windows dependency reuses the already locked 0.54 version. No callback work or device-ID parsing is added. Non-Windows backends retain their existing guarded names.

Legacy version-1 records without an ID remain readable and preserved; they match legacy keys only. First opening an ID-bound endpoint uses safe defaults instead of guessing legacy ownership, with an explicit warning when a corresponding legacy name profile is present. User changes create/update the separate endpoint profile; renames update its descriptive name without consuming another slot. Startup preferences now persist the Windows endpoint ID and resolve that exact endpoint across friendly-name changes. Because CPAL 0.16 exposes no public ID-based device constructor, startup maps the matched ID to a uniquely reported CPAL friendly name; duplicate/ambiguous names are refused and visibly fall back to system default. Legacy name-only startup preferences remain readable, while new WASAPI name-only saves are rejected. No native rename, unplug/replug, driver reinstall, listening or longevity acceptance is implied.

## MC4000 MIDI hotplug and LED output recovery (2026-10-30)

The all-on policy described in earlier checkpoints is superseded by the state-feedback slice below (2026-10-31).

The existing two-second MIDI discovery now requires one uniquely named MC4000 input port; duplicate matches fail closed. Missing ports or discovery errors release the input connection and its queued commands, with a persistent controller status prefixed to the existing application-status row (full text exposed accessibly). Reconnection obtains fresh queues and a new greeting, without replaying old actions. This is polling and can miss a complete unplug/replug cycle shorter than its interval; it is not OS-notification detection or MIDI hardware-ID matching.

LED output is independently checked on each discovery interval. Input remains usable when initial LED opening fails. Missing/ambiguous outputs are released, failed opens are retried and a successful new output restarts the five-round greeting followed by the user's requested all-on illumination. Existing outputs are retained while present. No transport-dependent LED-state mode is introduced because it would contradict the current permanent-all-on preference. Failed sends discard the LED output for the next retry. Repeated failures remain visible instead of silently disabling LED recovery permanently.

This slice addresses MIDI/LED connection lifecycle only. Unplugging the combined MC4000 also removes its audio endpoint; rebuilding the audio stream and restoring audio after physical reconnection is a separate open gate. No automatic playback start or restoration is added. Remaining controller controls, full transport Cue press/release semantics and complete hardware workflow acceptance remain under the existing open plan item.

## MC4000 MainView LED feedback (2026-10-31)

The five-round greeting remains, but now ends with All LEDs Off and a complete state frame instead of permanent all-on illumination. After each 33-ms controller tick refreshes MainView, the MIDI output reads those same Slint properties: PLAY follows playing, transport CUE indicates loaded/paused, Keylock follows its GUI state, headphone CUE follows the actual MON selection, and FX1 follows the single delay toggle. Four physical hot-cue pads per deck show occupied slots in blue; arbitrary GUI cue colors, additional FX slots, sampler and other unmapped lamps are not implemented.

Historical initial policy: the seven-segment meters mirrored MainView pre-fader peaks and clip holds. S10 supersedes that policy with contextual pre-fader/master selection and producer-liveness gating as described above; the manufacturer CC68/69 addresses and segment ranges remain unchanged. MainView's own PRE LAST and clip displays retain their existing behavior.

Only changed three-byte messages are sent (at most 22 per feedback frame, including the two Vinyl LEDs); all output work is on the GUI control plane, never the real-time audio callback. Failed sends release the output, report a visible retry status and do not acknowledge the state frame. Output recovery repeats the greeting and publishes a fresh full frame. Tests cover exact manufacturer ranges, independent A/B state, unchanged-tick coalescing, partial failure/replay and real engine/GUI/MIDI transport. Native LED timing, appearance and listening acceptance remain open: a shared tick is not a guarantee of simultaneous physical MIDI/display presentation.

RECONNECT MIDI provides explicit recovery if a same-name device disappears and returns between polling ticks: before further MIDI dispatch the application drops both old connections and queued inputs, forces discovery and opens fresh handles. The accessible footer action remains available even if no controller is detected. It does not rebuild the audio stream or resume tracks. Automatic detection of sub-two-second USB cycles remains open.

## Training and MC4000 deck presentation (2026-11-01)

The existing A/B deck components identify their presentation as TRAINING or MC4000. Connecting the supported MC4000 MIDI input automatically selects round virtual decks. The top toolbar's **Ansicht** selector switches between Training and Virtuelle Decks while connected. Disconnect always shows Training; reconnect defaults to virtual decks again. The selector is disabled without the controller. View choice is session-local and changes no engine state or playback callbacks.

Each virtual deck has a 120px round dark rim and light face, current BPM (prefixed with ~ for estimates), requested pitch and elapsed/remaining source times, following the provided reference's visual structure. Grid-based current BPM uses the local tempo/warp segment at the software playhead, multiplied by effective speed; unknown BPM stays unavailable. Beside it are the full source-time/BPM/pitch captions and software-progress strip. Both layouts retain the same transport, cue, loop, pitch, Sync, Keylock and drag/drop callbacks. The virtual layout uses taller deck cards and a smaller library minimum at constrained window sizes. No DSP or real-time audio callback changes are introduced; bounded local-BPM calculations run on the GUI control plane only.

Headless tests cover automatic connection/disconnection defaults, manual view choice, round geometry at 850x740, 1250x768 and 1280x860, and engine-driven seek/pitch/play/eject state. Native visual/typography/hardware acceptance remains open. Relative jog input now drives the source-time marker as described below; the controller does not provide an absolute platter angle.

## Audio stream fault handling and safe manual reopening (2026-10-30)

The Rodio error callback now only publishes a sticky atomic fault flag. On the following controller tick both decks pause; active Sync, nudge and scheduled cues are cancelled. The existing persistent audio warning directs the user to AUDIO / REOPEN CURRENT. Engine PLAY refuses a failed stream, including hardware/GUI paths. No stream enumeration, logging, allocation or lock is added to this error callback.

REOPEN CURRENT uses the current guarded device identity and existing save-before-open/paused-state transfer transaction, independently of the device dropdown. It validates the newly opened backend/endpoint/channel shape before copying prepared tracks. Opening failure, missing identity, identity/shape mismatch or a candidate already reporting an error retain the old engine and data; an explicit output selection remains available for intentional changes. Success clears the failure warning, uses the matching output/monitor profile and keeps both decks paused. Delayed MIDI buttons during opening use the existing timestamp filter. Device renames that prevent named resolution require explicit selection; startup/direct ID opening remains a separate open item.

This is explicit manual audio recovery after a reported stream failure, not automatic USB audio reconnection. CPAL must report an error for the pause/warning path; physical removal/error timing, reopened audio and headphones require native qualification. Mixer delay histories/meters/render clocks reset as in existing device replacement. No automatic playback restart is introduced.

## Drag library tracks into either deck (2026-10-30)

Each available library row is now a copy-only Slint DragArea carrying a file-path DataTransfer; normal click/keyboard selection remains available. Both full deck surfaces are DropAreas with a hover border. Drop acceptance requires one absolute supported audio path, a paused target, restored storage, and no pending track/import/device operation. The target is checked again at drop time. Decode, path canonicalization and metadata extraction use the existing cancellable worker; the picker is not opened, and loaded tracks remain paused. Worker validation errors retain the previous loaded deck through the existing completion path. Multiple files and text payloads are rejected; no source file or library row is moved or deleted.

Headless pointer press/move/release tests exercise actual library-to-A/B drops at 850x740 and 1280x860. Callback tests cover invalid deck, text/multiple-file payloads, pending load, playing-target rejection and loading the other paused deck while A plays. These tests do not claim native Windows mouse/visual or listening acceptance.

Slint's DragArea/DropArea mechanism is used for within-window dragging; a click alone retains existing selection behavior. External Windows file drops use the application bridge described below, not an upstream native DropArea implementation.

## External Windows file-manager drops (2026-10-30)

The Windows entry point explicitly selects the existing Winit backend and installs a window-local file-event bridge when real audio is enabled. This uses Slint 1.18.1's opt-in `unstable-winit-030` API; no framework upgrade, new crate or audio-render-path change is introduced. One absolute supported audio path is passed unchanged through the existing `drop-track` callback and cancellable background loader. The controller rechecks storage readiness, pending load/import/device work and the target's playing state at drop time. Successful loads remain paused; files are never moved or rewritten.

Winit reports each hovered/dropped file separately. A bounded gesture state retains only the first path and a count, rejecting every member of a multi-file drop, including duplicates. Cancellation clears the state; missing or mismatched hover events are rejected visibly. Actual drop coordinates come from the current Windows screen cursor and native client origin, divided by Slint's effective scale (including `SLINT_SCALE_FACTOR` overrides), not necessarily the native DPI scale. Slint computes the deck hit rectangles from actual absolute layout geometry, not fixed pixels or window halves. AUDIO/MON/FX panels block external targeting. A 33 ms UI timer refreshes the accepted deck border while hovering; leaving, invalid payloads or changing controller eligibility clear it.

Winit's Windows OLE implementation chooses the system copy cursor before application acceptance and exposes no per-target effect negotiation here. The copy cursor therefore does not promise that DrDJ will accept the drop; deck highlight and application status reflect actual eligibility. On 2026-10-30 the user confirmed the supplied native Explorer test protocol worked without errors, closing this bounded interaction acceptance. Exact window dimensions and DPI values were not separately reported; broader multi-monitor/configuration and application-wide visual qualification remain open. This user evidence is distinct from headless geometry/gesture/controller regressions and the inconclusive native automation attempt.

## Opt-in library performance recording (2026-10-30)

With an absolute, new `DRDJ_LIBRARY_PERF_PATH`, the existing status line gains `PERF <seconds>/120s Q<count>/10 S<count>/10 P<count>/10`, updated at most once per second. These are qualifying completed search/sort/page actions, not a PASS indicator. No additional panel, changed minimum geometry or normal-mode status change is introduced. Measurement uses actual controller handlers, accepted revision-bound model installation and Slint's AfterRendering notifier, before display presentation. A renderer without notifier support explicitly refuses measurement; there is no success-shaped model-only fallback.

Two-deck time and action eligibility use observed playing state, at least 3,000 entries in the searched all-tracks/crate scope and recent progress of the existing render-bus frame counter. A small crate within a large catalog cannot qualify as a large search workload. This reads existing control-plane observations; it adds no audio callback instrumentation, locks, logging or file access. It does not certify musical load, physical output, audio latency, dropouts, exact on-screen presentation or 100,000-track native capacity. See LIBRARY_STORAGE.md for fixed engineering budgets, JSON outcomes and the repeatable user protocol. Native qualification remains pending.

## Recursive library folder import (2026-10-30)

The library toolbar keeps **ADD FILES** and adds **ADD FOLDER**. Folder selection recursively discovers supported audio in nested directories and hands it to the existing cancellable library worker for canonicalization, metadata inspection, duplicate handling, and active-crate assignment. Unsupported/unreadable entries are reported as skipped; symlink directories are not followed. A scan is bounded to 100,000 directory entries and 5,000 supported audio files, with explicit failure rather than a partial import on either limit. The directory picker itself is native; headless tests exercise nested discovery, filtering, limits, and the controller callback at 850x740 and 1280x740.

## Controller-mode software playhead readout

The software-progress strip now sits beside the round virtual platter in the virtual view. It uses the deck's real software position, multiline source-time text, BPM and pitch captions, and playing state. Its progress bar is software playback position, not a rotating platter or jog input. Training retains its original overview and time row.

Headless layout/data-binding checks cover 850x740, 1250x768 and 1280x860. Labels use compact 10–11px text; native visual/readability acceptance and full alternate controller layout remain open. Jog integration is described below; hardware qualification is still pending.

## MC4000 relative jog and scratch audio

- Manufacturer mapping (MC4000 manual pp. 30–31): deck channels 1/2, CC6 forward 1–63 and reverse 127–64 (signed relative units); Note6 touch/release, Note7 toggles Vinyl. Vinyl starts enabled; its LEDs now join the existing feedback lane.
- With Vinyl enabled, touch suspends forward transport. Turning moves the shared PCM playhead in either direction and previews audio even when paused. Release retains the original play/pause intent and base pitch. Scratch uses original PCM, temporarily bypassing Keylock; release restores the existing Keylock renderer. Manual touch stops Sync and exits an active loop, retaining saved loop bounds.
- Without touch, or with Vinyl disabled, a playing deck uses the existing ±2% pitch bend, released after 120 ms without jog input. A paused deck is positioned silently. A/B are independent. Transport/source changes cancel scratch; release, MIDI reconnect, disconnect and queue overflow clear held controls without starting a paused deck.
- Sensitivity is provisionally **5 ms of source time per relative MIDI unit**; the manufacturer does not specify ticks per revolution here. The virtual marker follows actual source time at a software 33⅓-rpm convention (200 degrees/second), in both directions. This is not an absolute physical angle or motor feedback.
- MIDI uses the existing bounded ordered input queue and 33 ms UI dispatch. The audio renderer consumes atomic/seqlock settings with bounded reads, no locks or heap allocations, and interpolates towards each accumulated target over at most one 33⅓ ms block. Track boundaries clamp movement; a stationary held platter emits silence, not a DC sample. Release commits any remaining relative target, including touch/move/release within one dispatch tick.
- Automated tests cover manufacturer messages, stereo forward/reverse order, sample-rate conversion, Keylock restoration, zero realtime allocations, a measured render-block deadline, actual audio-to-UI marker binding, pause/play preservation, A/B isolation, bend expiry and disconnect/overflow release. Native scratch sound, controller sensitivity, visible smoothness and end-to-end latency still require the real MC4000; headless tests are not listening/haptic acceptance.
- Independent Sol-medium review findings were fixed: overflow advances a monotonic queue timestamp boundary so an old touch cannot rearm scratch after a lost release; quantized cues are rejected while touching, and render-frame cue execution excludes scratch. Dedicated regressions and the final workspace suite pass (427 tests, six ignored); formatting, strict Clippy and release build are green.

## Mouse scratching on virtual decks

Hold the **left mouse button** on a loaded round platter and drag around its center: clockwise moves forwards, counterclockwise backwards. One turn corresponds to 1.8 seconds of source time (the marker's 33⅓-rpm convention). The shortest angular difference crosses the left-side angle seam without a position jump. Inside the central 12px dead zone movement is re-anchored, preventing unstable angular jumps.

During an accepted mouse scratch, `assets/drag-hand-gesture.svg` replaces the standard grab cursor with a pointer-following 24px hand on a light 28px backing for contrast. It disappears on release/cancellation and does not appear for hardware-only touches or rejected mouse holds. The SVG is bundled at build time; no runtime asset loading is required.

Mouse input uses the same held-platter PCM renderer, Sync/loop takeover and play/pause restoration as hardware scratching, including audible preview while paused. It scratches regardless of the controller's Vinyl mode. No new audio-thread algorithm, allocation or lock is introduced; pointer angle calculation runs on the GUI control plane. Existing target interpolation and its timing limitations still apply.

Button release (including outside the platter), pointer cancellation, focus loss, hiding the virtual view, disconnect and transport/source replacement release the mouse session. Right-click cannot start scratching; releasing a secondary button during a left drag cancels safely because Slint releases pointer capture on any button-up. Hardware touch/jog cannot alter an active mouse-owned gesture, and the mouse cannot take over a hardware-owned hold. The hardware Vinyl toggle remains an explicit cancellation. A/B use independent sessions. Tests dispatch actual pointer events into the round GUI component, render audio, verify forward/reverse source positions and the marker, and cover pause/play restoration, focus/view cancellation, secondary-button capture loss followed by release outside, angle wrapping, center re-anchoring and input ownership.

The user confirmed the preceding physical-controller jog integration works (2026-11-02). This does not constitute a measured end-to-end latency qualification or native acceptance of the newly added mouse gesture.
