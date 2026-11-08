# Feature Specification: Audio Mastering

**Feature Branch**: `10-audio-mastering`  
**Created**: December 15, 2025  
**Status**: In Development  
**Input**: User description: "Add professional audio mastering with compressor, limiter, multiband compression, stereo enhancement, master EQ, and venue-specific presets"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Master Output Enhancement (Priority: P1)

As a DJ performing at venues, I want professional mastering applied to my master output so my mixes sound polished and match the venue's sound system.

**Why this priority**: Core value - ensures consistent, professional sound quality across all venues.

**Independent Test**: Can be fully tested by enabling mastering chain and verifying master output is louder, punchier, and more balanced.

**Acceptance Scenarios**:

1. **Given** mastering is enabled, **When** activated, **Then** master output has increased loudness and clarity
2. **Given** dynamics are uneven, **When** mastering applies, **Then** quiet and loud parts are more balanced
3. **Given** frequency imbalance exists, **When** mastering applies, **Then** EQ correction improves tonal balance
4. **Given** mastering is disabled, **When** turned off, **Then** output returns to original unprocessed sound

---

### User Story 2 - Venue-Specific Presets (Priority: P1)

As a DJ playing different venues, I want venue-specific mastering presets (club, festival, radio, streaming) so I can instantly optimize sound for each environment.

**Why this priority**: Saves time and provides professional results without audio engineering knowledge.

**Independent Test**: Can be tested by loading different presets and verifying mastering settings change appropriately.

**Acceptance Scenarios**:

1. **Given** "Club" preset is loaded, **When** activated, **Then** heavy bass compression and loudness maximization are applied
2. **Given** "Festival" preset is loaded, **When** activated, **Then** wide stereo field and high clarity settings are applied
3. **Given** "Radio" preset is loaded, **When** activated, **Then** broadcast-compliant loudness and heavy limiting are applied
4. **Given** preset is loaded, **When** applied, **Then** all mastering parameters update to preset values

---

### User Story 3 - Dynamic Range Control (Priority: P1)

As a DJ mixing tracks with varying loudness, I want compression and limiting so my set maintains consistent volume without manual gain adjustments.

**Why this priority**: Essential for professional sound - prevents volume jumps and ear fatigue.

**Independent Test**: Can be tested by mixing loud and quiet tracks and verifying output level remains consistent.

**Acceptance Scenarios**:

1. **Given** compressor is enabled, **When** loud signals occur, **Then** peaks are reduced smoothly
2. **Given** limiter is enabled, **When** signal exceeds threshold, **Then** hard ceiling prevents clipping
3. **Given** attack/release adjusted, **When** changed, **Then** compression response time adapts accordingly
4. **Given** ratio is increased, **When** adjusted, **Then** more aggressive compression is applied

---

### User Story 4 - Multiband Frequency Shaping (Priority: P2)

As a DJ wanting precise control, I want multiband compression and EQ so I can shape different frequency ranges independently.

**Why this priority**: Professional feature for advanced users, enables surgical frequency control.

**Independent Test**: Can be tested by adjusting individual frequency bands and verifying only target frequencies are affected.

**Acceptance Scenarios**:

1. **Given** low band is adjusted, **When** compression increased, **Then** only bass frequencies are compressed
2. **Given** mid band is boosted, **When** EQ adjusted, **Then** vocal presence increases without affecting bass
3. **Given** high band is reduced, **When** cut applied, **Then** harshness decreases without dulling mids
4. **Given** all bands are adjusted, **When** settings applied, **Then** overall tonal balance improves

---

### User Story 5 - Real-Time Visual Feedback (Priority: P2)

As a DJ monitoring my output, I want real-time meters and spectrum analyzers so I can see exactly what mastering is doing to my sound.

**Why this priority**: Educational and practical - helps users understand and adjust mastering effectively.

**Independent Test**: Can be tested by playing audio and verifying meters update in real-time showing gain reduction, loudness, spectrum.

**Acceptance Scenarios**:

1. **Given** mastering is active, **When** audio plays, **Then** gain reduction meter shows compression amount
2. **Given** limiter engages, **When** peaks occur, **Then** limiter meter shows peak reduction
3. **Given** spectrum analyzer is visible, **When** audio plays, **Then** frequency content is displayed in real-time
4. **Given** loudness meter shows, **When** audio plays, **Then** LUFS value displays current loudness level

---

### Edge Cases

- What happens when mastering settings cause audio distortion?
- How does system handle extremely quiet or extremely loud input signals?
- What happens when CPU usage spikes during heavy processing?
- How are look-ahead delay and latency managed for live mixing?
- What happens if user saves preset with extreme/dangerous settings?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide master channel compressor with threshold, ratio, attack, release, knee, makeup gain
- **FR-002**: System MUST provide master channel limiter with threshold, release, ceiling, look-ahead
- **FR-003**: System MUST provide multiband compressor with 3 bands: Low (20-250Hz), Mid (250-4kHz), High (4-20kHz)
- **FR-004**: System MUST allow independent threshold, ratio, attack, release per band
- **FR-005**: System MUST provide stereo enhancer with width control (0-200%), side gain, mid-side processing
- **FR-006**: System MUST provide master EQ with 3-band parametric: Low, Mid, High shelves with gain and frequency controls
- **FR-007**: System MUST provide harmonic exciter for adding warmth/brightness
- **FR-008**: System MUST include venue presets: Club, Festival, Radio, Streaming, Lounge, Outdoor
- **FR-009**: System MUST display real-time gain reduction meters for compressor and limiter
- **FR-010**: System MUST display LUFS loudness metering (-23 LUFS to 0 LUFS range)
- **FR-011**: System MUST display real-time frequency spectrum analyzer (20Hz-20kHz)
- **FR-012**: System MUST display true peak metering with overload indicators
- **FR-013**: System MUST allow saving and loading custom mastering presets
- **FR-014**: System MUST support bypass toggle for A/B comparison
- **FR-015**: System MUST provide input/output level meters
- **FR-016**: System MUST implement proper gain staging to prevent clipping between stages
- **FR-017**: System MUST support processing chain reordering (compressor → EQ → limiter, etc.)

### Key Entities

- **Compressor Settings**: Threshold (dB), ratio (1:1 to 20:1), attack (ms), release (ms), knee (dB), makeup gain (dB)
- **Limiter Settings**: Threshold (dB), release (ms), ceiling (dBFS), look-ahead (ms)
- **Multiband Band**: Frequency range, threshold, ratio, attack, release, enabled state
- **Stereo Enhancer**: Width (%), side gain (dB), mono bass frequency (Hz)
- **Master EQ**: Low shelf (freq, gain), mid shelf (freq, gain), high shelf (freq, gain)
- **Mastering Preset**: Name, venue type, all mastering settings, description
- **Metering Data**: Peak levels, RMS levels, LUFS, gain reduction, spectrum bins

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Mastering increases perceived loudness by 6-12 LUFS without distortion
- **SC-002**: Dynamic range is controlled to ±3dB for consistent output levels
- **SC-003**: Frequency response is balanced within ±3dB across 20Hz-20kHz
- **SC-004**: Processing latency is under 10ms for real-time monitoring
- **SC-005**: Venue presets produce appropriate loudness: Club (−6 LUFS), Festival (−8 LUFS), Radio (−14 LUFS)
- **SC-006**: Meters update at 30+ FPS for smooth visual feedback
- **SC-007**: No audible artifacts (pumping, distortion) occur with proper settings
- **SC-008**: CPU usage remains under 10% on mid-range hardware

## Dependencies

- Web Audio API for audio processing nodes
- Dynamic Compressor Node, Biquad Filter Node, Waveshaper Node
- Real-time metering and analysis system

## Assumptions

- Users have basic understanding of audio dynamics or will use presets
- System has sufficient CPU for real-time audio processing
- Audio quality is sufficient for mastering to be beneficial
- Venue acoustics are reasonably balanced

## Technical Constraints

- Web Audio API has limited mastering capabilities compared to native plugins
- Look-ahead limiting requires buffering which adds latency
- Multiband processing is CPU-intensive
- Precise LUFS metering requires complex integrated loudness calculation
- True peak detection requires oversampling

## Out of Scope

- Mastering individual tracks (only master output)
- Advanced algorithms like adaptive compression or AI mastering
- Mid-side EQ beyond stereo enhancement
- Linear phase EQ (only minimum phase)
- Spectral processing or de-essing
- Mastering automation or ducking
- Dithering for bit depth reduction
- Professional loudness standards compliance (EBU R128, ATSC A/85)
