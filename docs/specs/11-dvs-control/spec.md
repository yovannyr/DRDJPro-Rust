# Feature Specification: DVS (Digital Vinyl System)

**Feature Branch**: `11-dvs-control`  
**Created**: December 15, 2025  
**Status**: In Development  
**Input**: User description: "Add DVS support for vinyl/CDJ control with timecode analysis, scratch detection, and turntable simulation"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Vinyl Turntable Control (Priority: P1)

As a traditional DJ with turntables, I want to control digital tracks using vinyl timecode so I can maintain my vinyl workflow with digital convenience.

**Why this priority**: Core DVS feature - enables vinyl DJs to use digital audio while keeping tactile turntable control.

**Independent Test**: Can be fully tested by playing timecode vinyl and verifying digital track responds to turntable movements.

**Acceptance Scenarios**:

1. **Given** timecode vinyl is playing, **When** turntable spins forward, **Then** digital track plays forward at corresponding speed
2. **Given** turntable is stopped, **When** platter stops, **Then** digital track pauses immediately
3. **Given** turntable spins backward, **When** reverse detected, **Then** digital track plays in reverse
4. **Given** turntable speed changes, **When** pitch adjusted, **Then** digital track speed matches turntable pitch

---

### User Story 2 - Scratch and Trick Detection (Priority: P1)

As a scratch DJ, I want accurate tracking of fast scratches and tricks so my digital tracks respond like vinyl.

**Why this priority**: Essential for scratch DJs - without responsive scratch detection, DVS is unusable for turntablism.

**Independent Test**: Can be tested by performing scratches and verifying audio responds without lag or drift.

**Acceptance Scenarios**:

1. **Given** user performs baby scratch, **When** record moved back-and-forth, **Then** audio scratches responsively
2. **Given** user performs fast cuts, **When** fader cuts executed, **Then** audio cuts precisely without latency
3. **Given** user backspin, **When** record spun backward quickly, **Then** audio rewinds smoothly without dropout
4. **Given** needle skip occurs, **When** needle jumps, **Then** system detects and recovers position

---

### User Story 3 - Timecode Calibration (Priority: P2)

As a DJ setting up DVS, I want automatic and manual calibration so the system tracks accurately regardless of turntable condition or audio interface.

**Why this priority**: Ensures accurate tracking - poor calibration leads to drift and inaccurate scratching.

**Independent Test**: Can be tested by running calibration and verifying tracking accuracy improves.

**Acceptance Scenarios**:

1. **Given** calibration is initiated, **When** running, **Then** system analyzes timecode signal quality
2. **Given** calibration completes, **When** finished, **Then** tracking accuracy score is displayed
3. **Given** tracking drifts during use, **When** detected, **Then** auto-calibration corrects drift automatically
4. **Given** manual calibration needed, **When** adjusted, **Then** user can fine-tune tracking parameters

---

### User Story 4 - Multiple Timecode Format Support (Priority: P2)

As a DJ with existing timecode vinyl/CDs, I want support for multiple timecode formats (Serato, Traktor, Rekordbox) so I can use my existing hardware.

**Why this priority**: Compatibility with existing DJ equipment - users shouldn't need to buy new timecode records.

**Independent Test**: Can be tested by playing different timecode formats and verifying detection and tracking.

**Acceptance Scenarios**:

1. **Given** Serato timecode is playing, **When** detected, **Then** system switches to Serato decoding mode
2. **Given** Traktor timecode is playing, **When** detected, **Then** system switches to Traktor decoding mode
3. **Given** timecode format is ambiguous, **When** signal analyzed, **Then** user is prompted to select format
4. **Given** format is selected, **When** confirmed, **Then** format is saved for future sessions

---

### User Story 5 - Platter Simulation Display (Priority: P3)

As a DJ using CDJs or MIDI controllers with jog wheels, I want visual platter simulation so I can see vinyl-style position indication without physical vinyl.

**Why this priority**: Visual feedback useful for CDJ users and laptop-only setups, helps with position tracking.

**Independent Test**: Can be tested by playing track and verifying visual platter rotates and responds to seek operations.

**Acceptance Scenarios**:

1. **Given** track is playing, **When** position updates, **Then** visual platter rotates at corresponding speed
2. **Given** user scratches, **When** timecode reversed, **Then** platter rotation reverses
3. **Given** cue point is set, **When** marked, **Then** cue marker appears on platter
4. **Given** platter is clicked, **When** touched, **Then** playback can be controlled via mouse drag

---

### Edge Cases

- What happens when timecode signal is noisy or poor quality?
- How does system handle timecode signal dropouts?
- What happens when needle is lifted off vinyl mid-track?
- How are ultra-fast scratches (>10 Hz) handled?
- What happens when turntable motor speed drifts?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST decode timecode audio signals to extract position and velocity
- **FR-002**: System MUST support Serato CV02.5 timecode format
- **FR-003**: System MUST support Traktor MK2/MK3 timecode formats
- **FR-004**: System MUST support Rekordbox Control Vinyl timecode
- **FR-005**: System MUST track vinyl position with <5ms latency
- **FR-006**: System MUST detect and respond to scratches, backspins, and speed changes
- **FR-007**: System MUST implement automatic calibration on startup
- **FR-008**: System MUST allow manual calibration adjustments (gain, offset, phase)
- **FR-009**: System MUST display signal quality meter (0-100%)
- **FR-010**: System MUST display tracking accuracy score
- **FR-011**: System MUST auto-recover from temporary signal loss (<500ms)
- **FR-012**: System MUST support relative mode (scratch like vinyl) and absolute mode (scratch and seek)
- **FR-013**: System MUST provide visual platter simulation with rotation and cue points
- **FR-014**: System MUST support CDJ mode (jog wheel emulation)
- **FR-015**: System MUST track playback direction (forward/reverse/stopped)
- **FR-016**: System MUST calculate and display vinyl RPM
- **FR-017**: System MUST support pitch range adjustment (±8%, ±16%, ±50%)

### Key Entities

- **Timecode Signal**: Audio buffer, sample rate, frequency (1kHz or 1.3kHz), format type
- **DVS Playback State**: Position (seconds), velocity (RPM), direction (forward/reverse/stopped), pitch shift (%)
- **Calibration Data**: Signal gain, phase offset, tracking accuracy score, auto-calibrate enabled
- **Timecode Format**: Format name (Serato/Traktor/Rekordbox), frequency, encoding scheme
- **Platter State**: Rotation angle, RPM, cue points, active scratching

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Tracking latency under 5ms for responsive scratch performance
- **SC-002**: Position accuracy within ±50ms over 5-minute playback
- **SC-003**: Scratch detection responds to movements as fast as 10 Hz
- **SC-004**: Auto-calibration achieves 90%+ tracking accuracy within 5 seconds
- **SC-005**: System maintains sync without drift for 30+ minute continuous playback
- **SC-006**: Signal dropout recovery occurs within 100ms without audible glitch
- **SC-007**: Visual platter updates at 60 FPS for smooth rotation display
- **SC-008**: All three major timecode formats (Serato/Traktor/Rekordbox) are accurately decoded

## Dependencies

- Web Audio API for timecode audio input processing
- FFT analysis for frequency detection
- Audio input device support (sound card/audio interface)
- Real-time audio buffer processing

## Assumptions

- Users have audio interface with line inputs for timecode signal
- Timecode vinyl/CDs are in good condition without excessive wear
- Turntables or CDJs have stable motor speed
- System has sufficient CPU for real-time timecode decoding
- Audio interface has low latency (<10ms)

## Technical Constraints

- Web Audio API input latency adds minimum delay
- FFT processing requires CPU and adds latency
- Browser audio input permissions required
- Timecode signal quality varies with hardware condition
- Real-time processing limited by JavaScript performance

## Out of Scope

- Native audio driver integration (Web Audio API only)
- Support for non-standard timecode formats
- Multi-timecode input (multiple turntables on one PC)
- Timecode signal generation/recording
- DVS audio interface hardware bundling
- MIDI timecode synchronization
- Beat-matched recording from DVS input
- Slip mode with DVS control
