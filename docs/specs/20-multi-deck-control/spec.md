# Feature Specification: Multi-Deck Control

**Feature Branch**: `20-multi-deck-control`  
**Created**: December 15, 2025  
**Status**: In Development  
**Priority**: 🟡 Medium Priority

## User Scenarios & Testing *(mandatory)*

### User Story 1 - 4-Deck Simultaneous Mixing (Priority: P1)

As a professional DJ, I want to control 4 decks simultaneously so I can create complex multi-track performances beyond traditional 2-deck mixing.

**Why this priority**: Professional feature - enables advanced mixing techniques and larger sound palettes.

**Independent Test**: Can be fully tested by loading tracks on 4 decks, adjusting levels, and verifying mix output.

**Acceptance Scenarios**:

1. **Given** 4-deck mode is active, **When** all decks loaded and playing, **Then** all 4 tracks are audible in mix
2. **Given** deck faders are adjusted, **When** moved, **Then** corresponding deck volumes change independently
3. **Given** multiple decks are active, **When** mixed, **Then** no audio clipping or distortion occurs
4. **Given** 2-deck mode is selected, **When** switched, **Then** only decks 1-2 are visible and controllable

---

### User Story 2 - Independent Channel EQ Control (Priority: P1)

As a DJ balancing multiple tracks, I want independent 3-band EQ per channel so I can shape each track's frequency balance individually.

**Why this priority**: Essential mixing control - frequency separation prevents muddy multi-deck mixes.

**Independent Test**: Can be tested by adjusting EQ knobs on each channel and verifying only that channel is affected.

**Acceptance Scenarios**:

1. **Given** channel A is selected, **When** high EQ adjusted, **Then** only channel A highs are affected
2. **Given** channel B mid EQ is boosted, **When** adjusted, **Then** only channel B mids increase
3. **Given** channel C low EQ is cut, **When** adjusted, **Then** only channel C bass decreases
4. **Given** all EQs are at center, **When** default state, **Then** all frequencies pass unaffected

---

### User Story 3 - Crossfader Assignment (Priority: P1)

As a DJ routing decks flexibly, I want to assign each deck to crossfader left, center, or right so I can control which decks the crossfader affects.

**Why this priority**: Routing flexibility - allows 2-deck mixing within 4-deck setup or crossfader bypass.

**Independent Test**: Can be tested by assigning decks to different crossfader positions and moving crossfader.

**Acceptance Scenarios**:

1. **Given** deck 1 assigned to left, **When** crossfader moved right, **Then** deck 1 volume fades out
2. **Given** deck 2 assigned to right, **When** crossfader moved left, **Then** deck 2 volume fades out
3. **Given** deck 3 assigned to center, **When** crossfader moved, **Then** deck 3 volume remains constant
4. **Given** all decks assigned to center, **When** crossfader moved, **Then** no volume changes occur

---

### User Story 4 - Independent Gain Staging (Priority: P1)

As a DJ with tracks at different volume levels, I want independent gain control per channel so I can match perceived loudness before mixing.

**Why this priority**: Technical necessity - prevents volume jumps when mixing tracks with different mastering levels.

**Independent Test**: Can be tested by loading tracks with different loudness, adjusting gains, and verifying matched levels.

**Acceptance Scenarios**:

1. **Given** track is quieter than others, **When** gain increased, **Then** track matches others' perceived volume
2. **Given** track is too loud, **When** gain decreased, **Then** track level decreases without affecting others
3. **Given** optimal gain is found, **When** saved, **Then** gain setting persists for that track
4. **Given** gain exceeds safe level, **When** adjusted too high, **Then** warning indicator shows potential clipping

---

### User Story 5 - Channel Mute and Solo (Priority: P2)

As a DJ monitoring complex mixes, I want to mute/solo individual channels so I can isolate tracks for cueing or troubleshooting.

**Why this priority**: Workflow tool - helps with monitoring and preparing transitions in multi-deck context.

**Independent Test**: Can be tested by muting channels and verifying they're silent, soloing channels and verifying only they're audible.

**Acceptance Scenarios**:

1. **Given** channel is muted, **When** mute active, **Then** channel is silent but still playing
2. **Given** channel is soloed, **When** solo active, **Then** only soloed channel(s) are audible
3. **Given** multiple channels soloed, **When** all solo buttons active, **Then** all soloed channels are audible
4. **Given** solo is active, **When** all solos cleared, **Then** all unmuted channels return to audible

---

### User Story 6 - Master Output Control (Priority: P1)

As a DJ managing overall mix volume, I want master volume and headphone cue controls so I can adjust output independently.

**Why this priority**: Essential output control - final volume stage before audio output.

**Independent Test**: Can be tested by adjusting master volume and verifying all channels are affected equally.

**Acceptance Scenarios**:

1. **Given** master volume is adjusted, **When** changed, **Then** all channel outputs scale proportionally
2. **Given** headphone cue is enabled, **When** channel cued, **Then** headphones receive cued channel only
3. **Given** master is at 0%, **When** muted, **Then** no audio reaches master output
4. **Given** master limiter is active, **When** signals clip, **Then** output is safely limited to prevent distortion

---

### Edge Cases

- What happens when switching from 4-deck to 2-deck mode while decks 3-4 are playing?
- How does system handle CPU load with 4 simultaneous effect chains?
- What happens when all channels are muted?
- How is channel state managed when rapidly switching deck assignments?
- What happens when crossfader assignment changes while crossfader is moved?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support 2-deck and 4-deck mixer modes
- **FR-002**: System MUST provide independent volume faders per channel (0-100%)
- **FR-003**: System MUST provide 3-band EQ per channel: High, Mid, Low (0-100%, center = 50%)
- **FR-004**: System MUST provide independent gain control per channel (0-100%)
- **FR-005**: System MUST support crossfader with assignable routing (Left/Center/Right per deck)
- **FR-006**: System MUST provide mute button per channel
- **FR-007**: System MUST provide solo button per channel
- **FR-008**: System MUST provide master volume control (0-100%)
- **FR-009**: System MUST provide headphone cue system with cue/master mix control
- **FR-010**: System MUST display level meters per channel
- **FR-011**: System MUST display master output level meter
- **FR-012**: System MUST prevent audio clipping with master limiter
- **FR-013**: System MUST route deck audio to assigned mixer channels
- **FR-014**: System MUST allow resetting channel to default settings
- **FR-015**: System MUST clear all solo states with "Clear Solos" button
- **FR-016**: System MUST visually indicate active channels (playing, not muted, audible)
- **FR-017**: System MUST persist mixer settings across sessions

### Key Entities

- **Mixer Mode**: 2-deck or 4-deck
- **Mixer Channel**: A, B, C, D with volume, EQ (high/mid/low), gain, muted, solo, level meter
- **Crossfader Assignment**: Left, Center, Right per deck/channel
- **Crossfader State**: Position (0-100%), curve type, enabled
- **Master Section**: Volume (0-100%), limiter enabled, meter data
- **Channel Routing**: Deck number → Mixer channel mapping
- **Headphone Cue**: Cue mix (0-100%, 0=cue only, 100=master only), cued channels

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Mode switching (2-deck ↔ 4-deck) completes within 200ms
- **SC-002**: Fader adjustments apply within 10ms
- **SC-003**: EQ changes apply within 20ms
- **SC-004**: Mute/solo toggles respond within 10ms
- **SC-005**: Level meters update at 30+ FPS
- **SC-006**: 4-deck mixing maintains CPU usage under 30% on mid-range hardware
- **SC-007**: Master limiter prevents clipping with <1ms attack time
- **SC-008**: Crossfader curve feels smooth across entire range (no jumps or steps)

## Dependencies

- Web Audio API for audio routing and mixing
- Audio effects chain for per-channel EQ
- Level metering system
- Deck playback system

## Assumptions

- Users understand professional mixer concepts
- System has sufficient CPU for 4 simultaneous audio streams
- Audio interface supports desired output routing
- Users need flexibility beyond standard 2-deck setup
- EQ and gain adjustments use high-quality DSP

## Technical Constraints

- Web Audio API node limits for complex routing
- CPU usage scales with active channel count
- Real-time EQ processing is CPU-intensive
- Level metering requires frequent analysis (30-60 FPS)
- Crossfader curves require smooth interpolation

## Out of Scope

- Advanced mixer features (filters, send/return effects)
- Individual channel effects (beyond EQ)
- Recording individual channel stems
- MIDI mixer control mapping
- External mixer integration
- Channel grouping or bus routing
- Sidechain compression between channels
- Advanced metering (spectral, stereo width)
