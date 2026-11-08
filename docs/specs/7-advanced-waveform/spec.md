# Feature Specification: Advanced Waveform Analysis

**Feature Branch**: `7-advanced-waveform`  
**Created**: December 15, 2025  
**Status**: Completed  
**Input**: User description: "Add advanced waveform with spectrogram, frequency band analysis, beat grid overlay, and multiple visualization modes"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Multi-Layer Waveform Display (Priority: P1)

As a DJ analyzing tracks, I want detailed waveform displays showing both peak and RMS values so I can see track dynamics and energy levels at a glance.

**Why this priority**: Core visual representation - DJs rely on waveforms for track navigation and mix preparation.

**Independent Test**: Can be fully tested by loading a track and verifying waveform renders with both peak (outer) and RMS (inner) layers.

**Acceptance Scenarios**:

1. **Given** a track is loaded, **When** waveform generates, **Then** peak waveform (outer layer) and RMS waveform (inner layer) are both displayed
2. **Given** waveform is displayed, **When** track plays, **Then** playhead moves across waveform in real-time
3. **Given** user hovers over waveform, **When** mouse moves, **Then** tooltip shows time position and energy level
4. **Given** user clicks waveform, **When** clicked, **Then** playback seeks to clicked position

---

### User Story 2 - Frequency-Based Color Mapping (Priority: P1)

As a DJ preparing mixes, I want waveforms colored by dominant frequency content so I can identify bass drops, vocals, and high-energy sections visually.

**Why this priority**: Professional feature found in Rekordbox/Serato - helps DJs identify track sections quickly.

**Independent Test**: Can be tested by generating waveform and verifying color changes reflect frequency content (bass=blue, mids=green, highs=red).

**Acceptance Scenarios**:

1. **Given** frequency color mode is enabled, **When** waveform renders, **Then** colors reflect dominant frequency in each segment
2. **Given** bass-heavy section plays, **When** displayed, **Then** waveform shows blue/cool colors
3. **Given** high-frequency section plays, **When** displayed, **Then** waveform shows red/warm colors
4. **Given** color scheme is changed, **When** updated, **Then** waveform re-renders with new colors instantly

---

### User Story 3 - Real-Time Spectrogram Visualization (Priority: P2)

As a DJ analyzing track structure, I want spectrogram view showing time-frequency analysis so I can see exact frequency content over time.

**Why this priority**: Advanced analysis tool for technical DJs, useful for finding specific audio elements.

**Independent Test**: Can be tested by switching to spectrogram view and verifying 2D heatmap shows frequencies (vertical) vs time (horizontal).

**Acceptance Scenarios**:

1. **Given** spectrogram mode is selected, **When** analysis runs, **Then** 2D time-frequency heatmap is displayed
2. **Given** spectrogram is displayed, **When** hovering, **Then** crosshair shows exact time and frequency values
3. **Given** frequency labels are shown, **When** rendered, **Then** axis markers display common frequencies (100Hz, 1kHz, 10kHz)
4. **Given** user adjusts FFT size, **When** changed (1024/2048/4096/8192), **Then** spectrogram resolution updates

---

### User Story 4 - Beat Grid Overlay (Priority: P2)

As a DJ using sync and loops, I want beat grid markers overlaid on waveforms so I can see beat alignment and verify BPM accuracy.

**Why this priority**: Essential for beat-matching features, helps verify BPM detection accuracy.

**Independent Test**: Can be tested by analyzing a track with BPM and verifying beat markers appear at correct intervals on waveform.

**Acceptance Scenarios**:

1. **Given** BPM is detected, **When** beat grid enabled, **Then** beat markers appear on waveform at detected beat positions
2. **Given** beats are marked, **When** displayed, **Then** every 4th beat (downbeat) shows stronger marker
3. **Given** phrase markers enabled, **When** displayed, **Then** 16-beat (4-bar) phrases are highlighted
4. **Given** beat grid is inaccurate, **When** manually adjusted, **Then** markers update in real-time

---

### User Story 5 - Multiple Visualization Modes (Priority: P2)

As a DJ with different workflow preferences, I want multiple waveform styles (overview, zoom, frequency spectrum) so I can choose the view that best suits my current task.

**Why this priority**: Flexibility for different use cases - overview for set preparation, zoom for precise cue points.

**Independent Test**: Can be tested by switching between visualization modes and verifying each shows different level of detail.

**Acceptance Scenarios**:

1. **Given** overview mode is selected, **When** displayed, **Then** entire track waveform fits in view
2. **Given** zoom mode is selected, **When** displayed, **Then** zoomed view shows current playback region with fine detail
3. **Given** spectrum mode is selected, **When** displayed, **Then** real-time frequency spectrum analyzer is shown
4. **Given** mode is changed, **When** switched, **Then** transition occurs smoothly without lag

---

### Edge Cases

- What happens with extremely long tracks (30+ minutes)?
- How does system handle tracks with no audio (silence)?
- What happens when FFT analysis encounters numerical instability?
- How are ultra-low frequencies (<20Hz) and ultra-high frequencies (>20kHz) handled?
- What happens when waveform generation is interrupted?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST generate waveforms showing peak and RMS amplitude values
- **FR-002**: System MUST analyze tracks using Short-Time Fourier Transform (STFT) for spectrograms
- **FR-003**: System MUST calculate frequency band energy across 7 bands: Sub Bass (20-60Hz), Bass (60-250Hz), Low Mid (250-500Hz), Mid (500-2kHz), High Mid (2-4kHz), Presence (4-6kHz), Brilliance (6-20kHz)
- **FR-004**: System MUST color-map waveforms based on dominant frequency content
- **FR-005**: System MUST support multiple color schemes: Default, Frequency, Energy, Rainbow, Classic (Rekordbox-style), Hot, Cool, Grayscale
- **FR-006**: System MUST overlay beat grid markers from BPM detection
- **FR-007**: System MUST display bar markers (every 4 beats) and phrase markers (every 16 beats)
- **FR-008**: System MUST provide configurable waveform resolution (256 to 2048 pixels)
- **FR-009**: System MUST render spectrograms with configurable FFT sizes (1024, 2048, 4096, 8192 samples)
- **FR-010**: System MUST display frequency axis labels on spectrograms (20Hz, 100Hz, 1kHz, 10kHz, 20kHz)
- **FR-011**: System MUST normalize spectrogram amplitude to -90dB to 0dB range
- **FR-012**: System MUST show interactive crosshair on spectrogram hover with time/frequency values
- **FR-013**: System MUST display real-time frequency spectrum during playback
- **FR-014**: System MUST support overview mode (full track), zoom mode (playback region), and spectrum mode (real-time analyzer)
- **FR-015**: System MUST track playhead position on waveform during playback
- **FR-016**: System MUST allow click-to-seek on waveform
- **FR-017**: System MUST show time and energy tooltips on waveform hover
- **FR-018**: System MUST export waveforms and spectrograms as PNG images

### Key Entities

- **Waveform Data**: Array of peak values, array of RMS values, sample rate, duration, resolution
- **Spectrogram Data**: 2D array of frequency bins over time, FFT size, frequency range, dB range
- **Frequency Band**: Band name, frequency range (low-high Hz), energy value, color mapping
- **Beat Grid**: Array of beat positions (seconds), bar positions, phrase positions, confidence scores
- **Color Scheme**: Scheme name, color gradient definition, frequency mapping rules
- **Visualization Mode**: Mode name (overview/zoom/spectrum), zoom level, display parameters

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Waveforms render within 3 seconds for typical 4-minute tracks at standard resolution
- **SC-002**: Spectrogram generation completes within 10 seconds for 4-minute tracks at 2048 FFT size
- **SC-003**: Playhead tracking is smooth at 60 FPS during playback
- **SC-004**: Click-to-seek accuracy is within ±100ms of clicked position
- **SC-005**: Frequency color mapping correctly identifies bass, mid, and high-frequency sections with 85% accuracy
- **SC-006**: Beat grid markers align with actual beats within ±50ms for 90% of tracks
- **SC-007**: Visualization mode switching occurs within 200ms without interrupting playback
- **SC-008**: Hover tooltips update at 30+ FPS for responsive interaction

## Dependencies

- Web Audio API for audio analysis
- Canvas API for waveform rendering
- FFT implementation for frequency analysis
- Beat detection system for beat grid overlay

## Assumptions

- Audio files are in supported formats with sufficient quality
- Browser supports Canvas 2D rendering
- Sufficient CPU available for real-time FFT during playback
- Tracks have consistent audio quality (not heavily compressed/clipped)

## Technical Constraints

- Canvas rendering performance limits maximum waveform resolution
- FFT size affects both accuracy and computational cost
- Real-time spectrum analysis requires continuous CPU usage
- Memory usage scales with track length and resolution
- High-resolution spectrograms can consume significant memory (50+ MB per track)

## Out of Scope

- 3D waveform visualization
- Stereo waveforms (separate left/right channels)
- Waveform editing or manipulation
- Audio restoration or cleanup based on waveform analysis
- Automatic track section detection (intro/verse/chorus)
- Machine learning-based audio feature detection
- Waveform comparison or similarity matching
- MIDI or musical note overlay on waveform
