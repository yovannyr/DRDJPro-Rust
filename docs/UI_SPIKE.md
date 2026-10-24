# Slint two-deck prototype — current local status

The prototype combines a provisional Slint UI with Rodio audio playback. Default startup (or `--gui`) opens the default output device. `--gui <deck-a.mp3> <deck-b.wav>` preloads two tracks paused; `--demo` explicitly runs simulated transport with synthetic waveform data and no sound device. Native file selection and track preparation use a bounded background worker.

## Implemented and headless-tested

- Independent MP3/WAV loading, Play/Pause, Cue, EOF/restart and percent seeking for A/B.
- Channel A/B levels and a linear crossfader control the actual Rodio sinks.
- Elapsed/total time and position are derived from the engine.
- A static full-track overview per deck uses up to 256 peaks computed from decoded PCM. Each time bin takes the maximum absolute sample across all channels; opposite-phase stereo cannot cancel. The display clamps amplitudes to 0–1 without normalizing tracks. Silence has zero height.
- Bars scale across the available overview width; the playhead moves from the left edge at 0% to the right edge at 100%. Clicking the overview seeks using the whole track's time scale. The existing slider remains an accessible alternative, and the overview itself exposes an accessible percentage value.
- Overview data is shared with the prepared track. Seeking/Cue/timer updates reuse the same GUI model; successful reload replaces only its deck. Error/cancel preserve existing models.
- A second peak-envelope view follows the playhead with independent 2/4/8/16/32-second zoom per deck (8 seconds by default). At the start/end the view is pinned to track boundaries; short tracks fill the whole view. Its click target maps to the visible time range, while the overview still maps to the complete track.
- Detailed peaks are prepared once per 256 PCM frames on the loader. Only the visible envelope is re-binned to at most 256 GUI bars; paused/unchanged viewports reuse their model. Peak boundaries have 256-frame precision (about 5.8 ms at 44.1 kHz); this is an amplitude envelope, not sample-level editing.
- Procedural waveform data is used only by explicit demo mode; detail zoom is disabled there.

## Local acceptance status

| Gate | Status |
| --- | --- |
| Windows formatting, strict Clippy, tests and release build | See latest LOCAL_VALIDATION.md entry |
| Headless controls and actual Rodio mix samples | Automated tests |
| Overview peaks, click/time mapping and marker geometry | Headless tests at 1200 and 850 logical window widths, including local-time seeking and independent zoom bounds |
| Native file dialog and real audible output | Not accepted by these tests |
| Visual/design approval, resizing usability, CPU/GPU rendering and frame pacing | Open |
| macOS build, performance and visual acceptance | Open |

Geometry/event tests do not establish visual quality or rendering performance. Slint remains provisional; migration phase 3 is not complete.

## Explicit remaining scope

Frequency-colored waveform, beatgrid/BPM/key analysis, jog/pitch, EQ/meters, headphone routing, device recovery, library/persistence and controller integration remain open. Click-free seeking and professional real-time qualification also remain open. The library footer is still a status placeholder. Minimum window height is now 680 logical pixels to accommodate the detail view; layout geometry checks are not a visual usability acceptance.

Next scope requires review. Finish the representative waveform/UI evaluation in bounded slices and record visual/rendering evidence before choosing Slint as the final framework. See MIGRATION_PLAN.md, FEATURE_CATALOG.md and AUDIO_PLAYBACK.md.
