# Feature Specification: Beat Grid & Sync

**Feature Branch**: `17-beat-grid-sync`  
**Created**: December 15, 2025  
**Status**: In Development  
**Priority**: 🔴 High Priority

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Deck Beat Synchronization (Priority: P1)

As a DJ mixing multiple tracks, I want automatic beat synchronization between decks so transitions are perfectly timed without manual BPM matching.

**Why this priority**: Fundamental modern DJ feature - beat sync enables seamless mixing for all skill levels.

**Independent Test**: Can be fully tested by playing two tracks with different BPMs, enabling sync, and verifying they play in perfect time.

**Acceptance Scenarios**:

1. **Given** Deck A is master at 128 BPM, **When** Deck B syncs, **Then** Deck B adjusts to 128 BPM instantly
2. **Given** both decks are playing, **When** master deck changes BPM, **Then** synced decks follow automatically
3. **Given** tracks have different BPM, **When** sync enabled, **Then** pitch adjustment is applied without audio artifacts
4. **Given** sync is active, **When** disabled, **Then** deck returns to original BPM

---

### User Story 2 - Beat Grid Alignment (Priority: P1)

As a DJ with tracks that have incorrect beat grids, I want to manually adjust beat grid offset and downbeat so sync works perfectly.

**Why this priority**: Automatic BPM detection isn't always perfect - manual correction ensures accurate sync.

**Independent Test**: Can be tested by loading track with incorrect grid, adjusting offset, and verifying beats align correctly.

**Acceptance Scenarios**:

1. **Given** beat grid is misaligned, **When** offset adjusted, **Then** beats visually align with waveform peaks
2. **Given** downbeat is incorrect, **When** "Set 1st" pressed at bar start, **Then** downbeat position is saved
3. **Given** beat grid is adjusted, **When** saved, **Then** adjustments persist for future loads
4. **Given** grid is locked, **When** manual BPM change attempted, **Then** locked grid prevents BPM change

---

### User Story 3 - Phase Alignment (Priority: P1)

As a DJ mixing tracks, I want automatic phase alignment so beats land at exactly the same time for seamless transitions.

**Why this priority**: BPM matching alone isn't enough - phase must align for beats to hit together.

**Independent Test**: Can be tested by syncing two tracks and verifying kick drums hit simultaneously.

**Acceptance Scenarios**:

1. **Given** tracks are BPM synced, **When** phase alignment enabled, **Then** beats hit at same instant
2. **Given** phase drifts during playback, **When** detected, **Then** micro-adjustments keep phase locked
3. **Given** phase is misaligned, **When** visual indicator checked, **Then** phase difference is shown (±180°)
4. **Given** manual phase adjust used, **When** nudged, **Then** visual phase meter shows real-time alignment

---

### User Story 4 - Master Deck Selection (Priority: P1)

As a DJ controlling sync behavior, I want to designate master deck so other decks follow its tempo and phase.

**Why this priority**: Control over which deck leads the sync - critical for intentional BPM changes.

**Independent Test**: Can be tested by setting master deck, changing its BPM, and verifying other decks follow.

**Acceptance Scenarios**:

1. **Given** Deck A set as master, **When** marked, **Then** visual indicator shows "MASTER" badge
2. **Given** Deck A is master, **When** its BPM changed, **Then** Deck B (synced) updates to match
3. **Given** master deck is switched, **When** Deck B becomes master, **Then** Deck A now follows Deck B
4. **Given** no master is set, **When** sync engaged, **Then** first playing deck automatically becomes master

---

### User Story 5 - Quantize to Beat Grid (Priority: P2)

As a DJ cueing tracks, I want quantize enabled so hot cues, loops, and jumps snap to beat boundaries.

**Why this priority**: Ensures all DJ actions stay on beat - prevents off-beat triggering mistakes.

**Independent Test**: Can be tested by enabling quantize, setting hot cue, and verifying it snaps to nearest beat.

**Acceptance Scenarios**:

1. **Given** quantize is enabled, **When** hot cue set, **Then** hot cue snaps to nearest beat
2. **Given** quantize is enabled, **When** loop set, **Then** loop in/out points snap to beats
3. **Given** quantize is enabled, **When** play pressed, **Then** playback starts on next beat
4. **Given** quantize is disabled, **When** actions performed, **Then** exact timing is preserved without snapping

---

### User Story 6 - Beat Grid Visualization (Priority: P2)

As a DJ monitoring beat alignment, I want visual beat indicators (1-2-3-4) so I can see which beat is currently playing.

**Why this priority**: Visual feedback helps with phrasing and timing - especially for learning DJs.

**Independent Test**: Can be tested by playing track and verifying beat indicators update in time with music.

**Acceptance Scenarios**:

1. **Given** track is playing, **When** beat occurs, **Then** corresponding beat number (1-4) is highlighted
2. **Given** beat 1 occurs, **When** downbeat hits, **Then** "1" indicator is emphasized or colored differently
3. **Given** tracks are synced, **When** both playing, **Then** beat indicators show relative phase alignment
4. **Given** beat grid is incorrect, **When** visual checked, **Then** mismatch between audio and indicators is obvious

---

### Edge Cases

- What happens when track has variable BPM (live drumming)?
- How does system handle beat grid when tempo control exceeds ±50%?
- What happens when master deck is stopped?
- How are beat grids handled for tracks with no detectable beats (ambient)?
- What happens when BPM detection is completely wrong?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST synchronize BPM between multiple decks
- **FR-002**: System MUST designate master deck that controls sync tempo
- **FR-003**: System MUST automatically adjust follower deck tempo to match master
- **FR-004**: System MUST align phase so beats hit simultaneously
- **FR-005**: System MUST provide beat grid offset adjustment (±0.5 seconds, 0.01s increments)
- **FR-006**: System MUST allow setting downbeat (bar 1, beat 1) position
- **FR-007**: System MUST lock beat grid to prevent accidental changes
- **FR-008**: System MUST provide quantize toggle for beat-snapped actions
- **FR-009**: System MUST display current beat position (1, 2, 3, 4 of bar)
- **FR-010**: System MUST show beat phase indicator between synced decks
- **FR-011**: System MUST detect and display BPM automatically
- **FR-012**: System MUST allow manual BPM adjustment (0.01 BPM precision)
- **FR-013**: System MUST provide tap tempo for manual BPM determination
- **FR-014**: System MUST save beat grid adjustments per track
- **FR-015**: System MUST display BPM difference between decks
- **FR-016**: System MUST provide "Re-Analyze" function to re-detect BPM
- **FR-017**: System MUST maintain sync accuracy over long playback durations (30+ minutes)

### Key Entities

- **Beat Grid**: BPM, detected BPM, beat offset (seconds), downbeat position (seconds), locked (boolean), analyzed (boolean)
- **Sync State**: Master deck (A/B/none), synced (boolean), sync mode (BPM-only/BPM+phase)
- **Quantize Settings**: Enabled (boolean), quantize size (1/4, 1/8, 1/16 note)
- **Beat Position**: Current beat (1-4), current bar, beat phase (0-1), time to next beat (ms)
- **Phase Alignment**: Phase difference (degrees, -180 to +180), aligned (boolean), drift rate (ms/minute)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: BPM sync completes within 500ms of engaging sync button
- **SC-002**: Phase alignment accurate within ±5ms
- **SC-003**: Sync maintains accuracy within ±10ms over 30-minute playback
- **SC-004**: Beat grid offset adjustment precision of 0.01 seconds
- **SC-005**: BPM difference calculation updates in real-time (every 100ms)
- **SC-006**: Visual beat indicators update with <16ms latency (60 FPS)
- **SC-007**: Quantize snaps actions to beat within ±1ms
- **SC-008**: Tap tempo calculates BPM with ±0.5 BPM accuracy from 4+ taps

## Dependencies

- BPM detection system (Feature 3)
- Audio time stretching for tempo adjustment
- Beat grid storage and retrieval
- Waveform visualization for beat alignment verification

## Assumptions

- Tracks have consistent tempo (no significant tempo drift)
- BPM detection is accurate to within ±2 BPM
- Users understand beat counting and musical bars
- Sync is used for electronic/produced music (not live recordings)
- Web Audio API time stretching is acceptable quality

## Technical Constraints

- Web Audio API time stretching quality degrades above ±20% tempo change
- JavaScript timing precision affects sync accuracy (1-5ms jitter)
- Phase drift accumulation requires periodic re-sync correction
- Beat detection accuracy depends on track quality and mixing
- Visual updates limited by browser rendering rate (60 FPS typical)

## Out of Scope

- Beat detection improvement algorithms (uses existing Feature 3 BPM detection)
- Tempo mapping for variable BPM tracks
- Advanced sync modes (key sync, harmonic sync with pitch adjustment)
- Sync across more than 4 decks
- External MIDI clock sync (master/slave)
- Automatic BPM matching during track loading
- Beat jump sync (jump to next phrase/section)
- AI-powered sync that predicts optimal mix points
