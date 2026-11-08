# Feature Specification: Key Detection & Harmonic Mixing

**Feature Branch**: `6-key-detection`  
**Created**: December 15, 2025  
**Status**: Completed  
**Input**: User description: "Add key detection with Camelot wheel, harmonic mixing assistant, and compatibility checking for seamless musical transitions"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic Key Detection (Priority: P1)

As a DJ analyzing tracks, I want automatic musical key detection so I can identify compatible tracks for harmonic mixing without music theory knowledge.

**Why this priority**: Foundational feature - harmonic mixing impossible without knowing track keys.

**Independent Test**: Can be fully tested by analyzing an audio file and verifying key is detected in standard notation (e.g., "C Major", "8B").

**Acceptance Scenarios**:

1. **Given** an audio file is loaded, **When** key analysis is triggered, **Then** musical key is detected within 15 seconds
2. **Given** key is detected, **When** analysis completes, **Then** key is displayed in Camelot notation (1A-12B)
3. **Given** key is detected, **When** analysis completes, **Then** key is also shown in traditional notation (e.g., "A Minor")
4. **Given** key is detected, **When** result is saved, **Then** key is stored with track metadata

---

### User Story 2 - Visual Camelot Wheel (Priority: P1)

As a DJ learning harmonic mixing, I want an interactive Camelot wheel visualization so I can see key relationships and compatible mixing options at a glance.

**Why this priority**: Essential educational and workflow tool - visualizes complex music theory relationships simply.

**Independent Test**: Can be tested by displaying wheel and verifying all 24 keys are shown in correct positions with color coding.

**Acceptance Scenarios**:

1. **Given** wheel is displayed, **When** rendered, **Then** all 24 keys (1A-12B) are shown in circular layout
2. **Given** a track key is selected, **When** clicked on wheel, **Then** key is highlighted
3. **Given** a key is highlighted, **When** displayed, **Then** compatible keys are visually indicated
4. **Given** user hovers over a key, **When** mouse over, **Then** tooltip shows key name and relationships

---

### User Story 3 - Real-Time Compatibility Checking (Priority: P1)

As a DJ mixing two tracks, I want real-time compatibility checking between deck keys so I know if my mix will sound harmonious.

**Why this priority**: Core value - prevents dissonant mixes, essential for using harmonic mixing effectively.

**Independent Test**: Can be tested by loading tracks with different keys and verifying compatibility indicator shows appropriate rating (perfect/good/fair/poor).

**Acceptance Scenarios**:

1. **Given** both decks have tracks with keys, **When** keys differ, **Then** compatibility rating is displayed (perfect/good/fair/poor)
2. **Given** tracks are in same key, **When** compatibility checked, **Then** rating shows "Perfect" with green indicator
3. **Given** tracks are ±1 Camelot number (adjacent keys), **When** compatibility checked, **Then** rating shows "Perfect" 
4. **Given** tracks are incompatible (e.g., tritone relationship), **When** compatibility checked, **Then** rating shows "Poor" with red indicator

---

### User Story 4 - Mixing Assistant with Recommendations (Priority: P2)

As a DJ selecting my next track, I want mixing recommendations based on current deck's key so I can quickly find compatible tracks from my library.

**Why this priority**: Improves workflow efficiency by suggesting compatible tracks automatically.

**Independent Test**: Can be tested by opening mixing assistant with a track loaded and verifying recommended tracks are all harmonically compatible.

**Acceptance Scenarios**:

1. **Given** deck has track with detected key, **When** mixing assistant opens, **Then** compatible track suggestions are shown
2. **Given** suggestions are displayed, **When** rendered, **Then** tracks are sorted by compatibility score (highest first)
3. **Given** no compatible tracks found, **When** search completes, **Then** message suggests analyzing more tracks or expanding criteria
4. **Given** a suggested track is clicked, **When** selected, **Then** track loads to opposite deck ready to mix

---

### User Story 5 - Confidence Scoring and Manual Override (Priority: P2)

As a DJ reviewing analyzed keys, I want confidence scores and manual override options so I can correct inaccurate detections.

**Why this priority**: Quality control - key detection isn't perfect, users need ability to verify and correct.

**Independent Test**: Can be tested by analyzing tracks with varying clarity and verifying confidence scores correlate with accuracy, plus ability to manually set key.

**Acceptance Scenarios**:

1. **Given** key is detected, **When** analysis completes, **Then** confidence score (0-100%) is displayed
2. **Given** confidence is low (<50%), **When** displayed, **Then** warning indicator suggests manual verification
3. **Given** user wants to override, **When** manual key entry opened, **Then** dropdown shows all 24 keys
4. **Given** manual key is selected, **When** confirmed, **Then** detected key is replaced and confidence set to 100%

---

### Edge Cases

- What happens with atonal or experimental music without clear key?
- How does system handle tracks that modulate between multiple keys?
- What happens with tracks in exotic scales (pentatonic, blues, modes)?
- How are microtonal or non-Western music traditions handled?
- What happens when two tracks share key but have drastically different energy levels?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST analyze audio files to detect musical key
- **FR-002**: System MUST display keys in Camelot notation (1A-12B for minor/major keys)
- **FR-003**: System MUST display keys in traditional notation (e.g., "C Major", "A Minor")
- **FR-004**: System MUST support Open Key notation as alternative display format
- **FR-005**: System MUST calculate confidence scores for key detection (0-100%)
- **FR-006**: System MUST display interactive Camelot wheel with all 24 keys
- **FR-007**: System MUST highlight compatible keys on Camelot wheel when key is selected
- **FR-008**: System MUST provide real-time compatibility checking between deck keys
- **FR-009**: System MUST calculate compatibility ratings: Perfect (same key, ±1 number, relative major/minor), Good (±2 numbers), Fair (±3 numbers, tritone), Poor (all others)
- **FR-010**: System MUST implement mixing assistant with compatible track suggestions
- **FR-011**: System MUST show mixing relationship descriptions (e.g., "Perfect Fifth", "Relative Minor")
- **FR-012**: System MUST allow manual key entry and override of detected keys
- **FR-013**: System MUST use chromagram analysis (12-bin pitch class profiling)
- **FR-014**: System MUST implement Krumhansl-Schmuckler key finding algorithm
- **FR-015**: System MUST detect both major and minor keys
- **FR-016**: System MUST store key data with track metadata
- **FR-017**: System MUST visualize chromagram (pitch class distribution) for advanced users

### Key Entities

- **Key Analysis Result**: Detected key, Camelot notation, traditional notation, Open Key notation, confidence score, energy level
- **Camelot Wheel Position**: Camelot code (1A-12B), traditional key name, wheel position (angle), color coding
- **Compatibility Rating**: Score (0-100), category (perfect/good/fair/poor), relationship description, semitone distance
- **Mixing Recommendation**: Suggested track, compatibility score, key relationship, BPM compatibility
- **Chromagram**: 12-bin array of pitch class energies (C, C#, D, D#, E, F, F#, G, G#, A, A#, B)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Key detection accuracy of 85%+ for tonal music (pop, electronic, rock)
- **SC-002**: Key analysis completes within 15 seconds for typical 4-minute tracks
- **SC-003**: Confidence scores above 70% for 80% of tonal music tracks
- **SC-004**: Camelot wheel renders and is interactive within 500ms
- **SC-005**: Compatibility ratings accurately reflect music theory relationships in 95% of cases
- **SC-006**: Mixing assistant suggests at least 3 compatible tracks when available in library
- **SC-007**: Manual key override applies instantly without re-analysis
- **SC-008**: Users can successfully identify compatible mixes without prior music theory knowledge

## Dependencies

- Web Audio API for frequency analysis
- Audio file decoding capability
- Track metadata storage system

## Assumptions

- Most tracks are in standard Western 12-tone equal temperament
- Tracks maintain consistent key throughout (no modulation)
- Audio quality is sufficient for frequency analysis
- Users' libraries contain primarily tonal music
- Key accuracy is acceptable at 85% (some manual correction expected)

## Technical Constraints

- Analysis runs in JavaScript/TypeScript (Web Audio API)
- FFT processing requires significant CPU for accurate key detection
- Chromagram resolution affects accuracy vs performance trade-off
- Atonal and non-Western music may produce unreliable results
- Key modulation within tracks not detected (only overall key)

## Out of Scope

- Real-time key detection during playback (offline analysis only)
- Detection of key changes/modulation within tracks
- Automatic key transposition or pitch shifting
- Harmonic audio editing or correction
- Support for microtonal or non-12-tone systems
- Machine learning-based key detection
- Chord progression analysis
- Scale mode detection beyond major/minor (Dorian, Phrygian, etc.)
