# Feature Specification: Performance Pads

**Feature Branch**: `16-performance-pads`  
**Created**: December 15, 2025  
**Status**: In Development  
**Priority**: 🔴 High Priority

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Hot Cue Performance (Priority: P1)

As a performing DJ, I want hot cue pads to instantly jump to preset positions so I can creatively remix and mashup tracks on the fly.

**Why this priority**: Core DJ performance feature - hot cues are fundamental to modern DJing.

**Independent Test**: Can be fully tested by setting hot cue, jumping to it, and verifying playback position changes instantly.

**Acceptance Scenarios**:

1. **Given** hot cue mode is active, **When** pad pressed at desired position, **Then** hot cue is set with timestamp
2. **Given** hot cue is set, **When** pad pressed again, **Then** playback instantly jumps to saved position
3. **Given** hot cue exists, **When** right-clicked, **Then** hot cue is cleared and pad becomes empty
4. **Given** multiple hot cues set, **When** any pressed during playback, **Then** instant jump occurs without audio glitch

---

### User Story 2 - Roll Loop Effects (Priority: P1)

As a DJ creating build-ups, I want roll pads to create instant looping effects so I can add excitement to transitions.

**Why this priority**: Popular performance technique for energy builds and breakdowns.

**Independent Test**: Can be tested by holding roll pad and verifying looping audio effect.

**Acceptance Scenarios**:

1. **Given** roll mode active, **When** pad held down, **Then** audio loops at specified length (1/16, 1/8, 1/4, 1/2, 1, 2, 4, 8 bars)
2. **Given** roll is active, **When** pad released, **Then** playback continues from where it would have been without roll
3. **Given** different roll length pad pressed, **When** switched, **Then** loop length changes instantly
4. **Given** roll is active during transition, **When** used, **Then** creates seamless effect without tempo mismatch

---

### User Story 3 - Beat Slicer Performance (Priority: P2)

As a creative DJ, I want slicer pads to trigger beat-sliced sections so I can create stutter effects and rhythmic variations.

**Why this priority**: Advanced performance technique for unique live remixing.

**Independent Test**: Can be tested by activating slicer mode and triggering different slices.

**Acceptance Scenarios**:

1. **Given** slicer mode active, **When** enabled, **Then** current bar is divided into 8 slices
2. **Given** slices are active, **When** pad 1-8 pressed, **Then** corresponding slice plays
3. **Given** slice is playing, **When** different pad pressed, **Then** playback switches to new slice instantly
4. **Given** slices loop, **When** pad held, **Then** slice repeats until released

---

### User Story 4 - Sampler Pad Triggering (Priority: P2)

As a DJ with sound effects and samples, I want sampler pads to trigger loaded samples so I can add vocal drops and effects.

**Why this priority**: Adds creative flexibility - samples enhance performances with drops, effects, vocals.

**Independent Test**: Can be tested by loading samples, triggering pads, and verifying samples play.

**Acceptance Scenarios**:

1. **Given** sampler mode active, **When** pad pressed, **Then** assigned sample plays from beginning
2. **Given** sample is playing, **When** same pad pressed again, **Then** sample restarts from beginning
3. **Given** multiple pads triggered, **When** pressed simultaneously, **Then** samples play polyphonically
4. **Given** sample is one-shot, **When** triggered, **Then** plays to completion without looping

---

### User Story 5 - Beat Jump Navigation (Priority: P2)

As a DJ navigating tracks, I want beat jump pads to skip forward or backward by beat counts so I can quickly move through tracks without breaking timing.

**Why this priority**: Workflow enhancement - faster than scrubbing for beat-accurate navigation.

**Independent Test**: Can be tested by pressing beat jump pads and verifying position changes by exact beat count.

**Acceptance Scenarios**:

1. **Given** beat jump mode active, **When** "+4 beats" pad pressed, **Then** playback jumps forward exactly 4 beats
2. **Given** beat jump mode active, **When** "-8 beats" pad pressed, **Then** playback jumps backward exactly 8 beats
3. **Given** beat jump is used, **When** executed, **Then** tempo and phase stay perfectly in sync
4. **Given** beat jump exceeds track length, **When** pressed, **Then** jump is clamped to track boundaries

---

### User Story 6 - Saved Loop Recall (Priority: P2)

As a DJ preparing tracks, I want saved loop pads to instantly recall preset loops so I can trigger prepared sections during performance.

**Why this priority**: Preparation tool - allows DJs to mark key sections for live performance.

**Independent Test**: Can be tested by saving loops, recalling them from pads, and verifying correct loops activate.

**Acceptance Scenarios**:

1. **Given** loop is set and saved, **When** saved loop pad pressed, **Then** loop activates at saved position and length
2. **Given** saved loop is active, **When** pad pressed again, **Then** loop deactivates and normal playback resumes
3. **Given** track has multiple saved loops, **When** different pads pressed, **Then** each recalls its specific loop
4. **Given** saved loops exist, **When** track reloaded, **Then** saved loops persist across sessions

---

### Edge Cases

- What happens when hot cue is set while track is playing vs. paused?
- How does system handle rapid pad triggering (>10 presses/second)?
- What happens when roll is activated near track ending?
- How are pads handled when switching between decks?
- What happens when MIDI controller pads are mapped to performance pads?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide 8 performance pads per deck
- **FR-002**: System MUST support 6 pad modes: Hot Cue, Roll, Slicer, Sampler, Beat Jump, Saved Loop
- **FR-003**: System MUST allow setting hot cues at any playback position
- **FR-004**: System MUST instantly jump to hot cue position when triggered (<10ms latency)
- **FR-005**: System MUST support right-click or long-press to clear hot cues
- **FR-006**: System MUST provide 8 roll lengths: 1/16, 1/8, 1/4, 1/2, 1, 2, 4, 8 bars
- **FR-007**: System MUST create seamless roll loops without audio clicks
- **FR-008**: System MUST continue playback position correctly when roll is released
- **FR-009**: System MUST divide slicer into 8 equal slices per bar
- **FR-010**: System MUST trigger slicer slices with instant response
- **FR-011**: System MUST support sampler mode with assigned sound effects
- **FR-012**: System MUST provide beat jump distances: -32, -16, -8, -4, +4, +8, +16, +32 beats
- **FR-013**: System MUST maintain beat sync accuracy during beat jumps
- **FR-014**: System MUST save and recall loop positions for saved loop mode
- **FR-015**: System MUST visually indicate active pads with color coding
- **FR-016**: System MUST support velocity-sensitive triggering (if MIDI controller supports)
- **FR-017**: System MUST prevent pad triggering lag during high CPU load

### Key Entities

- **Hot Cue**: Position (seconds), label, color (hex), active (boolean)
- **Pad Mode**: Mode type (hotcue/roll/slicer/sampler/beatjump/saved-loop), active mode
- **Roll Settings**: Length (bars), quantize (boolean), active (boolean)
- **Slicer Settings**: Slice count, slice size, loop mode (boolean)
- **Beat Jump**: Distance (beats), direction (forward/backward)
- **Saved Loop**: Loop start (seconds), loop end (seconds), label, color
- **Performance State**: Current mode, pressed pad index, active effects

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Hot cue triggering latency under 10ms
- **SC-002**: Roll loops create sample-perfect loops without audible clicks
- **SC-003**: Beat jump accuracy within ±5ms of exact beat position
- **SC-004**: Pad visual response time under 16ms (60 FPS)
- **SC-005**: Slicer slices are perfectly quantized to beat grid
- **SC-006**: System handles 10+ pad triggers per second without lag
- **SC-007**: Mode switching completes within 100ms
- **SC-008**: Hot cues persist across app sessions without data loss

## Dependencies

- BPM detection and beat grid (Feature 3, Feature 17)
- Audio buffer manipulation for roll and slicer effects
- Sample management system for sampler mode
- MIDI input system for hardware controller integration (Feature 4)

## Assumptions

- Tracks have accurate beat grids for quantized effects
- Users understand DJ performance pad concepts
- Audio latency is acceptable for live performance (<20ms total system latency)
- Hot cues and saved loops are stored per track
- Roll and slicer effects use buffer manipulation, not audio processing

## Technical Constraints

- Web Audio API buffer manipulation for roll/slicer effects
- JavaScript timing precision affects pad trigger accuracy
- Roll loops require real-time audio buffer copying
- Slicer requires beat grid analysis and slice calculation
- Visual feedback limited by browser rendering frame rate

## Out of Scope

- Custom pad layout or grid size (fixed 8-pad layout)
- Advanced slicer modes (pitched slicer, reverse slicer)
- Pad sensitivity calibration for MIDI controllers
- Sampler editing (pitch, reverse, filters) - samples are played as-is
- Recording pad performances (separate feature)
- Undo/redo for hot cue placement
- Pad performance macros or sequences
- Touch screen multi-touch pad triggering
