# Feature Specification: MIDI Controller Support

**Feature Branch**: `4-midi-controller`  
**Created**: December 15, 2025  
**Status**: Completed  
**Input**: User description: "Enable full MIDI controller support with device detection, preset mappings, MIDI learn, and custom mapping editor"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic Device Detection (Priority: P1)

As a DJ plugging in my MIDI controller, I want the application to automatically detect and connect to my device so I can start mixing immediately without manual configuration.

**Why this priority**: Essential first-time experience - users expect plug-and-play functionality like professional DJ software.

**Independent Test**: Can be fully tested by connecting a MIDI device and verifying it appears in the device list within 2 seconds. Delivers immediate value.

**Acceptance Scenarios**:

1. **Given** a MIDI controller is connected, **When** device is detected, **Then** it appears in the device manager list
2. **Given** a supported controller is detected, **When** connection completes, **Then** appropriate preset mapping is applied automatically
3. **Given** an unsupported controller is detected, **When** connection completes, **Then** generic MIDI mapping is applied
4. **Given** controller is disconnected, **When** cable is unplugged, **Then** device is removed from active list and UI shows disconnected state

---

### User Story 2 - Real-Time MIDI Control (Priority: P1)

As a DJ using a MIDI controller, I want all controls (buttons, knobs, faders) to control DJ functions in real-time so I can mix hands-on like with traditional DJ equipment.

**Why this priority**: Core functionality - without responsive controls, the feature is unusable.

**Independent Test**: Can be tested by moving any controller element and verifying corresponding DJ function responds instantly (<50ms latency).

**Acceptance Scenarios**:

1. **Given** controller is connected, **When** play button is pressed, **Then** corresponding deck starts playing immediately
2. **Given** a volume fader is moved, **When** position changes, **Then** deck volume updates in real-time
3. **Given** an EQ knob is turned, **When** rotation detected, **Then** corresponding EQ band adjusts smoothly
4. **Given** multiple controls moved simultaneously, **When** messages received, **Then** all controls respond without lag or conflict

---

### User Story 3 - MIDI Learn for Custom Mapping (Priority: P2)

As a DJ with a custom or unsupported controller, I want MIDI learn functionality so I can map any control to any DJ function without editing configuration files.

**Why this priority**: Enables support for any MIDI device, critical for users with non-standard equipment.

**Independent Test**: Can be tested by clicking "Learn" on any DJ function, moving a controller element, and verifying mapping is created.

**Acceptance Scenarios**:

1. **Given** user clicks "Learn" button for a function, **When** learn mode activates, **Then** UI shows "waiting for MIDI input" state
2. **Given** learn mode is active, **When** user moves a controller element, **Then** mapping is created and learn mode exits
3. **Given** learn mode times out (10 seconds), **When** no input received, **Then** learn mode cancels automatically
4. **Given** mapping is created, **When** controller element is moved, **Then** mapped function responds immediately

---

### User Story 4 - Visual Feedback to Controller (Priority: P2)

As a DJ using a controller with LEDs, I want LED states to reflect application state so I can see which functions are active without looking at the screen.

**Why this priority**: Professional feature that matches behavior of Serato/Rekordbox, enhances usability in dark environments.

**Independent Test**: Can be tested by activating any function in the app and verifying corresponding controller LED lights up.

**Acceptance Scenarios**:

1. **Given** deck is playing, **When** play state changes, **Then** play button LED illuminates on controller
2. **Given** cue point is set, **When** cue button is pressed, **Then** corresponding pad LED lights up
3. **Given** sync is enabled, **When** sync activates, **Then** sync button LED changes color/intensity
4. **Given** effect is active, **When** effect turns on, **Then** effect button LED illuminates

---

### User Story 5 - Multiple Controller Support (Priority: P3)

As a DJ with multiple MIDI devices (controller + keyboard), I want to use multiple devices simultaneously so I can combine equipment for my workflow.

**Why this priority**: Advanced feature for power users, not essential but highly valuable for complex setups.

**Independent Test**: Can be tested by connecting two MIDI devices and verifying both respond independently.

**Acceptance Scenarios**:

1. **Given** two controllers are connected, **When** both devices send input, **Then** both are processed independently
2. **Given** two devices mapped to same function, **When** either sends input, **Then** function responds to both
3. **Given** one device disconnects, **When** cable unplugged, **Then** other device continues functioning normally

---

### Edge Cases

- What happens when controller sends invalid or malformed MIDI messages?
- How does system handle MIDI message flooding (>1000 messages/second)?
- What happens when the same control is mapped to multiple functions?
- How are encoder acceleration curves handled for different controller types?
- What happens when browser denies MIDI access permissions?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST detect all connected MIDI devices automatically on startup
- **FR-002**: System MUST detect MIDI device connect/disconnect in real-time (hot-plug support)
- **FR-003**: System MUST parse MIDI messages (Note On, Note Off, Control Change, Pitch Bend)
- **FR-004**: System MUST support bidirectional MIDI (input and output)
- **FR-005**: System MUST include preset mappings for: Pioneer DDJ-400, Pioneer DDJ-SB3, Numark Party Mix
- **FR-006**: System MUST auto-detect device models and apply appropriate preset mappings
- **FR-007**: System MUST provide generic MIDI mapping as fallback for unsupported devices
- **FR-008**: System MUST implement MIDI learn with one-click assignment
- **FR-009**: System MUST timeout MIDI learn after 10 seconds of inactivity
- **FR-010**: System MUST support custom mapping creation and editing
- **FR-011**: System MUST save custom mappings to persistent storage
- **FR-012**: System MUST allow mapping any MIDI control to any DJ function
- **FR-013**: System MUST support multiple control modes (toggle, momentary, absolute, relative)
- **FR-014**: System MUST support response curves (linear, logarithmic, exponential)
- **FR-015**: System MUST send LED feedback to controllers for active states
- **FR-016**: System MUST show visual activity indicators for incoming MIDI messages
- **FR-017**: System MUST support multiple simultaneous MIDI devices
- **FR-018**: System MUST handle MIDI message rates up to 1000 messages/second per device

### Key Entities

- **MIDI Device**: Device ID, name, manufacturer, input port, output port, connection state
- **MIDI Mapping**: Device ID, control type, channel, note/CC number, mapped function, control mode, response curve
- **Control Mode**: Toggle (on/off state), Momentary (active while held), Absolute (0-127 value), Relative (increment/decrement)
- **Response Curve**: Linear (1:1 mapping), Logarithmic (more precision at low values), Exponential (more precision at high values)
- **Mapped Function**: Target DJ function (play, cue, volume, EQ, effects, etc.), parameter range, default value

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: MIDI devices are detected within 2 seconds of connection
- **SC-002**: MIDI input latency is below 20ms from controller to DJ function response
- **SC-003**: 95% of popular DJ controllers work with preset mappings without customization
- **SC-004**: Users can create custom mappings using MIDI learn in under 5 minutes per control
- **SC-005**: System handles 1000+ MIDI messages per second without lag or dropped messages
- **SC-006**: LED feedback responds within 50ms of application state changes
- **SC-007**: Multiple controllers work simultaneously without conflicts or interference
- **SC-008**: Custom mappings persist across application restarts

## Dependencies

- Web MIDI API browser support
- MIDI device drivers installed on operating system
- User permission to access MIDI devices

## Assumptions

- Users have compatible MIDI controllers with standard MIDI protocol
- Operating system recognizes MIDI devices correctly
- Users understand basic MIDI concepts or can follow tutorials
- Controllers use standard MIDI messages (not proprietary protocols)

## Technical Constraints

- Web MIDI API may not be available in all browsers (Chrome/Edge supported)
- Some controllers use proprietary protocols not accessible via Web MIDI
- MIDI messages are limited to 0-127 value range (7-bit resolution)
- Output timing precision depends on browser and OS MIDI implementation

## Out of Scope

- Support for proprietary controller protocols (e.g., HID mode for Native Instruments)
- MIDI 2.0 protocol support (only MIDI 1.0)
- MIDI clock sync or MIDI timecode
- Macros or multi-function MIDI mapping (one control = one function)
- MIDI routing to external software or DAWs
- Controller firmware updates
- Motorized fader support
- Jog wheel touch sensitivity (capacitive sensors)
