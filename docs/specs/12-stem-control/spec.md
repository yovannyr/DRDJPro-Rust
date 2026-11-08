# Feature Specification: Stem Separation & Control

**Feature Branch**: `12-stem-control`  
**Created**: December 15, 2025  
**Status**: In Development  
**Input**: User description: "Add AI stem separation to isolate vocals, drums, bass, and other instruments for independent mixing and effects"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Stem Extraction from Tracks (Priority: P1)

As a DJ wanting creative control, I want to separate tracks into individual stems (vocals, drums, bass, other) so I can mix and manipulate each element independently.

**Why this priority**: Core feature that enables all stem-based mixing - without separation, no stem control is possible.

**Independent Test**: Can be fully tested by loading a track, running stem separation, and verifying 4 stems are extracted.

**Acceptance Scenarios**:

1. **Given** track is loaded, **When** stem separation initiated, **Then** analysis extracts vocals, drums, bass, and other stems
2. **Given** separation is running, **When** progress updates, **Then** percentage complete is shown (0-100%)
3. **Given** separation completes, **When** finished, **Then** all 4 stems are playable independently
4. **Given** separation fails, **When** error occurs, **Then** clear error message explains issue

---

### User Story 2 - Independent Stem Volume Control (Priority: P1)

As a DJ with separated stems, I want independent volume faders for each stem so I can create acapellas, instrumentals, or custom mixes.

**Why this priority**: Core mixing value - enables creative stem-based mixing techniques.

**Independent Test**: Can be fully tested by adjusting stem volumes and verifying only target stem volume changes.

**Acceptance Scenarios**:

1. **Given** stems are loaded, **When** vocal fader is lowered, **Then** only vocals become quieter
2. **Given** vocal fader at 0%, **When** muted, **Then** instrumental version plays (instant acapella removal)
3. **Given** all stems except vocals at 0%, **When** adjusted, **Then** acapella version plays
4. **Given** drum fader is increased, **When** boosted, **Then** drums become more prominent in mix

---

### User Story 3 - Stem-Based Effects (Priority: P2)

As a creative DJ, I want to apply effects to individual stems so I can add reverb to vocals only, delay to drums, etc.

**Why this priority**: Advanced creative control - enables professional remix-style effects.

**Independent Test**: Can be tested by applying effect to one stem and verifying other stems are unaffected.

**Acceptance Scenarios**:

1. **Given** vocal stem is selected, **When** reverb applied, **Then** only vocals have reverb, other stems dry
2. **Given** drum stem is selected, **When** delay applied, **Then** only drums have delay effect
3. **Given** multiple stem effects active, **When** playing, **Then** all effects combine naturally
4. **Given** effect is bypassed, **When** disabled, **Then** stem returns to dry signal

---

### User Story 4 - Stem Caching and Management (Priority: P2)

As a DJ working with large libraries, I want separated stems cached so I don't need to re-process tracks every session.

**Why this priority**: Performance and workflow - stem separation is slow, caching saves significant time.

**Independent Test**: Can be tested by separating a track, reloading it, and verifying stems load instantly from cache.

**Acceptance Scenarios**:

1. **Given** track stems are extracted, **When** separation completes, **Then** stems are cached to storage
2. **Given** cached stems exist, **When** track is loaded, **Then** stems load instantly without re-processing
3. **Given** cache is full, **When** storage limit reached, **Then** oldest stems are removed automatically
4. **Given** user clears cache, **When** requested, **Then** all cached stems are deleted

---

### User Story 5 - Real-Time Stem Switching (Priority: P2)

As a performing DJ, I want to switch between stem combinations in real-time so I can create live mashups and remixes.

**Why this priority**: Performance feature that enables live creativity and audience engagement.

**Independent Test**: Can be tested by rapidly toggling stem mutes during playback and verifying smooth transitions.

**Acceptance Scenarios**:

1. **Given** track is playing, **When** vocal mute toggled, **Then** vocals drop out/in smoothly without clicks
2. **Given** multiple stems toggled quickly, **When** rapid changes occur, **Then** no audio artifacts or glitches
3. **Given** preset combinations saved, **When** recalled, **Then** all stem states change simultaneously
4. **Given** stem switching occurs, **When** playing, **Then** timing stays perfectly in sync

---

### Edge Cases

- What happens when stem separation takes extremely long (10+ minutes)?
- How does system handle tracks where stems can't be cleanly separated?
- What happens if storage runs out during stem extraction?
- How are mono tracks handled (separation designed for stereo)?
- What happens when stem quality is poor due to complex mixing?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST separate tracks into 4 stems: Vocals, Drums, Bass, Other (melodic instruments)
- **FR-002**: System MUST provide progress indication during stem extraction (0-100%)
- **FR-003**: System MUST support stem extraction cancellation mid-process
- **FR-004**: System MUST cache separated stems for instant reloading
- **FR-005**: System MUST provide independent volume control per stem (0-100%)
- **FR-006**: System MUST provide mute/solo buttons per stem
- **FR-007**: System MUST apply effects independently to each stem
- **FR-008**: System MUST support stem preset combinations (save/recall)
- **FR-009**: System MUST display waveforms for each individual stem
- **FR-010**: System MUST support headphone cueing individual stems
- **FR-011**: System MUST maintain stem synchronization during playback
- **FR-012**: System MUST handle cache storage limits gracefully
- **FR-013**: System MUST support stem export to individual audio files
- **FR-014**: System MUST provide stem quality indicators
- **FR-015**: System MUST support batch stem extraction for multiple tracks
- **FR-016**: System MUST estimate separation time before starting
- **FR-017**: System MUST use AI/ML-based separation algorithms (Spleeter, Demucs, or similar)

### Key Entities

- **Stem Collection**: Track ID, separation timestamp, cache status, quality score, file paths
- **Stem Type**: Vocals, Drums, Bass, Other
- **Stem Data**: Audio buffer, duration, sample rate, bit depth, file size
- **Stem Control**: Volume (0-100%), muted, soloed, effects chain, pan
- **Separation Progress**: Current step, percentage, estimated time remaining, cancellable
- **Stem Preset**: Preset name, stem mute states, stem volumes, effects settings

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Stem separation completes within 2 minutes for typical 4-minute tracks
- **SC-002**: Vocal isolation achieves 85%+ accuracy (minimal bleed from other instruments)
- **SC-003**: Drum separation achieves 80%+ accuracy with clean kick and snare
- **SC-004**: Cached stems load within 500ms
- **SC-005**: Real-time stem switching occurs within 10ms without audible clicks
- **SC-006**: Stem synchronization remains accurate within ±1ms over full track duration
- **SC-007**: Cache management maintains 100 most recent stem sets without manual intervention
- **SC-008**: Batch processing handles 10+ tracks without crashes or memory issues

## Dependencies

- AI stem separation library (Spleeter, Demucs, or Web Audio API implementation)
- Web Audio API for audio processing and playback
- IndexedDB or local storage for stem caching
- Sufficient CPU/GPU for AI processing

## Assumptions

- Tracks are stereo (separation algorithms work best with stereo)
- System has sufficient storage for cached stems (500MB-1GB)
- CPU/GPU capable of running stem separation (may require web assembly or GPU acceleration)
- Users accept 1-2 minute processing time per track
- Stem quality acceptable at 80-90% separation accuracy

## Technical Constraints

- AI stem separation is computationally expensive (CPU/GPU intensive)
- Processing time scales linearly with track length
- Stem quality depends on track mixing complexity
- Web-based AI models may require large downloads (50-200MB)
- Browser performance varies significantly (Chrome/Edge preferred)
- Storage requirements substantial (4 stems × track length × sample rate)

## Out of Scope

- Custom ML model training (pre-trained models only)
- Real-time stem separation during playback (offline processing only)
- Stem separation for more than 4 categories
- Stem re-synthesis or enhancement
- Automatic stem beat-matching across different tracks
- Stem MIDI note detection
- Lyric transcription from vocal stems
- Stem-to-MIDI conversion
