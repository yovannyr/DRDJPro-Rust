# Feature Specification: Mix Recording System

**Feature Branch**: `14-recording-system`  
**Created**: December 15, 2025  
**Status**: In Development  
**Priority**: 🔴 High Priority

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Professional Mix Recording (Priority: P1)

As a DJ performing live or practicing, I want to record my mixes in high quality so I can share them on social media, podcasts, or review my performance.

**Why this priority**: Core value proposition - DJs need to capture their work for promotion and improvement.

**Independent Test**: Can be fully tested by starting recording, mixing tracks, stopping recording, and verifying file is saved.

**Acceptance Scenarios**:

1. **Given** recording is started, **When** audio is playing, **Then** mix is captured in real-time to file
2. **Given** recording format is selected (WAV/MP3/FLAC), **When** recording completes, **Then** file is saved in chosen format
3. **Given** quality is set to high, **When** recording completes, **Then** file has appropriate bitrate (320kbps MP3 or lossless)
4. **Given** recording is saved, **When** completed, **Then** file includes metadata (date, duration, artist name)

---

### User Story 2 - Multi-Source Recording (Priority: P1)

As a DJ, I want to choose what to record (master output, individual decks) so I can capture specific parts of my setup.

**Why this priority**: Flexibility for different recording needs - practice vs. live sets vs. individual deck capture.

**Independent Test**: Can be tested by switching recording sources and verifying correct audio is captured.

**Acceptance Scenarios**:

1. **Given** "Master Output" source selected, **When** recording, **Then** both decks and crossfader output are captured
2. **Given** "Deck A" source selected, **When** recording, **Then** only Deck A audio is captured
3. **Given** "Deck B" source selected, **When** recording, **Then** only Deck B audio is captured
4. **Given** source is changed mid-recording, **When** switched, **Then** error prevents source change during active recording

---

### User Story 3 - Format and Quality Selection (Priority: P1)

As a DJ with different use cases, I want to choose recording format and quality so I can balance file size with audio quality.

**Why this priority**: Different platforms and use cases require different file formats (lossless for archives, MP3 for distribution).

**Independent Test**: Can be tested by recording with different format/quality combinations and verifying file properties.

**Acceptance Scenarios**:

1. **Given** WAV format selected, **When** recording completes, **Then** uncompressed 1411kbps/44.1kHz file is created
2. **Given** FLAC format selected, **When** recording completes, **Then** lossless compressed file (~850kbps) is created
3. **Given** MP3 low quality selected, **When** recording completes, **Then** 128kbps MP3 file is created
4. **Given** MP3 high quality selected, **When** recording completes, **Then** 320kbps MP3 file is created

---

### User Story 4 - Recording Session Management (Priority: P2)

As a DJ with multiple recordings, I want to view, rename, and manage my recordings so I can organize my archive.

**Why this priority**: Library management for recorded content - keeps recordings organized and findable.

**Independent Test**: Can be tested by creating multiple recordings, renaming them, and deleting old ones.

**Acceptance Scenarios**:

1. **Given** recording is saved, **When** viewed in list, **Then** name, duration, size, date, and source are displayed
2. **Given** recording is selected, **When** rename action chosen, **Then** name can be edited and saved
3. **Given** recording is selected, **When** delete action chosen, **Then** confirmation prompt appears before deletion
4. **Given** recordings exist, **When** list is viewed, **Then** total storage used is calculated and displayed

---

### User Story 5 - Auto-Split Long Recordings (Priority: P2)

As a DJ recording long sets, I want automatic file splitting so I don't end up with unwieldy large files.

**Why this priority**: Practical file management - 3-hour sets shouldn't be single 2GB files.

**Independent Test**: Can be tested by enabling auto-split, recording for longer than split duration, and verifying multiple files are created.

**Acceptance Scenarios**:

1. **Given** auto-split is enabled with 30-minute duration, **When** recording exceeds 30 minutes, **Then** new file automatically starts
2. **Given** auto-split is enabled, **When** split occurs, **Then** audio continues seamlessly without gap
3. **Given** auto-split is disabled, **When** recording for any duration, **Then** single file is created
4. **Given** split duration is set, **When** displayed, **Then** countdown shows time until next split

---

### User Story 6 - Pause and Resume Recording (Priority: P2)

As a DJ taking breaks during practice, I want to pause and resume recording so I don't capture dead air.

**Why this priority**: Workflow convenience - avoids editing out gaps later.

**Independent Test**: Can be tested by pausing recording, waiting, resuming, and verifying gap is not included in final file.

**Acceptance Scenarios**:

1. **Given** recording is active, **When** pause button clicked, **Then** recording pauses and elapsed time stops
2. **Given** recording is paused, **When** resume button clicked, **Then** recording continues from same point
3. **Given** recording is paused, **When** status checked, **Then** "Paused" indicator is visible with yellow color
4. **Given** recording is paused for 5+ minutes, **When** resumed, **Then** audio quality remains consistent

---

### Edge Cases

- What happens when disk space runs out during recording?
- How does system handle recording for extremely long durations (8+ hours)?
- What happens when app crashes during active recording?
- How are recordings handled if format changes mid-save?
- What happens when trying to start recording with no audio output?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST record audio in WAV, MP3, and FLAC formats
- **FR-002**: System MUST provide quality settings: Low (128kbps), Medium (256kbps), High (320kbps), Lossless (WAV/FLAC)
- **FR-003**: System MUST support recording from master output, Deck A, or Deck B
- **FR-004**: System MUST display real-time recording duration (hours:minutes:seconds)
- **FR-005**: System MUST calculate and display estimated file size during recording
- **FR-006**: System MUST pause and resume recording without audio gaps
- **FR-007**: System MUST support auto-split recording at configurable intervals (15-120 minutes)
- **FR-008**: System MUST save recordings with metadata: name, duration, file size, date, source
- **FR-009**: System MUST display list of all recordings with sortable columns
- **FR-010**: System MUST allow renaming recordings
- **FR-011**: System MUST allow deleting recordings with confirmation prompt
- **FR-012**: System MUST calculate and display total storage used by recordings
- **FR-013**: System MUST tag audio files with ID3 tags (for MP3) or metadata (for WAV/FLAC)
- **FR-014**: System MUST prevent source changes during active recording
- **FR-015**: System MUST warn when disk space is low (< 500MB remaining)
- **FR-016**: System MUST provide "Stop and Save" button to end recording
- **FR-017**: System MUST allow downloading recordings to local file system

### Key Entities

- **Recording**: ID, name, duration (seconds), file size (MB), date, source (master/deckA/deckB), format (WAV/MP3/FLAC), file path
- **Recording State**: isRecording, isPaused, currentDuration, source, format, quality, autoSplit enabled, autoSplit duration (minutes)
- **Recording Settings**: Default format, default quality, auto-split enabled by default, default auto-split duration, storage location
- **File Metadata**: Artist name, mix title, date recorded, BPM (if applicable), genre, comments

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Recording latency under 100ms from audio output to captured file
- **SC-002**: File size calculations accurate within ±5% of actual file size
- **SC-003**: WAV recordings maintain full 1411kbps/44.1kHz fidelity
- **SC-004**: MP3 320kbps recordings indistinguishable from lossless in blind tests
- **SC-005**: Auto-split creates seamless transitions with <10ms gap between files
- **SC-006**: Pause/resume functionality maintains audio sync with <50ms drift
- **SC-007**: Recording list displays and sorts 1000+ recordings without lag
- **SC-008**: Storage usage calculation updates within 1 second of recording completion

## Dependencies

- Web Audio API MediaRecorder for audio capture
- Browser file system API for saving recordings
- IndexedDB or cloud storage for recording metadata
- ID3 tag library for MP3 metadata embedding

## Assumptions

- Users have sufficient disk space for recordings (1GB+ available)
- Audio quality is suitable for professional distribution
- Users understand file format and quality tradeoffs
- Browser supports MediaRecorder API with desired codecs
- Recordings are for personal/promotional use (copyright compliance)

## Technical Constraints

- MediaRecorder API codec support varies by browser (MP3 may require polyfill)
- Large files (>2GB) may have browser limitations
- Real-time encoding is CPU-intensive for high-quality formats
- WAV file size is 10MB per minute (stereo, 44.1kHz, 16-bit)
- Browser storage limitations may require external storage options

## Out of Scope

- Live streaming (separate Feature 15)
- Cloud automatic upload/sync of recordings
- Audio editing or trimming within app
- Automated mastering or loudness normalization
- Track markers or cue point recording
- Video recording alongside audio
- Multitrack recording (stems/channels separately)
- Professional studio features (SMPTE timecode, jingle insertion)
