# Feature Specification: BPM Detection

**Feature Branch**: `3-bpm-detection`  
**Created**: December 15, 2025  
**Status**: Completed  
**Input**: User description: "Add professional BPM detection with beat position tracking, confidence scoring, and tap tempo functionality"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Automatic BPM Analysis (Priority: P1)

As a DJ importing new tracks, I want automatic BPM detection so I can immediately know each track's tempo without manual analysis or external tools.

**Why this priority**: Fundamental feature for beat matching - DJs cannot mix effectively without knowing BPM.

**Independent Test**: Can be fully tested by loading an audio file and verifying BPM is detected within reasonable accuracy (±2 BPM). Delivers immediate value.

**Acceptance Scenarios**:

1. **Given** an audio file is loaded, **When** analysis is triggered, **Then** BPM is detected and displayed within 10 seconds
2. **Given** a track with clear beat structure, **When** analysis completes, **Then** confidence score is above 80%
3. **Given** a track without clear rhythm, **When** analysis completes, **Then** confidence score is below 50% and marked as uncertain
4. **Given** BPM is detected, **When** result is saved, **Then** BPM is stored with track metadata

---

### User Story 2 - Beat Position Detection (Priority: P1)

As a DJ using beat-sync features, I want exact beat positions detected so auto-sync and beat grid features work accurately.

**Why this priority**: Required for beat grid visualization, auto-sync, and loop quantization features.

**Independent Test**: Can be tested by analyzing a track and verifying beat markers align with actual beats in waveform.

**Acceptance Scenarios**:

1. **Given** BPM is detected, **When** beat analysis runs, **Then** individual beat positions are identified throughout the track
2. **Given** beat positions are detected, **When** displayed on waveform, **Then** markers align with actual beats
3. **Given** a track with tempo changes, **When** analysis runs, **Then** beat detection adapts to tempo variations

---

### User Story 3 - Batch Analysis for Library (Priority: P2)

As a DJ with a large library, I want to analyze multiple tracks simultaneously so I can process my entire collection efficiently.

**Why this priority**: Saves time for users with large libraries, improves initial setup experience.

**Independent Test**: Can be tested by selecting multiple tracks and verifying all are analyzed in sequence with progress tracking.

**Acceptance Scenarios**:

1. **Given** multiple tracks are selected, **When** batch analysis starts, **Then** tracks are analyzed sequentially
2. **Given** batch analysis is running, **When** progress updates, **Then** current track number and total count are shown
3. **Given** batch analysis encounters an error, **When** a track fails, **Then** analysis continues with remaining tracks
4. **Given** batch analysis completes, **When** finished, **Then** summary shows success/failure counts

---

### User Story 4 - Manual Tap Tempo (Priority: P2)

As a DJ working with tracks that have unclear rhythm or electronic/experimental music, I want to manually set BPM by tapping so I can override automatic detection.

**Why this priority**: Provides fallback for edge cases where automatic detection fails or is inaccurate.

**Independent Test**: Can be tested by tapping spacebar 4+ times at consistent intervals and verifying BPM is calculated correctly.

**Acceptance Scenarios**:

1. **Given** tap tempo mode is active, **When** user taps 4 times at regular intervals, **Then** average BPM is calculated
2. **Given** BPM is calculated from taps, **When** result is stable, **Then** BPM is applied to current track
3. **Given** taps are irregular, **When** intervals vary significantly, **Then** calculation waits for more consistent taps
4. **Given** no taps for 5 seconds, **When** timeout occurs, **Then** tap tempo resets

---

### User Story 5 - Confidence Scoring (Priority: P2)

As a DJ reviewing analyzed tracks, I want to see confidence scores so I can identify tracks that need manual verification.

**Why this priority**: Helps users quickly identify potentially incorrect BPM detections for quality control.

**Independent Test**: Can be tested by analyzing tracks with varying clarity and verifying confidence scores correlate with accuracy.

**Acceptance Scenarios**:

1. **Given** BPM is detected, **When** analysis completes, **Then** confidence score (0-100%) is displayed
2. **Given** confidence is above 80%, **When** displayed, **Then** indicator shows green/high confidence
3. **Given** confidence is below 50%, **When** displayed, **Then** indicator shows red/low confidence warning
4. **Given** low confidence track, **When** user reviews, **Then** manual BPM entry or tap tempo is suggested

---

### Edge Cases

- What happens with variable tempo tracks (classical, jazz)?
- How does system handle tracks without clear beats (ambient, noise)?
- What happens with extremely slow (40 BPM) or fast (200+ BPM) tempos?
- How are half-time and double-time errors corrected?
- What happens with polyrhythmic or complex time signatures?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST analyze audio files to detect beats per minute (BPM)
- **FR-002**: System MUST detect individual beat positions throughout tracks
- **FR-003**: System MUST calculate confidence scores for BPM detection (0-100%)
- **FR-004**: System MUST classify tempo ranges (slow: 60-90, medium: 90-120, fast: 120-160, very fast: 160+)
- **FR-005**: System MUST detect time signatures (4/4, 3/4, 6/8, etc.)
- **FR-006**: System MUST handle tracks from 40 BPM to 200+ BPM
- **FR-007**: System MUST correct half-time and double-time octave errors
- **FR-008**: System MUST provide manual BPM entry option
- **FR-009**: System MUST implement tap tempo functionality (minimum 4 taps)
- **FR-010**: System MUST support batch analysis of multiple tracks
- **FR-011**: System MUST display analysis progress for batch operations
- **FR-012**: System MUST store BPM data with track metadata
- **FR-013**: System MUST allow manual override of detected BPM
- **FR-014**: System MUST calculate beat interval variance for consistency measurement
- **FR-015**: System MUST use onset strength detection for beat finding
- **FR-016**: System MUST apply low-pass filtering (0-200Hz) to focus on kick drums
- **FR-017**: System MUST use histogram clustering to identify dominant tempo

### Key Entities

- **BPM Analysis Result**: BPM value, confidence score, beat count, average interval, variance, tempo classification
- **Beat Position**: Time position in seconds for each detected beat
- **Tempo Classification**: Category (slow/medium/fast/very fast), time signature, tempo stability
- **Analysis Progress**: Current track, total tracks, percentage complete, estimated time remaining
- **Tap Tempo State**: Tap count, tap timestamps, calculated BPM, reset timer

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: BPM detection accuracy within ±2 BPM for 95% of tracks with clear rhythm
- **SC-002**: Analysis completes within 10 seconds for typical 4-minute tracks
- **SC-003**: Confidence scores above 80% for 90% of electronic/pop music tracks
- **SC-004**: Beat positions align with actual beats within ±50ms for 95% of detections
- **SC-005**: Half-time/double-time errors are automatically corrected in 85% of cases
- **SC-006**: Batch analysis processes 100 tracks in under 20 minutes
- **SC-007**: Tap tempo calculates BPM within ±1 BPM of actual tempo when tapped accurately
- **SC-008**: Users can successfully analyze any genre without crashes or hangs

## Dependencies

- Web Audio API for audio processing and decoding
- Audio file access and decoding capability

## Assumptions

- Audio files are in supported formats (MP3, WAV, FLAC, AAC, OGG)
- Most tracks have clear, consistent rhythm (electronic, pop, rock)
- Tracks have sufficient bass frequency content for kick drum detection
- Audio quality is sufficient for frequency analysis
- Users understand some tracks may require manual BPM entry

## Technical Constraints

- Analysis runs in JavaScript/TypeScript (Web Audio API)
- Processing time increases with track length
- Memory usage scales with audio buffer size
- FFT processing requires significant CPU for long tracks
- Real-time analysis during playback is not supported (offline processing only)

## Out of Scope

- Real-time tempo tracking during playback
- Detection of tempo changes within tracks (only average BPM)
- Time-stretching to match detected BPM
- Audio warping or beat correction
- Multi-tempo track support (DJ mixes, medleys)
- Machine learning-based BPM detection
