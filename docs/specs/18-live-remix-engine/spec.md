# Feature Specification: Live Remix Engine

**Feature Branch**: `18-live-remix-engine`  
**Created**: December 15, 2025  
**Status**: In Development  
**Priority**: 🟡 Medium Priority

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Audio Slicing and Arrangement (Priority: P1)

As a DJ creating live remixes, I want to slice tracks into sections and rearrange them so I can create unique versions and mashups on the fly.

**Why this priority**: Core remix functionality - enables creative track reconstruction.

**Independent Test**: Can be fully tested by auto-slicing track, rearranging slices, and playing back new arrangement.

**Acceptance Scenarios**:

1. **Given** track is loaded, **When** auto-slice activated, **Then** track is divided into beat-aligned slices
2. **Given** slices are created, **When** slice clicked, **Then** slice is selected and highlighted
3. **Given** slices are selected, **When** reordered, **Then** playback follows new arrangement
4. **Given** arrangement is created, **When** played, **Then** slices play seamlessly without gaps

---

### User Story 2 - Multiple Slicing Modes (Priority: P1)

As a remix artist, I want different slicing modes (beats, bars, phrases, custom) so I can slice tracks according to musical structure.

**Why this priority**: Flexibility for different remix styles and track types.

**Independent Test**: Can be tested by selecting different slice modes and verifying slice boundaries match mode.

**Acceptance Scenarios**:

1. **Given** "Beats" mode selected, **When** auto-slice runs, **Then** track is sliced every N beats (configurable: 4, 8, 16)
2. **Given** "Bars" mode selected, **When** auto-slice runs, **Then** track is sliced at bar boundaries (4, 8, 16 bars)
3. **Given** "Phrases" mode selected, **When** auto-slice runs, **Then** track is sliced at phrase boundaries (typically 16 or 32 bars)
4. **Given** "Custom" mode selected, **When** markers placed, **Then** slices created at exact marker positions

---

### User Story 3 - Remix Pattern Creation (Priority: P2)

As a DJ preparing performances, I want to save slice arrangements as patterns so I can instantly recall remixes during live sets.

**Why this priority**: Performance preparation - enables repeatable creative arrangements.

**Independent Test**: Can be tested by creating arrangement, saving as pattern, loading pattern, and verifying arrangement is restored.

**Acceptance Scenarios**:

1. **Given** slice arrangement is created, **When** "Save Pattern" clicked, **Then** pattern is saved with name
2. **Given** pattern is saved, **When** pattern loaded, **Then** slice arrangement matches saved state
3. **Given** multiple patterns exist, **When** browsed, **Then** all patterns are listed with names and preview info
4. **Given** pattern is modified, **When** saved again, **Then** pattern is updated with changes

---

### User Story 4 - Performance Recording (Priority: P2)

As a live performer, I want to record my slice triggering and effects so I can replay or export my remix performance.

**Why this priority**: Captures creative moments - allows reviewing and sharing performances.

**Independent Test**: Can be tested by recording slice triggers, stopping recording, and playing back performance.

**Acceptance Scenarios**:

1. **Given** "Record" button pressed, **When** slices triggered, **Then** all actions are captured with timestamps
2. **Given** performance is recording, **When** stopped, **Then** performance is saved with duration and action count
3. **Given** performance is saved, **When** played back, **Then** exact slice sequence and timing is replicated
4. **Given** performance is exported, **When** downloaded, **Then** JSON file contains all performance data

---

### User Story 5 - Slice Effect Application (Priority: P2)

As a creative DJ, I want to apply effects to individual slices so I can add variation and texture to remixes.

**Why this priority**: Creative enhancement - makes remixes more dynamic and interesting.

**Independent Test**: Can be tested by selecting slice, applying effect, and verifying effect is heard on that slice only.

**Acceptance Scenarios**:

1. **Given** slice is selected, **When** reverb applied, **Then** only that slice has reverb effect
2. **Given** slice effect is set, **When** slice plays, **Then** effect is applied in real-time
3. **Given** multiple slices have effects, **When** arrangement plays, **Then** effects apply correctly to each slice
4. **Given** slice effect is removed, **When** cleared, **Then** slice returns to dry sound

---

### User Story 6 - Remix Preset Management (Priority: P3)

As a DJ with signature remix styles, I want to save complete remix setups (slices + patterns + effects) as presets so I can quickly set up familiar workflows.

**Why this priority**: Workflow efficiency - speeds up setup for recurring remix styles.

**Independent Test**: Can be tested by creating complete remix setup, saving as preset, loading preset, and verifying all elements are restored.

**Acceptance Scenarios**:

1. **Given** complete remix is created, **When** "Save Preset" clicked, **Then** all slices, patterns, and settings are saved
2. **Given** preset is loaded, **When** applied, **Then** all slices, arrangements, and effects are restored
3. **Given** presets are browsed, **When** viewed, **Then** preset name, description, and track info are shown
4. **Given** preset is shared, **When** exported, **Then** preset file can be imported by other users

---

### Edge Cases

- What happens when slicing track with variable tempo?
- How does system handle slicing tracks without beat grids?
- What happens when arrangement has more slices than original track length?
- How are effects handled when slice is duplicated?
- What happens when importing preset for track with different structure?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST slice tracks into audio segments based on beat grid
- **FR-002**: System MUST provide 4 slicing modes: Beats (4/8/16), Bars (4/8/16), Phrases (16/32), Custom
- **FR-003**: System MUST allow selecting multiple slices simultaneously
- **FR-004**: System MUST support drag-and-drop slice reordering
- **FR-005**: System MUST play arrangements seamlessly without audio gaps
- **FR-006**: System MUST maintain slice timing accuracy within ±5ms
- **FR-007**: System MUST save slice arrangements as named patterns
- **FR-008**: System MUST allow loading and applying saved patterns
- **FR-009**: System MUST record live slice triggering with timestamps
- **FR-010**: System MUST playback recorded performances exactly
- **FR-011**: System MUST apply effects independently to each slice
- **FR-012**: System MUST support slice effects: Reverb, Delay, Filter, Pitch Shift, Reverse
- **FR-013**: System MUST clone, merge, and delete slices
- **FR-014**: System MUST display waveform with slice boundaries
- **FR-015**: System MUST save complete remix setups as presets
- **FR-016**: System MUST export/import remix data as JSON
- **FR-017**: System MUST visualize playback position across slices

### Key Entities

- **Audio Slice**: ID, start time, end time, label, color, effects, gain, looped, waveform data
- **Remix Pattern**: ID, name, slice IDs (ordered array), loop enabled, created date, description
- **Remix Performance**: ID, track ID, track title, start time, actions (timestamp + action type + slice ID), duration
- **Slice Effect**: Type (reverb/delay/filter/pitch/reverse), parameters, enabled, wet/dry mix
- **Remix Preset**: ID, name, description, track ID, slices, patterns, settings, created date
- **Arrangement Mode**: Freeplay, Pattern, Performance, Sequencer

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Auto-slicing completes within 5 seconds for typical 4-minute tracks
- **SC-002**: Slice transitions maintain sample-perfect accuracy (<1ms gaps)
- **SC-003**: Pattern loading restores arrangement within 500ms
- **SC-004**: Performance recording captures actions with <10ms timestamp precision
- **SC-005**: Playback follows arrangement without audible glitches at 60 FPS
- **SC-006**: Effects apply to slices without increasing latency beyond 20ms
- **SC-007**: Preset loading restores complete setup within 2 seconds
- **SC-008**: System handles 100+ slices per track without performance degradation

## Dependencies

- BPM detection and beat grid system (Feature 3, Feature 17)
- Web Audio API for audio buffer manipulation
- Audio effects chain (from existing effects system)
- Waveform visualization system

## Assumptions

- Tracks have accurate beat grids for quantized slicing
- Audio quality is sufficient for buffer manipulation
- Users understand remix/arrangement concepts
- Slices are stored as buffer references, not duplicated audio data
- Real-time slicing uses pre-analyzed beat data

## Technical Constraints

- Web Audio API buffer manipulation limitations
- Memory usage scales with slice count
- Real-time slice switching requires low-latency audio path
- Complex arrangements may require audio buffer pre-loading
- Effect processing is CPU-intensive with many active slices

## Out of Scope

- Stem-based remixing (separate Feature 12)
- AI-generated slice arrangements
- Collaborative remote remixing
- MIDI note-based slice triggering
- Time-stretching individual slices beyond track tempo
- Automatic beat-matching between slices from different tracks
- Visual waveform editing (zoom, splice, fade editing)
- Slice quantization correction (assumes perfect beat grid)
