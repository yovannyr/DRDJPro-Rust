# Feature Specification: Video Mixing

**Feature Branch**: `8-video-mixing`  
**Created**: December 15, 2025  
**Status**: Completed  
**Input**: User description: "Add professional video mixing with dual video decks, effects, transitions, chroma key, and real-time output rendering"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Dual Video Deck Loading and Playback (Priority: P1)

As a video DJ, I want to load video files to two independent decks and control playback so I can mix videos like I mix audio tracks.

**Why this priority**: Core functionality - without dual deck loading/playback, video mixing is impossible.

**Independent Test**: Can be fully tested by loading videos to Deck A and Deck B, then verifying both play independently with controls.

**Acceptance Scenarios**:

1. **Given** video file is selected, **When** "Load to Deck A" clicked, **Then** video loads and displays thumbnail
2. **Given** video is loaded, **When** play button clicked, **Then** video plays with audio
3. **Given** video is playing, **When** pause button clicked, **Then** video pauses at current frame
4. **Given** timeline is visible, **When** user drags playhead, **Then** video scrubs to selected position

---

### User Story 2 - Video Crossfader Mixing (Priority: P1)

As a video DJ, I want to crossfade between two videos smoothly so I can transition between video sources like a traditional DJ crossfader.

**Why this priority**: Core mixing feature - enables basic video transitions essential for live performances.

**Independent Test**: Can be fully tested by loading two videos and moving crossfader to verify smooth blend between sources.

**Acceptance Scenarios**:

1. **Given** two videos are loaded, **When** crossfader is at full left, **Then** only Deck A video is visible
2. **Given** two videos are loaded, **When** crossfader is centered, **Then** both videos visible at 50% opacity (blended)
3. **Given** two videos are loaded, **When** crossfader is at full right, **Then** only Deck B video is visible
4. **Given** crossfader moves, **When** position changes, **Then** transition is smooth without frame drops

---

### User Story 3 - Real-Time Video Effects (Priority: P2)

As a creative video DJ, I want to apply real-time effects (brightness, contrast, saturation, blur, etc.) to videos so I can create unique visual experiences.

**Why this priority**: Differentiates from basic video playback, enables creative expression and visual matching.

**Independent Test**: Can be tested by enabling an effect and adjusting its parameter to verify real-time visual changes.

**Acceptance Scenarios**:

1. **Given** effect is enabled, **When** brightness slider adjusted, **Then** video brightness changes in real-time
2. **Given** blur effect enabled, **When** blur amount increased, **Then** video becomes progressively blurred
3. **Given** saturation adjusted, **When** slider moved, **Then** color intensity changes accordingly
4. **Given** multiple effects enabled, **When** all active, **Then** effects combine correctly without artifacts

---

### User Story 4 - Chroma Key (Green Screen) (Priority: P2)

As a video DJ using green screen content, I want chroma key functionality so I can remove backgrounds and composite videos creatively.

**Why this priority**: Professional feature for advanced video mixing, enables creative layering effects.

**Independent Test**: Can be tested by loading green screen video, enabling chroma key, and verifying green is made transparent.

**Acceptance Scenarios**:

1. **Given** video with green background loaded, **When** chroma key enabled, **Then** green areas become transparent
2. **Given** chroma key is active, **When** color picker changed, **Then** different color is keyed out
3. **Given** tolerance slider adjusted, **When** increased, **Then** more similar colors are removed
4. **Given** edge smoothing enabled, **When** adjusted, **Then** transparency edges appear smoother

---

### User Story 5 - Professional Transitions (Priority: P2)

As a video DJ switching between videos, I want pre-programmed transitions (fade, wipe, slide, zoom) so I can create polished video switches.

**Why this priority**: Professional polish for live performances, easier than manual crossfader techniques.

**Independent Test**: Can be tested by triggering a transition and verifying it executes smoothly over specified duration.

**Acceptance Scenarios**:

1. **Given** transition type selected, **When** "Trigger" button clicked, **Then** transition executes over set duration
2. **Given** fade transition selected, **When** triggered, **Then** current video fades to next video smoothly
3. **Given** wipe transition selected, **When** triggered, **Then** next video reveals left-to-right
4. **Given** transition duration set, **When** transition runs, **Then** it completes in specified time (0.5s-5s)

---

### User Story 6 - Multiple Video Layouts (Priority: P2)

As a video DJ with different venue requirements, I want multiple video layouts (single, split-screen, picture-in-picture) so I can adapt to different display setups.

**Why this priority**: Flexibility for different venues and creative styles, standard feature in VDJ/Serato Video.

**Independent Test**: Can be tested by switching between layout modes and verifying video arrangement changes correctly.

**Acceptance Scenarios**:

1. **Given** "Single" mode selected, **When** activated, **Then** crossfader-controlled single video fills output
2. **Given** "Split Screen" mode selected, **When** activated, **Then** both videos display side-by-side 50/50
3. **Given** "Picture-in-Picture" mode selected, **When** activated, **Then** one video overlays smaller in corner
4. **Given** layout changes, **When** switched, **Then** transition occurs smoothly without black frames

---

### Edge Cases

- What happens when video file is corrupted or unsupported codec?
- How does system handle extremely large video files (4K, 8K)?
- What happens when video and audio durations don't match?
- How are different aspect ratios handled (16:9, 4:3, 9:16)?
- What happens when system performance can't maintain 60 FPS rendering?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST support loading MP4, WebM, and MOV video files
- **FR-002**: System MUST provide two independent video decks (A and B)
- **FR-003**: System MUST support playback controls per deck (play, pause, seek, skip ±5 seconds)
- **FR-004**: System MUST display video thumbnails for loaded videos
- **FR-005**: System MUST show video metadata (resolution, FPS, duration, format)
- **FR-006**: System MUST implement video crossfader for mixing between decks
- **FR-007**: System MUST display crossfader position as percentage (0-100%)
- **FR-008**: System MUST support per-deck effects: Brightness (0-200%), Contrast (0-200%), Saturation (0-200%), Hue Rotation (0-360°), Blur (0-10px), Sepia (0-100%), Grayscale (0-100%), Invert (0-100%)
- **FR-009**: System MUST support master effects applied to final output
- **FR-010**: System MUST implement chroma key (green screen) with color picker, tolerance (0-100%), and edge smoothing (0-100%)
- **FR-011**: System MUST provide pre-programmed transitions: Fade, Wipe, Slide, Zoom, Dissolve
- **FR-012**: System MUST support configurable transition duration (0.5s to 5s)
- **FR-013**: System MUST support transition easing functions (linear, ease-in, ease-out, ease-in-out)
- **FR-014**: System MUST implement video layouts: Single (crossfader-controlled), Split Screen (50/50), Picture-in-Picture, Side-by-Side
- **FR-015**: System MUST support aspect ratios: 16:9, 4:3, 1:1, Auto
- **FR-016**: System MUST render output at 1920×1080 resolution (Full HD)
- **FR-017**: System MUST render at 30 FPS minimum for smooth playback
- **FR-018**: System MUST extract and sync audio from video files with DJ audio engine
- **FR-019**: System MUST support frame capture (screenshot current output)
- **FR-020**: System MUST provide real-time preview of final output

### Key Entities

- **Video Deck**: Deck ID (A/B), video source, current time, playback state, effects chain, thumbnail
- **Video Source**: File path, duration, resolution, FPS, aspect ratio, format, audio track
- **Effect**: Effect type, parameters, enabled state, target (deck/master)
- **Transition**: Type, duration, easing function, progress, source deck, target deck
- **Layout**: Layout type (single/split/PIP/side-by-side), dimensions, positions
- **Chroma Key Settings**: Key color, tolerance, edge smoothing, enabled state
- **Output Configuration**: Resolution, aspect ratio, FPS, layout, active effects

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Videos load and display first frame within 2 seconds of file selection
- **SC-002**: Playback maintains 30+ FPS without dropped frames on typical hardware
- **SC-003**: Crossfader provides smooth transitions with no visible tearing or stuttering
- **SC-004**: Effects apply in real-time with <16ms latency (60 FPS target)
- **SC-005**: Chroma key removes 95%+ of target color with clean edges at moderate tolerance
- **SC-006**: Transitions complete within specified duration ±100ms
- **SC-007**: Layout switching occurs within 200ms without black frames
- **SC-008**: System handles 1080p video at 30 FPS on mid-range hardware (4-core CPU, integrated GPU)

## Dependencies

- HTML5 Video API for video playback
- Canvas API for video rendering and compositing
- Web Audio API for video audio extraction
- CSS Filters or WebGL for real-time effects

## Assumptions

- Videos are in web-compatible formats (H.264 MP4, VP9 WebM)
- System has sufficient CPU/GPU for real-time video processing
- Videos are pre-encoded at reasonable bitrates (<10 Mbps)
- Browser supports hardware-accelerated video decoding
- Display output is 1920×1080 or compatible resolution

## Technical Constraints

- Video processing is CPU/GPU intensive - performance varies by hardware
- Browser codec support limits video format compatibility
- Canvas 2D performance limits maximum resolution and effects complexity
- Real-time rendering requires consistent frame timing (30/60 FPS)
- Memory usage scales with video resolution and loaded video count

## Out of Scope

- 4K or 8K video support (limited to 1080p)
- 60 FPS rendering (30 FPS target)
- Video recording or encoding (output to file)
- Live camera/webcam input
- NDI or Spout/Syphon video output
- Audio/video synchronization adjustment (lip sync)
- Video effects beyond provided presets (no custom shaders)
- Multi-layer video compositing (more than 2 layers)
- Video timeline editing or cutting
- 3D transformations or perspective warping
