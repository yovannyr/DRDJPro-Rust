# Vollständiges Anforderungsregister: Specs 1–25

Stand: 08.11.2026. Planungsstand: **418 FR und 200 SC**, alle offen bis zum belegten Abschluss. Quelle sind die 25 `spec.md`-Dateien einschließlich ausdrücklich dokumentierter Nutzerkorrekturen. Dieses Register ist zusammen mit [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) zu verwenden. Die dortige Spec-Abdeckung weist jeder Spec Umsetzungsslices und einen Abnahmeslice zu; diese gelten für sämtliche nachstehenden FR bzw. SC einschließlich ihrer Unterpunkte. Die vorhandene Teilimplementierung wird im Baseline-Slice gegen die [Analyse](MISSING_FEATURES_ANALYSIS.md) nachvalidiert.

Die Tabellen übernehmen die Originaltexte, damit Grenzwerte, Randbedingungen und Unterpunkte nicht in Zusammenfassungen verloren gehen. User Stories, Acceptance Scenarios, Edge Cases und Constraints der jeweils verlinkten Spec gehören zusätzlich zur Abnahme. Widersprüche erhalten eine dokumentierte Entscheidung; bis dahin bleiben die betroffenen Anforderungen offen. Eine Zeile wird erst mit Quellcode-, Test-/Messnachweis und ggf. nativer Abnahme geschlossen. Nutzerstudien und Betriebsquoten erhalten ein eigenes Nachweisprotokoll und werden nicht durch Unit-Tests als erfüllt markiert.

| Nr. | FR | SC | Umsetzungspakete | Abnahmepaket |
|---:|---:|---:|---|---|
| 1 | 13 | 8 | P01a, P01b | Q01 |
| 2 | 17 | 8 | P02a, P02b | Q02 |
| 3 | 17 | 8 | P03a, P03b | Q03 |
| 4 | 18 | 8 | P04a, P04b | Q04 |
| 5 | 17 | 8 | P05a, P05b | Q05 |
| 6 | 17 | 8 | P06a, P06b | Q06 |
| 7 | 18 | 8 | P07a, P07b | Q07 |
| 8 | 20 | 8 | P08a, P08b | Q08 |
| 9 | 17 | 8 | P09a, P09b | Q09 |
| 10 | 17 | 8 | P10a, P10b | Q10 |
| 11 | 17 | 8 | P11a, P11b | Q11 |
| 12 | 17 | 8 | P12a, P12b | Q12 |
| 13 | 17 | 8 | P13a, P13b | Q13 |
| 14 | 17 | 8 | P14a, P14b | Q14 |
| 15 | 17 | 8 | P15a, P15b | Q15 |
| 16 | 17 | 8 | P16a, P16b | Q16 |
| 17 | 17 | 8 | P17a, P17b | Q17 |
| 18 | 17 | 8 | P18a, P18b | Q18 |
| 19 | 17 | 8 | P19a, P19b | Q19 |
| 20 | 17 | 8 | P20a, P20b | Q20 |
| 21 | 17 | 8 | P21a, P21b | Q21 |
| 22 | 15 | 8 | P22a, P22b | Q22 |
| 23 | 15 | 8 | P23a, P23b | Q23 |
| 24 | 15 | 8 | P24a, P24b | Q24 |
| 25 | 15 | 8 | P25a, P25b | Q25 |

## Spec 1: 1-auto-updates

Quelle und zusätzliche Abnahmeszenarien: [1-auto-updates/spec.md](1-auto-updates/spec.md). Zuordnung: FR → P01a/P01b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q01. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST check for updates automatically on application startup after a 3-second delay | P01a.1 | Offen |
| FR-002 | System MUST display update notifications with version number and release notes | P01a.1 | Offen |
| FR-003 | System MUST allow users to download updates with real-time progress display | P01b.3 | Offen |
| FR-004 | System MUST install downloaded updates when the application quits | P01b.3 | Offen |
| FR-005 | System MUST allow users to manually trigger update checks by clicking the version display | P01a.2 | Offen |
| FR-006 | System MUST show current version number in the UI at all times | P01a.2 | Offen |
| FR-007 | System MUST support background downloads without blocking the UI | P01b.1 | Offen |
| FR-008 | System MUST handle download failures with appropriate error messages | P01b.1 | Offen |
| FR-009 | System MUST integrate with GitHub Releases for update hosting | P01a.3 | Offen |
| FR-010 | System MUST verify update file integrity before installation | P01b.2 | Offen |
| FR-011 | System MUST preserve user data during update installation | P01b.2 | Offen |
| FR-012 | System MUST log all update-related events for debugging | P01b.2 | Offen |
| FR-013 | System MUST support multiple update channels (stable, beta) | P01a.3 | Offen |
| SC-001 | Users receive update notifications within 5 seconds of app startup when updates are available | Q01 | Offen |
| SC-002 | Update downloads complete without user intervention in the background | Q01 | Offen |
| SC-003 | 95% of users successfully install updates on first attempt | Q01 | Offen |
| SC-004 | Update installations complete within 30 seconds after app quit | Q01 | Offen |
| SC-005 | Zero data loss occurs during update installation process | Q01 | Offen |
| SC-006 | Download progress updates at least once per second during active downloads | Q01 | Offen |
| SC-007 | Manual update checks complete within 2 seconds | Q01 | Offen |
| SC-008 | System gracefully handles network failures with clear error messages | Q01 | Offen |

## Spec 2: 2-cloud-sync

Quelle und zusätzliche Abnahmeszenarien: [2-cloud-sync/spec.md](2-cloud-sync/spec.md). Zuordnung: FR → P02a/P02b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q02. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST sync track library metadata (title, artist, album, BPM, key, duration, file path) | P02a.2 | Offen |
| FR-002 | System MUST sync all user playlists and playlist contents | P02a.2 | Offen |
| FR-003 | System MUST sync cue points for each track (hot cues with positions, colors, labels) | P02a.2 | Offen |
| FR-004 | System MUST sync beat grid analysis data for each track | P02a.2 | Offen |
| FR-005 | System MUST sync all application settings and preferences | P02a.3 | Offen |
| FR-006 | System MUST display real-time sync status (syncing, synced, error, offline) | P02a.3 | Offen |
| FR-007 | System MUST support manual sync trigger via UI button | P02a.3 | Offen |
| FR-008 | System MUST support automatic sync on a configurable interval | P02a.3 | Offen |
| FR-009 | System MUST allow selective sync by category (library, playlists, settings, cue points, beat grids) | P02b.1 | Offen |
| FR-010 | System MUST track last sync timestamp per device | P02b.1 | Offen |
| FR-011 | System MUST identify each device uniquely for conflict resolution | P02b.1 | Offen |
| FR-012 | System MUST implement last-write-wins conflict resolution strategy | P02b.2 | Offen |
| FR-013 | System MUST work offline and queue changes for next sync | P02b.2 | Offen |
| FR-014 | System MUST authenticate users before allowing sync operations | P02a.1 | Offen |
| FR-015 | System MUST isolate data per user (no cross-user data access) | P02a.1 | Offen |
| FR-016 | System MUST handle sync errors gracefully with user-friendly messages | P02b.3 | Offen |
| FR-017 | System MUST report item counts after successful sync (e.g., "Synced 1,234 tracks") | P02b.3 | Offen |
| SC-001 | Users can access their complete library on any device within 2 minutes of sync completion | Q02 | Offen |
| SC-002 | Sync operations complete within 30 seconds for libraries under 10,000 tracks | Q02 | Offen |
| SC-003 | 99% of sync operations complete successfully without data loss | Q02 | Offen |
| SC-004 | Conflict resolution preserves at least 95% of user changes in multi-device scenarios | Q02 | Offen |
| SC-005 | Sync status updates are visible within 1 second of status changes | Q02 | Offen |
| SC-006 | Offline changes sync successfully when connectivity is restored | Q02 | Offen |
| SC-007 | System handles 100,000+ track libraries without performance degradation | Q02 | Offen |
| SC-008 | Auto-sync completes without user awareness or UI interruption | Q02 | Offen |

## Spec 3: 3-bpm-detection

Quelle und zusätzliche Abnahmeszenarien: [3-bpm-detection/spec.md](3-bpm-detection/spec.md). Zuordnung: FR → P03a/P03b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q03. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST analyze audio files to detect beats per minute (BPM) | P03a.1 | Offen |
| FR-002 | System MUST detect individual beat positions throughout tracks | P03a.1 | Offen |
| FR-003 | System MUST calculate confidence scores for BPM detection (0-100%) | P03a.2 | Offen |
| FR-004 | System MUST classify tempo ranges (slow: 60-90, medium: 90-120, fast: 120-160, very fast: 160+) | P03a.2 | Offen |
| FR-005 | System MUST detect time signatures (4/4, 3/4, 6/8, etc.) | P03a.2 | Offen |
| FR-006 | System MUST handle tracks from 40 BPM to 200+ BPM | P03a.3 | Offen |
| FR-007 | System MUST correct half-time and double-time octave errors | P03a.3 | Offen |
| FR-008 | System MUST provide manual BPM entry option | P03a.3 | Offen |
| FR-009 | System MUST implement tap tempo functionality (minimum 4 taps) | P03b.1 | Offen |
| FR-010 | System MUST support batch analysis of multiple tracks | P03b.1 | Offen |
| FR-011 | System MUST display analysis progress for batch operations | P03b.1 | Offen |
| FR-012 | System MUST store BPM data with track metadata | P03b.2 | Offen |
| FR-013 | System MUST allow manual override of detected BPM | P03b.2 | Offen |
| FR-014 | System MUST calculate beat interval variance for consistency measurement | P03b.2 | Offen |
| FR-015 | System MUST use onset strength detection for beat finding | P03b.3 | Offen |
| FR-016 | System MUST apply low-pass filtering (0-200Hz) to focus on kick drums | P03b.3 | Offen |
| FR-017 | System MUST use histogram clustering to identify dominant tempo | P03b.3 | Offen |
| SC-001 | BPM detection accuracy within ±2 BPM for 95% of tracks with clear rhythm | Q03 | Offen |
| SC-002 | Analysis completes within 10 seconds for typical 4-minute tracks | Q03 | Offen |
| SC-003 | Confidence scores above 80% for 90% of electronic/pop music tracks | Q03 | Offen |
| SC-004 | Beat positions align with actual beats within ±50ms for 95% of detections | Q03 | Offen |
| SC-005 | Half-time/double-time errors are automatically corrected in 85% of cases | Q03 | Offen |
| SC-006 | Batch analysis processes 100 tracks in under 20 minutes | Q03 | Offen |
| SC-007 | Tap tempo calculates BPM within ±1 BPM of actual tempo when tapped accurately | Q03 | Offen |
| SC-008 | Users can successfully analyze any genre without crashes or hangs | Q03 | Offen |

## Spec 4: 4-midi-controller

Quelle und zusätzliche Abnahmeszenarien: [4-midi-controller/spec.md](4-midi-controller/spec.md). Zuordnung: FR → P04a/P04b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q04. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST detect all connected MIDI devices automatically on startup | P04a.1 | Offen |
| FR-002 | System MUST detect MIDI device connect/disconnect in real-time (hot-plug support) | P04a.1 | Offen |
| FR-003 | System MUST parse MIDI messages (Note On, Note Off, Control Change, Pitch Bend) | P04a.2 | Offen |
| FR-004 | System MUST support bidirectional MIDI (input and output) | P04a.2 | Offen |
| FR-005 | System MUST include preset mappings for: Pioneer DDJ-400, Pioneer DDJ-SB3, Numark Party Mix | P04a.2 | Offen |
| FR-006 | System MUST auto-detect device models and apply appropriate preset mappings | P04a.3 | Offen |
| FR-007 | System MUST provide generic MIDI mapping as fallback for unsupported devices | P04a.3 | Offen |
| FR-008 | System MUST implement MIDI learn with one-click assignment | P04a.3 | Offen |
| FR-009 | System MUST timeout MIDI learn after 10 seconds of inactivity | P04b.1 | Offen |
| FR-010 | System MUST support custom mapping creation and editing | P04b.1 | Offen |
| FR-011 | System MUST save custom mappings to persistent storage | P04b.1 | Offen |
| FR-012 | System MUST allow mapping any MIDI control to any DJ function | P04b.2 | Offen |
| FR-013 | System MUST support multiple control modes (toggle, momentary, absolute, relative) | P04b.2 | Offen |
| FR-014 | System MUST support response curves (linear, logarithmic, exponential) | P04b.2 | Offen |
| FR-015 | System MUST send LED feedback to controllers for active states | P04b.3 | Offen |
| FR-016 | System MUST show visual activity indicators for incoming MIDI messages | P04b.3 | Offen |
| FR-017 | System MUST support multiple simultaneous MIDI devices | P04b.3 | Offen |
| FR-018 | System MUST handle MIDI message rates up to 1000 messages/second per device | P04b.3 | Offen |
| SC-001 | MIDI devices are detected within 2 seconds of connection | Q04 | Offen |
| SC-002 | MIDI input latency is below 20ms from controller to DJ function response | Q04 | Offen |
| SC-003 | 95% of popular DJ controllers work with preset mappings without customization | Q04 | Offen |
| SC-004 | Users can create custom mappings using MIDI learn in under 5 minutes per control | Q04 | Offen |
| SC-005 | System handles 1000+ MIDI messages per second without lag or dropped messages | Q04 | Offen |
| SC-006 | LED feedback responds within 50ms of application state changes | Q04 | Offen |
| SC-007 | Multiple controllers work simultaneously without conflicts or interference | Q04 | Offen |
| SC-008 | Custom mappings persist across application restarts | Q04 | Offen |

## Spec 5: 5-streaming-integration

Quelle und zusätzliche Abnahmeszenarien: [5-streaming-integration/spec.md](5-streaming-integration/spec.md). Zuordnung: FR → P05a/P05b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q05. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST support OAuth 2.0 authentication for Spotify, SoundCloud, and TIDAL | P05a.1 | Offen |
| FR-002 | System MUST securely store and encrypt authentication tokens | P05a.1 | Offen |
| FR-003 | System MUST automatically refresh expired authentication tokens | P05a.2 | Offen |
| FR-004 | System MUST allow users to disconnect from any service | P05a.2 | Offen |
| FR-005 | System MUST provide unified search across all connected services | P05a.2 | Offen |
| FR-006 | System MUST display search results with service source indicators | P05a.3 | Offen |
| FR-007 | System MUST load streaming tracks directly to decks for playback | P05a.3 | Offen |
| FR-008 | System MUST extract and display track metadata (BPM, key, duration, genre, artwork) | P05a.3 | Offen |
| FR-009 | System MUST browse user's personal playlists from each service | P05b.1 | Offen |
| FR-010 | System MUST stream audio at highest quality available per service (Spotify 320kbps, TIDAL HiFi/Master, SoundCloud Go+) | P05b.1 | Offen |
| FR-011 | System MUST handle preview-only tracks (30-second Spotify previews) and indicate limitations | P05b.1 | Offen |
| FR-012 | System MUST show connection status for each service (connected, disconnected, error) | P05b.2 | Offen |
| FR-013 | System MUST handle API rate limiting gracefully with user feedback | P05b.2 | Offen |
| FR-014 | System MUST handle region-locked and unavailable tracks with clear messages | P05b.2 | Offen |
| FR-015 | System MUST support searching with filters (genre, BPM range, key) | P05b.3 | Offen |
| FR-016 | System MUST cache frequently accessed data (playlists, metadata) for performance | P05b.3 | Offen |
| FR-017 | System MUST support multiple services connected simultaneously | P05b.3 | Offen |
| SC-001 | Users can authenticate with any streaming service in under 30 seconds | Q05 | Offen |
| SC-002 | Search results appear within 2 seconds of query submission | Q05 | Offen |
| SC-003 | Tracks load to decks and begin playback within 3 seconds | Q05 | Offen |
| SC-004 | 95% of searched tracks are available and playable | Q05 | Offen |
| SC-005 | Audio quality matches service's highest tier (320kbps+ for Spotify Premium, lossless for TIDAL HiFi) | Q05 | Offen |
| SC-006 | Personal playlists load within 5 seconds regardless of playlist size | Q05 | Offen |
| SC-007 | Token refresh occurs automatically without user interruption | Q05 | Offen |
| SC-008 | System gracefully handles all error conditions with actionable user messages | Q05 | Offen |

## Spec 6: 6-key-detection

Quelle und zusätzliche Abnahmeszenarien: [6-key-detection/spec.md](6-key-detection/spec.md). Zuordnung: FR → P06a/P06b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q06. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST analyze audio files to detect musical key | P06a.1 | Offen |
| FR-002 | System MUST display keys in Camelot notation (1A-12B for minor/major keys) | P06a.1 | Offen |
| FR-003 | System MUST display keys in traditional notation (e.g., "C Major", "A Minor") | P06a.2 | Offen |
| FR-004 | System MUST support Open Key notation as alternative display format | P06a.2 | Offen |
| FR-005 | System MUST calculate confidence scores for key detection (0-100%) | P06a.2 | Offen |
| FR-006 | System MUST display interactive Camelot wheel with all 24 keys | P06a.3 | Offen |
| FR-007 | System MUST highlight compatible keys on Camelot wheel when key is selected | P06a.3 | Offen |
| FR-008 | System MUST provide real-time compatibility checking between deck keys | P06a.3 | Offen |
| FR-009 | System MUST calculate compatibility ratings: Perfect (same key, ±1 number, relative major/minor), Good (±2 numbers), Fair (±3 numbers, tritone), Poor (all others) | P06b.1 | Offen |
| FR-010 | System MUST implement mixing assistant with compatible track suggestions | P06b.1 | Offen |
| FR-011 | System MUST show mixing relationship descriptions (e.g., "Perfect Fifth", "Relative Minor") | P06b.1 | Offen |
| FR-012 | System MUST allow manual key entry and override of detected keys | P06b.2 | Offen |
| FR-013 | System MUST use chromagram analysis (12-bin pitch class profiling) | P06b.2 | Offen |
| FR-014 | System MUST implement Krumhansl-Schmuckler key finding algorithm | P06b.2 | Offen |
| FR-015 | System MUST detect both major and minor keys | P06b.3 | Offen |
| FR-016 | System MUST store key data with track metadata | P06b.3 | Offen |
| FR-017 | System MUST visualize chromagram (pitch class distribution) for advanced users | P06b.3 | Offen |
| SC-001 | Key detection accuracy of 85%+ for tonal music (pop, electronic, rock) | Q06 | Offen |
| SC-002 | Key analysis completes within 15 seconds for typical 4-minute tracks | Q06 | Offen |
| SC-003 | Confidence scores above 70% for 80% of tonal music tracks | Q06 | Offen |
| SC-004 | Camelot wheel renders and is interactive within 500ms | Q06 | Offen |
| SC-005 | Compatibility ratings accurately reflect music theory relationships in 95% of cases | Q06 | Offen |
| SC-006 | Mixing assistant suggests at least 3 compatible tracks when available in library | Q06 | Offen |
| SC-007 | Manual key override applies instantly without re-analysis | Q06 | Offen |
| SC-008 | Users can successfully identify compatible mixes without prior music theory knowledge | Q06 | Offen |

## Spec 7: 7-advanced-waveform

Quelle und zusätzliche Abnahmeszenarien: [7-advanced-waveform/spec.md](7-advanced-waveform/spec.md). Zuordnung: FR → P07a/P07b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q07. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST generate waveforms showing peak and RMS amplitude values | P07a.1 | Offen |
| FR-002 | System MUST analyze tracks using Short-Time Fourier Transform (STFT) for spectrograms | P07a.1 | Offen |
| FR-003 | System MUST calculate frequency band energy across 7 bands: Sub Bass (20-60Hz), Bass (60-250Hz), Low Mid (250-500Hz), Mid (500-2kHz), High Mid (2-4kHz), Presence (4-6kHz), Brilliance (6-20kHz) | P07a.1 | Offen |
| FR-004 | System MUST color-map waveforms based on dominant frequency content | P07a.2 | Offen |
| FR-005 | System MUST support multiple color schemes: Default, Frequency, Energy, Rainbow, Classic (Rekordbox-style), Hot, Cool, Grayscale | P07a.2 | Offen |
| FR-006 | System MUST overlay beat grid markers from BPM detection | P07a.2 | Offen |
| FR-007 | System MUST display bar markers (every 4 beats) and phrase markers (every 16 beats) | P07a.3 | Offen |
| FR-008 | System MUST provide configurable waveform resolution (256 to 2048 pixels) | P07a.3 | Offen |
| FR-009 | System MUST render spectrograms with configurable FFT sizes (1024, 2048, 4096, 8192 samples) | P07a.3 | Offen |
| FR-010 | System MUST display frequency axis labels on spectrograms (20Hz, 100Hz, 1kHz, 10kHz, 20kHz) | P07b.1 | Offen |
| FR-011 | System MUST normalize spectrogram amplitude to -90dB to 0dB range | P07b.1 | Offen |
| FR-012 | System MUST show interactive crosshair on spectrogram hover with time/frequency values | P07b.1 | Offen |
| FR-013 | System MUST display real-time frequency spectrum during playback | P07b.2 | Offen |
| FR-014 | System MUST support overview mode (full track), zoom mode (playback region), and spectrum mode (real-time analyzer) | P07b.2 | Offen |
| FR-015 | System MUST track playhead position on waveform during playback | P07b.2 | Offen |
| FR-016 | System MUST allow click-to-seek on waveform | P07b.3 | Offen |
| FR-017 | System MUST show time and energy tooltips on waveform hover | P07b.3 | Offen |
| FR-018 | System MUST export waveforms and spectrograms as PNG images | P07b.3 | Offen |
| SC-001 | Waveforms render within 3 seconds for typical 4-minute tracks at standard resolution | Q07 | Offen |
| SC-002 | Spectrogram generation completes within 10 seconds for 4-minute tracks at 2048 FFT size | Q07 | Offen |
| SC-003 | Playhead tracking is smooth at 60 FPS during playback | Q07 | Offen |
| SC-004 | Click-to-seek accuracy is within ±100ms of clicked position | Q07 | Offen |
| SC-005 | Frequency color mapping correctly identifies bass, mid, and high-frequency sections with 85% accuracy | Q07 | Offen |
| SC-006 | Beat grid markers align with actual beats within ±50ms for 90% of tracks | Q07 | Offen |
| SC-007 | Visualization mode switching occurs within 200ms without interrupting playback | Q07 | Offen |
| SC-008 | Hover tooltips update at 30+ FPS for responsive interaction | Q07 | Offen |

## Spec 8: 8-video-mixing

Quelle und zusätzliche Abnahmeszenarien: [8-video-mixing/spec.md](8-video-mixing/spec.md). Zuordnung: FR → P08a/P08b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q08. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST support loading MP4, WebM, and MOV video files | P08a.1 | Offen |
| FR-002 | System MUST provide two independent video decks (A and B) | P08a.1 | Offen |
| FR-003 | System MUST support playback controls per deck (play, pause, seek, skip ±5 seconds) | P08a.1 | Offen |
| FR-004 | System MUST display video thumbnails for loaded videos | P08a.2 | Offen |
| FR-005 | System MUST show video metadata (resolution, FPS, duration, format) | P08a.2 | Offen |
| FR-006 | System MUST implement video crossfader for mixing between decks | P08a.2 | Offen |
| FR-007 | System MUST display crossfader position as percentage (0-100%) | P08a.3 | Offen |
| FR-008 | System MUST support per-deck effects: Brightness (0-200%), Contrast (0-200%), Saturation (0-200%), Hue Rotation (0-360°), Blur (0-10px), Sepia (0-100%), Grayscale (0-100%), Invert (0-100%) | P08a.3 | Offen |
| FR-009 | System MUST support master effects applied to final output | P08a.3 | Offen |
| FR-010 | System MUST implement chroma key (green screen) with color picker, tolerance (0-100%), and edge smoothing (0-100%) | P08b.1 | Offen |
| FR-011 | System MUST provide pre-programmed transitions: Fade, Wipe, Slide, Zoom, Dissolve | P08b.1 | Offen |
| FR-012 | System MUST support configurable transition duration (0.5s to 5s) | P08b.1 | Offen |
| FR-013 | System MUST support transition easing functions (linear, ease-in, ease-out, ease-in-out) | P08b.2 | Offen |
| FR-014 | System MUST implement video layouts: Single (crossfader-controlled), Split Screen (50/50), Picture-in-Picture, Side-by-Side | P08b.2 | Offen |
| FR-015 | System MUST support aspect ratios: 16:9, 4:3, 1:1, Auto | P08b.2 | Offen |
| FR-016 | System MUST render output at 1920×1080 resolution (Full HD) | P08b.2 | Offen |
| FR-017 | System MUST render at 30 FPS minimum for smooth playback | P08b.3 | Offen |
| FR-018 | System MUST extract and sync audio from video files with DJ audio engine | P08b.3 | Offen |
| FR-019 | System MUST support frame capture (screenshot current output) | P08b.3 | Offen |
| FR-020 | System MUST provide real-time preview of final output | P08b.3 | Offen |
| SC-001 | Videos load and display first frame within 2 seconds of file selection | Q08 | Offen |
| SC-002 | Playback maintains 30+ FPS without dropped frames on typical hardware | Q08 | Offen |
| SC-003 | Crossfader provides smooth transitions with no visible tearing or stuttering | Q08 | Offen |
| SC-004 | Effects apply in real-time with <16ms latency (60 FPS target) | Q08 | Offen |
| SC-005 | Chroma key removes 95%+ of target color with clean edges at moderate tolerance | Q08 | Offen |
| SC-006 | Transitions complete within specified duration ±100ms | Q08 | Offen |
| SC-007 | Layout switching occurs within 200ms without black frames | Q08 | Offen |
| SC-008 | System handles 1080p video at 30 FPS on mid-range hardware (4-core CPU, integrated GPU) | Q08 | Offen |

## Spec 9: 9-ai-automix

Quelle und zusätzliche Abnahmeszenarien: [9-ai-automix/spec.md](9-ai-automix/spec.md). Zuordnung: FR → P09a/P09b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q09. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST analyze tracks for BPM, key, energy level, and genre | P09a.1 | Offen |
| FR-002 | System MUST calculate compatibility scores between track pairs (0-100%) | P09a.1 | Offen |
| FR-003 | System MUST consider BPM compatibility (±6 BPM preferred, ±10 maximum) | P09a.2 | Offen |
| FR-004 | System MUST consider key compatibility using Camelot wheel rules | P09a.2 | Offen |
| FR-005 | System MUST analyze energy levels (low/medium/high) from amplitude and frequency analysis | P09a.2 | Offen |
| FR-006 | System MUST generate mix plans with track sequencing and transition points | P09a.3 | Offen |
| FR-007 | System MUST support multiple energy flow modes: Build Up, Build Down, Peak & Valley, Steady | P09a.3 | Offen |
| FR-008 | System MUST execute automated transitions with configurable lengths (15-60 seconds) | P09a.3 | Offen |
| FR-009 | System MUST support automatic BPM syncing during transitions | P09b.1 | Offen |
| FR-010 | System MUST allow manual track selection override during auto-mix | P09b.1 | Offen |
| FR-011 | System MUST provide detailed compatibility breakdowns (BPM %, Key %, Energy %, Genre %) | P09b.1 | Offen |
| FR-012 | System MUST allow mix plan preview and manual editing before execution | P09b.2 | Offen |
| FR-013 | System MUST support genre-aware mixing (stay within genre or allow cross-genre) | P09b.2 | Offen |
| FR-014 | System MUST handle tracks with missing metadata gracefully | P09b.2 | Offen |
| FR-015 | System MUST save and load mix plans for later execution | P09b.3 | Offen |
| FR-016 | System MUST learn from user overrides to improve future suggestions | P09b.3 | Offen |
| FR-017 | System MUST support manual transition triggering during auto-mix | P09b.3 | Offen |
| SC-001 | AI suggests compatible next tracks with 85%+ compatibility scores | Q09 | Offen |
| SC-002 | Automated transitions sound seamless with no audible clashing or key dissonance | Q09 | Offen |
| SC-003 | Mix plans generate within 5 seconds for libraries under 1000 tracks | Q09 | Offen |
| SC-004 | Energy flow follows configured mode in 90%+ of transitions | Q09 | Offen |
| SC-005 | BPM transitions complete smoothly within configured transition duration | Q09 | Offen |
| SC-006 | Users can manually override AI decisions at any point without interrupting playback | Q09 | Offen |
| SC-007 | Compatibility scoring accuracy improves by 10% after 10 manual overrides | Q09 | Offen |
| SC-008 | 80% of DJ users find AI suggestions acceptable without manual editing | Q09 | Offen |

## Spec 10: 10-audio-mastering

Quelle und zusätzliche Abnahmeszenarien: [10-audio-mastering/spec.md](10-audio-mastering/spec.md). Zuordnung: FR → P10a/P10b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q10. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST provide master channel compressor with threshold, ratio, attack, release, knee, makeup gain | P10a.1 | Offen |
| FR-002 | System MUST provide master channel limiter with threshold, release, ceiling, look-ahead | P10a.1 | Offen |
| FR-003 | System MUST provide multiband compressor with 3 bands: Low (20-250Hz), Mid (250-4kHz), High (4-20kHz) | P10a.2 | Offen |
| FR-004 | System MUST allow independent threshold, ratio, attack, release per band | P10a.2 | Offen |
| FR-005 | System MUST provide stereo enhancer with width control (0-200%), side gain, mid-side processing | P10a.2 | Offen |
| FR-006 | System MUST provide master EQ with 3-band parametric: Low, Mid, High shelves with gain and frequency controls | P10a.3 | Offen |
| FR-007 | System MUST provide harmonic exciter for adding warmth/brightness | P10a.3 | Offen |
| FR-008 | System MUST include venue presets: Club, Festival, Radio, Streaming, Lounge, Outdoor | P10a.3 | Offen |
| FR-009 | System MUST display real-time gain reduction meters for compressor and limiter | P10b.1 | Offen |
| FR-010 | System MUST display LUFS loudness metering (-23 LUFS to 0 LUFS range) | P10b.1 | Offen |
| FR-011 | System MUST display real-time frequency spectrum analyzer (20Hz-20kHz) | P10b.1 | Offen |
| FR-012 | System MUST display true peak metering with overload indicators | P10b.2 | Offen |
| FR-013 | System MUST allow saving and loading custom mastering presets | P10b.2 | Offen |
| FR-014 | System MUST support bypass toggle for A/B comparison | P10b.2 | Offen |
| FR-015 | System MUST provide input/output level meters | P10b.3 | Offen |
| FR-016 | System MUST implement proper gain staging to prevent clipping between stages | P10b.3 | Offen |
| FR-017 | System MUST support processing chain reordering (compressor → EQ → limiter, etc.) | P10b.3 | Offen |
| SC-001 | Mastering increases perceived loudness by 6-12 LUFS without distortion | Q10 | Offen |
| SC-002 | Dynamic range is controlled to ±3dB for consistent output levels | Q10 | Offen |
| SC-003 | Frequency response is balanced within ±3dB across 20Hz-20kHz | Q10 | Offen |
| SC-004 | Processing latency is under 10ms for real-time monitoring | Q10 | Offen |
| SC-005 | Venue presets produce appropriate loudness: Club (−6 LUFS), Festival (−8 LUFS), Radio (−14 LUFS) | Q10 | Offen |
| SC-006 | Meters update at 30+ FPS for smooth visual feedback | Q10 | Offen |
| SC-007 | No audible artifacts (pumping, distortion) occur with proper settings | Q10 | Offen |
| SC-008 | CPU usage remains under 10% on mid-range hardware | Q10 | Offen |

## Spec 11: 11-dvs-control

Quelle und zusätzliche Abnahmeszenarien: [11-dvs-control/spec.md](11-dvs-control/spec.md). Zuordnung: FR → P11a/P11b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q11. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST decode timecode audio signals to extract position and velocity | P11a.1 | Offen |
| FR-002 | System MUST support Serato CV02.5 timecode format | P11a.1 | Offen |
| FR-003 | System MUST support Traktor MK2/MK3 timecode formats | P11a.2 | Offen |
| FR-004 | System MUST support Rekordbox Control Vinyl timecode | P11a.2 | Offen |
| FR-005 | System MUST track vinyl position with <5ms latency | P11a.2 | Offen |
| FR-006 | System MUST detect and respond to scratches, backspins, and speed changes | P11a.3 | Offen |
| FR-007 | System MUST implement automatic calibration on startup | P11a.3 | Offen |
| FR-008 | System MUST allow manual calibration adjustments (gain, offset, phase) | P11a.3 | Offen |
| FR-009 | System MUST display signal quality meter (0-100%) | P11b.1 | Offen |
| FR-010 | System MUST display tracking accuracy score | P11b.1 | Offen |
| FR-011 | System MUST auto-recover from temporary signal loss (<500ms) | P11b.1 | Offen |
| FR-012 | System MUST support relative mode (scratch like vinyl) and absolute mode (scratch and seek) | P11b.2 | Offen |
| FR-013 | System MUST provide visual platter simulation with rotation and cue points | P11b.2 | Offen |
| FR-014 | System MUST support CDJ mode (jog wheel emulation) | P11b.2 | Offen |
| FR-015 | System MUST track playback direction (forward/reverse/stopped) | P11b.3 | Offen |
| FR-016 | System MUST calculate and display vinyl RPM | P11b.3 | Offen |
| FR-017 | System MUST support pitch range adjustment (±8%, ±16%, ±50%) | P11b.3 | Offen |
| SC-001 | Tracking latency under 5ms for responsive scratch performance | Q11 | Offen |
| SC-002 | Position accuracy within ±50ms over 5-minute playback | Q11 | Offen |
| SC-003 | Scratch detection responds to movements as fast as 10 Hz | Q11 | Offen |
| SC-004 | Auto-calibration achieves 90%+ tracking accuracy within 5 seconds | Q11 | Offen |
| SC-005 | System maintains sync without drift for 30+ minute continuous playback | Q11 | Offen |
| SC-006 | Signal dropout recovery occurs within 100ms without audible glitch | Q11 | Offen |
| SC-007 | Visual platter updates at 60 FPS for smooth rotation display | Q11 | Offen |
| SC-008 | All three major timecode formats (Serato/Traktor/Rekordbox) are accurately decoded | Q11 | Offen |

## Spec 12: 12-stem-control

Quelle und zusätzliche Abnahmeszenarien: [12-stem-control/spec.md](12-stem-control/spec.md). Zuordnung: FR → P12a/P12b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q12. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST separate tracks into 4 stems: Vocals, Drums, Bass, Other (melodic instruments) | P12a.1 | Offen |
| FR-002 | System MUST provide progress indication during stem extraction (0-100%) | P12a.1 | Offen |
| FR-003 | System MUST support stem extraction cancellation mid-process | P12a.2 | Offen |
| FR-004 | System MUST cache separated stems for instant reloading | P12a.2 | Offen |
| FR-005 | System MUST provide independent volume control per stem (0-100%) | P12a.2 | Offen |
| FR-006 | System MUST provide mute/solo buttons per stem | P12a.3 | Offen |
| FR-007 | System MUST apply effects independently to each stem | P12a.3 | Offen |
| FR-008 | System MUST support stem preset combinations (save/recall) | P12a.3 | Offen |
| FR-009 | System MUST display waveforms for each individual stem | P12b.1 | Offen |
| FR-010 | System MUST support headphone cueing individual stems | P12b.1 | Offen |
| FR-011 | System MUST maintain stem synchronization during playback | P12b.1 | Offen |
| FR-012 | System MUST handle cache storage limits gracefully | P12b.2 | Offen |
| FR-013 | System MUST support stem export to individual audio files | P12b.2 | Offen |
| FR-014 | System MUST provide stem quality indicators | P12b.2 | Offen |
| FR-015 | System MUST support batch stem extraction for multiple tracks | P12b.3 | Offen |
| FR-016 | System MUST estimate separation time before starting | P12b.3 | Offen |
| FR-017 | System MUST use AI/ML-based separation algorithms (Spleeter, Demucs, or similar) | P12b.3 | Offen |
| SC-001 | Stem separation completes within 2 minutes for typical 4-minute tracks | Q12 | Offen |
| SC-002 | Vocal isolation achieves 85%+ accuracy (minimal bleed from other instruments) | Q12 | Offen |
| SC-003 | Drum separation achieves 80%+ accuracy with clean kick and snare | Q12 | Offen |
| SC-004 | Cached stems load within 500ms | Q12 | Offen |
| SC-005 | Real-time stem switching occurs within 10ms without audible clicks | Q12 | Offen |
| SC-006 | Stem synchronization remains accurate within ±1ms over full track duration | Q12 | Offen |
| SC-007 | Cache management maintains 100 most recent stem sets without manual intervention | Q12 | Offen |
| SC-008 | Batch processing handles 10+ tracks without crashes or memory issues | Q12 | Offen |

## Spec 13: 13-library-management

Quelle und zusätzliche Abnahmeszenarien: [13-library-management/spec.md](13-library-management/spec.md). Zuordnung: FR → P13a/P13b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q13. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST support creating, renaming, and deleting playlists | P13a.1 | Offen |
| FR-002 | System MUST support adding/removing tracks from playlists | P13a.1 | Offen |
| FR-003 | System MUST support reordering tracks within playlists | P13a.2 | Offen |
| FR-004 | System MUST provide full-text search across all metadata fields | P13a.2 | Offen |
| FR-005 | System MUST support filtering by BPM range, key, genre, year, rating | P13a.2 | Offen |
| FR-006 | System MUST support smart playlists with auto-population rules | P13a.3 | Offen |
| FR-007 | System MUST support AND/OR logic in smart playlist rules | P13a.3 | Offen |
| FR-008 | System MUST support crates (folders) containing multiple playlists | P13a.3 | Offen |
| FR-009 | System MUST support track metadata editing (title, artist, album, BPM, key, genre, year, rating, comments) | P13b.1 | Offen |
| FR-010 | System MUST support custom color tagging per track | P13b.1 | Offen |
| FR-011 | System MUST support play count and last played tracking | P13b.1 | Offen |
| FR-012 | System MUST provide list view and grid view modes | P13b.2 | Offen |
| FR-013 | System MUST support sorting by any metadata field (ascending/descending) | P13b.2 | Offen |
| FR-014 | System MUST support multi-select for batch operations | P13b.2 | Offen |
| FR-015 | System MUST display total track count and duration for playlists | P13b.3 | Offen |
| FR-016 | System MUST support playlist export to M3U/PLS formats | P13b.3 | Offen |
| FR-017 | System MUST support duplicate track detection | P13b.3 | Offen |
| SC-001 | Search returns results within 100ms for libraries under 10,000 tracks | Q13 | Offen |
| SC-002 | Playlist creation and track addition completes within 200ms | Q13 | Offen |
| SC-003 | Smart playlist updates occur within 1 second of metadata changes | Q13 | Offen |
| SC-004 | View mode switching completes within 300ms without lag | Q13 | Offen |
| SC-005 | Batch operations on 100 tracks complete within 2 seconds | Q13 | Offen |
| SC-006 | Filtering by multiple criteria returns results within 150ms | Q13 | Offen |
| SC-007 | Metadata edits persist immediately and sync to cloud within 5 seconds | Q13 | Offen |
| SC-008 | Users can organize 10,000+ track libraries without performance degradation | Q13 | Offen |

## Spec 14: 14-recording-system

Quelle und zusätzliche Abnahmeszenarien: [14-recording-system/spec.md](14-recording-system/spec.md). Zuordnung: FR → P14a/P14b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q14. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST record audio in WAV, MP3, and FLAC formats | P14a.1 | Offen |
| FR-002 | System MUST provide quality settings: Low (128kbps), Medium (256kbps), High (320kbps), Lossless (WAV/FLAC) | P14a.1 | Offen |
| FR-003 | System MUST support recording from master output, Deck A, or Deck B | P14a.2 | Offen |
| FR-004 | System MUST display real-time recording duration (hours:minutes:seconds) | P14a.2 | Offen |
| FR-005 | System MUST calculate and display estimated file size during recording | P14a.2 | Offen |
| FR-006 | System MUST pause and resume recording without audio gaps | P14a.3 | Offen |
| FR-007 | System MUST support auto-split recording at configurable intervals (15-120 minutes) | P14a.3 | Offen |
| FR-008 | System MUST save recordings with metadata: name, duration, file size, date, source | P14a.3 | Offen |
| FR-009 | System MUST display list of all recordings with sortable columns | P14b.1 | Offen |
| FR-010 | System MUST allow renaming recordings | P14b.1 | Offen |
| FR-011 | System MUST allow deleting recordings with confirmation prompt | P14b.1 | Offen |
| FR-012 | System MUST calculate and display total storage used by recordings | P14b.2 | Offen |
| FR-013 | System MUST tag audio files with ID3 tags (for MP3) or metadata (for WAV/FLAC) | P14b.2 | Offen |
| FR-014 | System MUST prevent source changes during active recording | P14b.2 | Offen |
| FR-015 | System MUST warn when disk space is low (< 500MB remaining) | P14b.3 | Offen |
| FR-016 | System MUST provide "Stop and Save" button to end recording | P14b.3 | Offen |
| FR-017 | System MUST allow downloading recordings to local file system | P14b.3 | Offen |
| SC-001 | Recording latency under 100ms from audio output to captured file | Q14 | Offen |
| SC-002 | File size calculations accurate within ±5% of actual file size | Q14 | Offen |
| SC-003 | WAV recordings maintain full 1411kbps/44.1kHz fidelity | Q14 | Offen |
| SC-004 | MP3 320kbps recordings indistinguishable from lossless in blind tests | Q14 | Offen |
| SC-005 | Auto-split creates seamless transitions with <10ms gap between files | Q14 | Offen |
| SC-006 | Pause/resume functionality maintains audio sync with <50ms drift | Q14 | Offen |
| SC-007 | Recording list displays and sorts 1000+ recordings without lag | Q14 | Offen |
| SC-008 | Storage usage calculation updates within 1 second of recording completion | Q14 | Offen |

## Spec 15: 15-live-streaming

Quelle und zusätzliche Abnahmeszenarien: [15-live-streaming/spec.md](15-live-streaming/spec.md). Zuordnung: FR → P15a/P15b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q15. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST support Twitch, YouTube Live, Facebook Live, and Custom RTMP streaming | P15a.1 | Offen |
| FR-002 | System MUST provide quality presets: Low (480p, 1.5Mbps), Medium (720p, 3Mbps), High (1080p, 6Mbps), Ultra (1080p60, 9Mbps) | P15a.1 | Offen |
| FR-003 | System MUST accept stream keys from platform dashboards | P15a.2 | Offen |
| FR-004 | System MUST accept custom RTMP server URLs for non-standard platforms | P15a.2 | Offen |
| FR-005 | System MUST display real-time viewer count | P15a.2 | Offen |
| FR-006 | System MUST display real-time connection stats: bitrate, FPS, frames dropped, connection quality | P15a.3 | Offen |
| FR-007 | System MUST test connection before going live | P15a.3 | Offen |
| FR-008 | System MUST integrate platform chat (Twitch, YouTube) with in-app chat panel | P15a.3 | Offen |
| FR-009 | System MUST allow sending chat messages as broadcaster | P15b.1 | Offen |
| FR-010 | System MUST support stream overlays: Now Playing, Logo, Social Media Links, Viewer Count, Chat Messages | P15b.1 | Offen |
| FR-011 | System MUST allow positioning and resizing overlays | P15b.1 | Offen |
| FR-012 | System MUST allow pausing and resuming stream | P15b.2 | Offen |
| FR-013 | System MUST save stream sessions with metadata: platform, duration, peak viewers, average bitrate, dropped frames | P15b.2 | Offen |
| FR-014 | System MUST provide stream session history with analytics | P15b.2 | Offen |
| FR-015 | System MUST set stream title, description, category/game, and tags | P15b.3 | Offen |
| FR-016 | System MUST warn when connection quality degrades | P15b.3 | Offen |
| FR-017 | System MUST automatically attempt reconnection on temporary disconnects | P15b.3 | Offen |
| SC-001 | Stream starts within 10 seconds of clicking "Go Live" | Q15 | Offen |
| SC-002 | Viewer count updates within 10 seconds of actual viewer changes | Q15 | Offen |
| SC-003 | Connection stats (bitrate, FPS, dropped frames) update every 1 second | Q15 | Offen |
| SC-004 | Chat messages appear within 2 seconds of being sent by viewers | Q15 | Offen |
| SC-005 | Stream quality maintains 95%+ of target bitrate with stable connection | Q15 | Offen |
| SC-006 | Dropped frames stay under 1% with adequate bandwidth | Q15 | Offen |
| SC-007 | Automatic reconnection succeeds within 30 seconds of disconnect | Q15 | Offen |
| SC-008 | Stream overlays render at 60 FPS without performance impact | Q15 | Offen |

## Spec 16: 16-performance-pads

Quelle und zusätzliche Abnahmeszenarien: [16-performance-pads/spec.md](16-performance-pads/spec.md). Zuordnung: FR → P16a/P16b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q16. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST provide 8 performance pads per deck | P16a.1 | Offen |
| FR-002 | System MUST support 6 pad modes: Hot Cue, Roll, Slicer, Sampler, Beat Jump, Saved Loop | P16a.1, P16b.3 | Offen |
| FR-003 | System MUST allow setting hot cues at any playback position | P16a.2 | Offen |
| FR-004 | System MUST instantly jump to hot cue position when triggered (<10ms latency) | P16a.2 | Offen |
| FR-005 | System MUST support right-click or long-press to clear hot cues | P16a.2 | Offen |
| FR-006 | System MUST provide 8 roll lengths: 1/16, 1/8, 1/4, 1/2, 1, 2, 4, 8 bars | P16a.3 | Offen |
| FR-007 | System MUST create seamless roll loops without audio clicks | P16a.3 | Offen |
| FR-008 | System MUST continue playback position correctly when roll is released | P16a.3 | Offen |
| FR-009 | System MUST divide slicer into 8 equal slices per bar | P16b.1 | Offen |
| FR-010 | System MUST trigger slicer slices with instant response | P16b.1 | Offen |
| FR-011 | System MUST support sampler mode with assigned sound effects | P16b.1 | Offen |
| FR-012 | System MUST provide beat jump distances: -32, -16, -8, -4, +4, +8, +16, +32 beats | P16b.2 | Offen |
| FR-013 | System MUST maintain beat sync accuracy during beat jumps | P16b.2 | Offen |
| FR-014 | System MUST save and recall loop positions for saved loop mode | P16b.2 | Offen |
| FR-015 | System MUST visually indicate active pads with color coding | P16b.3 | Offen |
| FR-016 | System MUST support velocity-sensitive triggering (if MIDI controller supports) | P16b.3 | Offen |
| FR-017 | System MUST prevent pad triggering lag during high CPU load | P16b.3 | Offen |
| SC-001 | Hot cue triggering latency under 10ms | Q16 | Offen |
| SC-002 | Roll loops create sample-perfect loops without audible clicks | Q16 | Offen |
| SC-003 | Beat jump accuracy within ±5ms of exact beat position | Q16 | Offen |
| SC-004 | Pad visual response time under 16ms (60 FPS) | Q16 | Offen |
| SC-005 | Slicer slices are perfectly quantized to beat grid | Q16 | Offen |
| SC-006 | System handles 10+ pad triggers per second without lag | Q16 | Offen |
| SC-007 | Mode switching completes within 100ms | Q16 | Offen |
| SC-008 | Hot cues persist across app sessions without data loss | Q16 | Offen |

## Spec 17: 17-beat-grid-sync

Quelle und zusätzliche Abnahmeszenarien: [17-beat-grid-sync/spec.md](17-beat-grid-sync/spec.md). Zuordnung: FR → P17a/P17b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q17. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST synchronize BPM between multiple decks | P17a.1 | Offen |
| FR-002 | System MUST designate master deck that controls sync tempo | P17a.1 | Offen |
| FR-003 | System MUST automatically adjust follower deck tempo to match master | P17a.2 | Offen |
| FR-004 | System MUST align phase so beats hit simultaneously | P17a.2 | Offen |
| FR-005 | System MUST provide beat grid offset adjustment (±0.5 seconds, 0.01s increments) | P17a.2 | Offen |
| FR-006 | System MUST allow setting downbeat (bar 1, beat 1) position | P17a.3 | Offen |
| FR-007 | System MUST lock beat grid to prevent accidental changes | P17a.3 | Offen |
| FR-008 | System MUST provide quantize toggle for beat-snapped actions | P17a.3 | Offen |
| FR-009 | System MUST display current beat position (1, 2, 3, 4 of bar) | P17b.1 | Offen |
| FR-010 | System MUST show beat phase indicator between synced decks | P17b.1 | Offen |
| FR-011 | System MUST detect and display BPM automatically | P17b.1 | Offen |
| FR-012 | System MUST allow manual BPM adjustment (0.01 BPM precision) | P17b.2 | Offen |
| FR-013 | System MUST provide tap tempo for manual BPM determination | P17b.2 | Offen |
| FR-014 | System MUST save beat grid adjustments per track | P17b.2 | Offen |
| FR-015 | System MUST display BPM difference between decks | P17b.3 | Offen |
| FR-016 | System MUST provide "Re-Analyze" function to re-detect BPM | P17b.3 | Offen |
| FR-017 | System MUST maintain sync accuracy over long playback durations (30+ minutes) | P17b.3 | Offen |
| SC-001 | BPM sync completes within 500ms of engaging sync button | Q17 | Offen |
| SC-002 | Phase alignment accurate within ±5ms | Q17 | Offen |
| SC-003 | Sync maintains accuracy within ±10ms over 30-minute playback | Q17 | Offen |
| SC-004 | Beat grid offset adjustment precision of 0.01 seconds | Q17 | Offen |
| SC-005 | BPM difference calculation updates in real-time (every 100ms) | Q17 | Offen |
| SC-006 | Visual beat indicators update with <16ms latency (60 FPS) | Q17 | Offen |
| SC-007 | Quantize snaps actions to beat within ±1ms | Q17 | Offen |
| SC-008 | Tap tempo calculates BPM with ±0.5 BPM accuracy from 4+ taps | Q17 | Offen |

## Spec 18: 18-live-remix-engine

Quelle und zusätzliche Abnahmeszenarien: [18-live-remix-engine/spec.md](18-live-remix-engine/spec.md). Zuordnung: FR → P18a/P18b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q18. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST slice tracks into audio segments based on beat grid | P18a.1 | Offen |
| FR-002 | System MUST provide 4 slicing modes: Beats (4/8/16), Bars (4/8/16), Phrases (16/32), Custom | P18a.1 | Offen |
| FR-003 | System MUST allow selecting multiple slices simultaneously | P18a.2 | Offen |
| FR-004 | System MUST support drag-and-drop slice reordering | P18a.2 | Offen |
| FR-005 | System MUST play arrangements seamlessly without audio gaps | P18a.2 | Offen |
| FR-006 | System MUST maintain slice timing accuracy within ±5ms | P18a.3 | Offen |
| FR-007 | System MUST save slice arrangements as named patterns | P18a.3 | Offen |
| FR-008 | System MUST allow loading and applying saved patterns | P18a.3 | Offen |
| FR-009 | System MUST record live slice triggering with timestamps | P18b.1 | Offen |
| FR-010 | System MUST playback recorded performances exactly | P18b.1 | Offen |
| FR-011 | System MUST apply effects independently to each slice | P18b.1 | Offen |
| FR-012 | System MUST support slice effects: Reverb, Delay, Filter, Pitch Shift, Reverse | P18b.2 | Offen |
| FR-013 | System MUST clone, merge, and delete slices | P18b.2 | Offen |
| FR-014 | System MUST display waveform with slice boundaries | P18b.2 | Offen |
| FR-015 | System MUST save complete remix setups as presets | P18b.3 | Offen |
| FR-016 | System MUST export/import remix data as JSON | P18b.3 | Offen |
| FR-017 | System MUST visualize playback position across slices | P18b.3 | Offen |
| SC-001 | Auto-slicing completes within 5 seconds for typical 4-minute tracks | Q18 | Offen |
| SC-002 | Slice transitions maintain sample-perfect accuracy (<1ms gaps) | Q18 | Offen |
| SC-003 | Pattern loading restores arrangement within 500ms | Q18 | Offen |
| SC-004 | Performance recording captures actions with <10ms timestamp precision | Q18 | Offen |
| SC-005 | Playback follows arrangement without audible glitches at 60 FPS | Q18 | Offen |
| SC-006 | Effects apply to slices without increasing latency beyond 20ms | Q18 | Offen |
| SC-007 | Preset loading restores complete setup within 2 seconds | Q18 | Offen |
| SC-008 | System handles 100+ slices per track without performance degradation | Q18 | Offen |

## Spec 19: 19-sampler-section

Quelle und zusätzliche Abnahmeszenarien: [19-sampler-section/spec.md](19-sampler-section/spec.md). Zuordnung: FR → P19a/P19b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q19. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST provide 8 sample pads | P19a.1 | Offen |
| FR-002 | System MUST support one-shot and loop playback modes per pad | P19a.1 | Offen |
| FR-003 | System MUST allow independent volume control per pad (0-100%) | P19a.2 | Offen |
| FR-004 | System MUST trigger samples with <10ms latency | P19a.2 | Offen |
| FR-005 | System MUST support polyphonic sample playback (all 8 pads simultaneously) | P19a.2 | Offen |
| FR-006 | System MUST provide sample library browser modal | P19a.3 | Offen |
| FR-007 | System MUST display sample name and duration on each pad | P19a.3 | Offen |
| FR-008 | System MUST support "Stop All" button to silence all playing samples | P19a.3 | Offen |
| FR-009 | System MUST support "Loop All" button to enable loop mode on all pads | P19b.1 | Offen |
| FR-010 | System MUST color-code pads based on sample properties | P19b.1 | Offen |
| FR-011 | System MUST show visual feedback when pad is triggered (animation/glow) | P19b.1 | Offen |
| FR-012 | System MUST persist loaded samples across sessions | P19b.2 | Offen |
| FR-013 | System MUST support sample categorization (drums, vocals, FX, percussion, etc.) | P19b.2 | Offen |
| FR-014 | System MUST allow clearing/unloading samples from pads | P19b.2 | Offen |
| FR-015 | System MUST prevent audio clipping when multiple samples play | P19b.3 | Offen |
| FR-016 | System MUST provide keyboard shortcuts for triggering pads (1-8 keys) | P19b.3 | Offen |
| FR-017 | System MUST support custom sample upload | P19b.3 | Offen |
| SC-001 | Sample triggering latency under 10ms | Q19 | Offen |
| SC-002 | Sample library loads and displays within 500ms | Q19 | Offen |
| SC-003 | Volume changes apply within 50ms | Q19 | Offen |
| SC-004 | "Stop All" silences all samples within 10ms | Q19 | Offen |
| SC-005 | System supports 8 simultaneous samples without audio dropouts | Q19 | Offen |
| SC-006 | Sample loading to pad completes within 1 second | Q19 | Offen |
| SC-007 | Visual pad feedback (animation) responds within 16ms (60 FPS) | Q19 | Offen |
| SC-008 | Sample library supports 500+ samples without search lag | Q19 | Offen |

## Spec 20: 20-multi-deck-control

Quelle und zusätzliche Abnahmeszenarien: [20-multi-deck-control/spec.md](20-multi-deck-control/spec.md). Zuordnung: FR → P20a/P20b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q20. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST support 2-deck and 4-deck mixer modes | P20a.1 | Offen |
| FR-002 | System MUST provide independent volume faders per channel (0-100%) | P20a.1 | Offen |
| FR-003 | System MUST provide 3-band EQ per channel: High, Mid, Low (0-100%, center = 50%) | P20a.2 | Offen |
| FR-004 | System MUST provide independent gain control per channel (0-100%) | P20a.2 | Offen |
| FR-005 | System MUST support crossfader with assignable routing (Left/Center/Right per deck) | P20a.2 | Offen |
| FR-006 | System MUST provide mute button per channel | P20a.3 | Offen |
| FR-007 | System MUST provide solo button per channel | P20a.3 | Offen |
| FR-008 | System MUST provide master volume control (0-100%) | P20a.3 | Offen |
| FR-009 | System MUST provide headphone cue system with cue/master mix control | P20b.1 | Offen |
| FR-010 | System MUST display level meters per channel | P20b.1 | Offen |
| FR-011 | System MUST display master output level meter | P20b.1 | Offen |
| FR-012 | System MUST prevent audio clipping with master limiter | P20b.2 | Offen |
| FR-013 | System MUST route deck audio to assigned mixer channels | P20b.2 | Offen |
| FR-014 | System MUST allow resetting channel to default settings | P20b.2 | Offen |
| FR-015 | System MUST clear all solo states with "Clear Solos" button | P20b.3 | Offen |
| FR-016 | System MUST visually indicate active channels (playing, not muted, audible) | P20b.3 | Offen |
| FR-017 | System MUST persist mixer settings across sessions | P20b.3 | Offen |
| SC-001 | Mode switching (2-deck ↔ 4-deck) completes within 200ms | Q20 | Offen |
| SC-002 | Fader adjustments apply within 10ms | Q20 | Offen |
| SC-003 | EQ changes apply within 20ms | Q20 | Offen |
| SC-004 | Mute/solo toggles respond within 10ms | Q20 | Offen |
| SC-005 | Level meters update at 30+ FPS | Q20 | Offen |
| SC-006 | 4-deck mixing maintains CPU usage under 30% on mid-range hardware | Q20 | Offen |
| SC-007 | Master limiter prevents clipping with <1ms attack time | Q20 | Offen |
| SC-008 | Crossfader curve feels smooth across entire range (no jumps or steps) | Q20 | Offen |

## Spec 21: 21-track-preparation

Quelle und zusätzliche Abnahmeszenarien: [21-track-preparation/spec.md](21-track-preparation/spec.md). Zuordnung: FR → P21a/P21b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q21. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-001 | System MUST analyze tracks for BPM, key, energy level, and danceability | P21a.1 | Offen |
| FR-002 | System MUST generate beat markers at every beat with strength indicators | P21a.1 | Offen |
| FR-003 | System MUST display analysis progress (0-100%) | P21a.2 | Offen |
| FR-004 | System MUST allow manual BPM adjustment with 0.01 BPM precision | P21a.2 | Offen |
| FR-005 | System MUST allow beat grid offset adjustment with 0.01 second precision | P21a.2 | Offen |
| FR-006 | System MUST allow setting downbeat (first beat) position | P21a.3 | Offen |
| FR-007 | System MUST create, edit, and delete cue points | P21a.3 | Offen |
| FR-008 | System MUST assign colors and labels to cue points | P21a.3 | Offen |
| FR-009 | System MUST jump playback to cue point positions | P21b.1 | Offen |
| FR-010 | System MUST detect musical key with confidence score | P21b.1 | Offen |
| FR-011 | System MUST allow manual key override selection | P21b.1 | Offen |
| FR-012 | System MUST edit metadata: title, artist, album, genre, year, comments | P21b.2 | Offen |
| FR-013 | System MUST display waveform with overlaid cue points and beat markers | P21b.2 | Offen |
| FR-014 | System MUST save all analysis data and cue points persistently | P21b.2 | Offen |
| FR-015 | System MUST display track preparation status (analyzed, cues set, grid locked) | P21b.3 | Offen |
| FR-016 | System MUST support batch analysis of multiple tracks | P21b.3 | Offen |
| FR-017 | System MUST re-analyze tracks with "Re-Analyze" button | P21b.3 | Offen |
| SC-001 | Track analysis completes within 10 seconds for typical 4-minute tracks | Q21 | Offen |
| SC-002 | BPM detection accuracy of 95%+ for electronic/dance music | Q21 | Offen |
| SC-003 | Key detection accuracy of 85%+ with confidence scores | Q21 | Offen |
| SC-004 | Beat grid adjustment responds within 50ms to slider changes | Q21 | Offen |
| SC-005 | Cue point creation and jumping latency under 100ms | Q21 | Offen |
| SC-006 | Waveform with overlays renders at 30+ FPS | Q21 | Offen |
| SC-007 | Batch analysis processes 20+ tracks without UI freezing | Q21 | Offen |
| SC-008 | Analysis data saves and loads within 200ms | Q21 | Offen |

## Spec 22: 22-neural-sync-engine

Quelle und zusätzliche Abnahmeszenarien: [22-neural-sync-engine/spec.md](22-neural-sync-engine/spec.md). Zuordnung: FR → P22a/P22b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q22. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-022-001 | Track Analysis — The system SHALL analyze loaded tracks to detect:<br>- Musical phrases with type classification (intro, verse, chorus, breakdown, buildup, drop, bridge, outro)<br>- Beat grid with precise downbeat markers<br>- Energy levels per phrase (low/medium/high/peak)<br>- Phrase boundaries with confidence scores<br>- Track duration and overall structure | P22a.1 | Offen |
| FR-022-002 | Phrase Detection — The system SHALL classify phrases based on:<br>- Spectral content analysis<br>- Energy level variations<br>- Repetition patterns<br>- Bar count (typically 4, 8, 16, or 32 bars)<br>- Musical context (position in track) | P22a.1 | Offen |
| FR-022-003 | Sync Suggestion Generation — The system SHALL generate sync suggestions containing:<br>- Source phrase (from-track) and target phrase (to-track)<br>- Exact timing positions for both decks<br>- Overall confidence score (0-100%)<br>- Energy match percentage<br>- Key compatibility percentage<br>- Phrase alignment score<br>- Suggested transition description | P22a.2 | Offen |
| FR-022-004 | Suggestion Ranking — The system SHALL rank sync suggestions by:<br>- Overall confidence score (weighted combination)<br>- Phrase type compatibility<br>- Energy level matching<br>- Key compatibility (if key sync enabled)<br>- Temporal proximity to current playback position | P22a.2 | Offen |
| FR-022-005 | Visual Phrase Display — The system SHALL provide waveform visualizations showing:<br>- Phrase regions color-coded by type<br>- Phrase type labels<br>- Current playhead position<br>- Downbeat markers<br>- Bar numbers within phrases | P22a.3 | Offen |
| FR-022-006 | Auto Sync Application — The system SHALL support one-click auto sync that:<br>- Applies the highest-rated sync suggestion<br>- Adjusts deck positions to sync points<br>- Calculates tempo adjustment if needed<br>- Provides smooth transition between positions<br>- Validates sync was successful | P22a.3 | Offen |
| FR-022-007 | Sync Modes — The system SHALL support multiple sync modes:<br>- **Beat Sync**: Lock to beat boundaries<br>- **Bar Sync**: Lock to bar (4-beat) boundaries<br>- **Phrase Sync**: Lock to phrase boundaries<br>- **Smart Sync**: AI-determined optimal sync mode | P22a.3 | Offen |
| FR-022-008 | Real-Time Phrase Tracking — The system SHALL display for each deck:<br>- Current phrase type and name<br>- Current bar position within phrase<br>- Total bars in phrase<br>- Time remaining in phrase<br>- Next phrase preview | P22b.1 | Offen |
| FR-022-009 | Energy Matching — When energy matching is enabled, the system SHALL:<br>- Calculate energy profiles for both tracks<br>- Score transitions based on energy compatibility<br>- Filter suggestions below tolerance threshold<br>- Provide visual energy flow indicators<br>- Warn about dramatic energy shifts | P22b.1 | Offen |
| FR-022-010 | Key Sync Integration — When key sync is enabled, the system SHALL:<br>- Integrate with key detection system<br>- Prioritize harmonically compatible transitions<br>- Adjust suggestion scores based on key relationships<br>- Display key compatibility in suggestions<br>- Support Camelot wheel matching rules | P22b.2 | Offen |
| FR-022-011 | Phrase Alignment Scoring — The system SHALL score phrase alignment based on:<br>- Phrase type compatibility (matching or complementary)<br>- Bar alignment (phrases starting on downbeats)<br>- Musical structure logic (verse→chorus, buildup→drop)<br>- Transition smoothness prediction<br>- Historical success rates of similar transitions | P22b.2 | Offen |
| FR-022-012 | Sync Tolerance Settings — The system SHALL allow users to configure:<br>- Tolerance percentage (0-100%)<br>- Lower values = stricter matching, fewer suggestions<br>- Higher values = more lenient, more suggestions<br>- Separate tolerances for energy, key, and phrase matching<br>- Global tolerance affecting all sync calculations | P22b.2 | Offen |
| FR-022-013 | Performance Metrics — The system SHALL track and display:<br>- Analysis time per track<br>- Number of phrases detected<br>- Sync suggestion count<br>- Applied suggestion success rate<br>- Average confidence scores | P22b.3 | Offen |
| FR-022-014 | Waveform Rendering — The system SHALL render phrase-annotated waveforms with:<br>- 800x100 pixel canvas resolution<br>- Color-coded phrase regions with transparency<br>- Waveform overlay showing amplitude<br>- Playhead indicator<br>- Downbeat markers<br>- Phrase type labels (when space allows) | P22b.3 | Offen |
| FR-022-015 | Settings Persistence — The system SHALL persist user preferences:<br>- Sync mode selection<br>- Key sync enabled/disabled<br>- Phrase sync enabled/disabled<br>- Energy matching enabled/disabled<br>- Tolerance level<br>- Last used configuration | P22b.3 | Offen |
| SC-022-001 | Analysis Performance — Track analysis completes within 2 seconds for typical 4-minute tracks | Q22 | Offen |
| SC-022-002 | Phrase Detection Accuracy — System detects major phrase boundaries with 85% accuracy compared to manual annotation | Q22 | Offen |
| SC-022-003 | Sync Quality — Applied sync suggestions result in beatmatched transitions with <10ms timing error | Q22 | Offen |
| SC-022-004 | Suggestion Relevance — Top 3 sync suggestions score above 70% confidence for compatible track pairs | Q22 | Offen |
| SC-022-005 | Response Time — Sync suggestion generation completes within 500ms after both tracks are analyzed | Q22 | Offen |
| SC-022-006 | Visual Performance — Waveform visualization renders at 30 FPS during playback with no dropped frames | Q22 | Offen |
| SC-022-007 | Energy Match Accuracy — Energy matching filters correctly identify energy-compatible transitions with 90% relevance | Q22 | Offen |
| SC-022-008 | User Adoption — DJs use Neural Sync Engine for 50%+ of their transitions after initial learning period | Q22 | Offen |

## Spec 23: 23-mix-history-analytics

Quelle und zusätzliche Abnahmeszenarien: [23-mix-history-analytics/spec.md](23-mix-history-analytics/spec.md). Zuordnung: FR → P23a/P23b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q23. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-023-001 | Session Creation — The system SHALL automatically create a mix session when:<br>- First track is loaded to any deck<br>- Manual "Start Session" button is pressed<br>- Application starts with tracks already loaded | P23a.1 | Offen |
| FR-023-002 | Session Metadata — The system SHALL capture for each session:<br>- Unique session ID<br>- Session name (auto-generated or user-provided)<br>- Start and end timestamps<br>- Total duration in seconds<br>- Track count<br>- Venue name (optional)<br>- Total listener count (optional)<br>- Recording status and file path (if recorded) | P23a.1 | Offen |
| FR-023-003 | Transition Recording — The system SHALL record transitions containing:<br>- Source track metadata (title, artist, BPM, key, genre)<br>- Destination track metadata<br>- Transition timestamp<br>- Transition type (crossfade/echo/spin/brake/instant)<br>- Transition duration (seconds)<br>- BPM difference (absolute value)<br>- Key compatibility flag (boolean)<br>- Crossfader position at transition point<br>- Quality score (0-100) | P23a.2 | Offen |
| FR-023-004 | Quality Scoring Algorithm — The system SHALL calculate transition quality based on:<br>- Beat matching accuracy (±2 BPM = higher score)<br>- Key compatibility (harmonic = bonus points)<br>- Transition smoothness (appropriate duration)<br>- Phrase alignment (if available)<br>- EQ and volume balance<br>- Base score: 100, with deductions for issues | P23a.2 | Offen |
| FR-023-005 | Session Statistics — The system SHALL calculate per-session:<br>- Average BPM across all tracks<br>- Average transition quality score<br>- Peak energy level (0-100)<br>- Genre distribution (percentage per genre)<br>- Transition type distribution<br>- Total duration<br>- Most frequently used tracks | P23a.3 | Offen |
| FR-023-006 | Session List Display — The system SHALL display sessions with:<br>- Date and time formatting (weekday, month, day, time)<br>- Duration in hours/minutes format<br>- Track count<br>- Quality score with color coding (green ≥85, blue ≥70, yellow ≥60, red <60)<br>- Peak energy indicator with progress bar<br>- Genre tags<br>- Venue badge (if applicable)<br>- Recording badge (if recorded)<br>- Listener count (if tracked) | P23a.3 | Offen |
| FR-023-007 | Session Filtering — The system SHALL support filtering by:<br>- Date range<br>- Genre<br>- Minimum track count<br>- Minimum quality score<br>- Venue<br>- Recording status | P23a.3 | Offen |
| FR-023-008 | Session Sorting — The system SHALL support sorting by:<br>- Date (newest/oldest first)<br>- Duration (longest/shortest first)<br>- Quality score (highest/lowest first)<br>- Track count (most/least first) | P23b.1 | Offen |
| FR-023-009 | Detailed Transition View — For each transition, the system SHALL display:<br>- Transition number in sequence<br>- Source and destination track details<br>- Transition type icon and label<br>- Transition duration with precision to 0.1 seconds<br>- BPM difference warning (if >5 BPM)<br>- Key compatibility indicator (checkmark if compatible)<br>- Quality score with color-coded badge<br>- Exact timestamp of transition | P23b.1 | Offen |
| FR-023-010 | Session Deletion — The system SHALL allow users to:<br>- Delete individual sessions with confirmation<br>- Permanently remove session data<br>- Update session list immediately after deletion<br>- Preserve recording files if user opts in | P23b.2 | Offen |
| FR-023-011 | Overall Statistics — The system SHALL display aggregate statistics:<br>- Total sessions count<br>- Total mixing time (all sessions combined)<br>- Total tracks played (unique count)<br>- Average quality score across all sessions<br>- Most common genres (top 5)<br>- Session frequency (sessions per week/month) | P23b.2 | Offen |
| FR-023-012 | Data Export — The system SHALL support exporting:<br>- Single session as JSON<br>- Single session as CSV<br>- All sessions as JSON archive<br>- Session summary report as PDF<br>- Recording audio file link (if available) | P23b.2 | Offen |
| FR-023-013 | Session Replay — The system SHALL provide session replay showing:<br>- Transition timeline visualization<br>- Track progression flow<br>- Quality score trends<br>- Energy level changes<br>- Ability to load specific tracks from history | P23b.3 | Offen |
| FR-023-014 | Data Persistence — The system SHALL:<br>- Store sessions in local database/storage<br>- Automatically save transitions as they occur<br>- Persist across application restarts<br>- Support backup and restore operations<br>- Limit history to configurable number of sessions (default 100) | P23b.3 | Offen |
| FR-023-015 | Performance Tracking — The system SHALL track over time:<br>- Quality score trends<br>- Transition technique improvement<br>- Genre preference evolution<br>- Average session length changes<br>- Track variety metrics | P23b.3 | Offen |
| SC-023-001 | Recording Reliability — 100% of transitions are captured and recorded without data loss during normal operation | Q23 | Offen |
| SC-023-002 | Storage Efficiency — Session data storage uses less than 1KB per transition on average | Q23 | Offen |
| SC-023-003 | Performance Impact — Mix history recording adds less than 5% CPU overhead during active mixing | Q23 | Offen |
| SC-023-004 | Query Performance — Session list loads within 200ms for 100+ sessions | Q23 | Offen |
| SC-023-005 | Quality Score Accuracy — Automated quality scores correlate with DJ self-assessment at 80% agreement rate | Q23 | Offen |
| SC-023-006 | Export Speed — Session export completes within 1 second for typical session (30 transitions) | Q23 | Offen |
| SC-023-007 | Data Retention — System maintains complete session history for minimum 6 months without degradation | Q23 | Offen |
| SC-023-008 | User Engagement — DJs review their mix history at least once per week on average | Q23 | Offen |

## Spec 24: 24-smart-playlist-manager

Quelle und zusätzliche Abnahmeszenarien: [24-smart-playlist-manager/spec.md](24-smart-playlist-manager/spec.md). Zuordnung: FR → P24a/P24b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q24. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-024-001 | Playlist Types — The system SHALL support five playlist types:<br>- **Energy Flow**: Sequences tracks by energy level progression<br>- **Harmonic Mixing**: Prioritizes key-compatible transitions<br>- **BPM Progression**: Orders tracks by BPM changes<br>- **Genre-Based**: Groups tracks by genre with smooth transitions<br>- **Mood-Based**: Sequences tracks by mood/vibe similarity | P24a.1 | Offen |
| FR-024-002 | Playlist Criteria — The system SHALL support criteria configuration:<br>- BPM range with min/max values (80-180 range)<br>- Key matching toggle (enable/disable)<br>- Genre selection (multi-select from available genres)<br>- Minimum rating filter (0-5 stars)<br>- Energy flow pattern selection<br>- Exclude recently played option (configurable time window)<br>- Maximum playlist duration limit (optional) | P24a.1 | Offen |
| FR-024-003 | Track Filtering — The system SHALL filter tracks based on:<br>- BPM within specified range<br>- Genre matches selected list (if specified)<br>- Rating meets minimum threshold<br>- Not in exclude list (if enabled)<br>- Has required metadata (BPM, key if key matching enabled)<br>- File accessible and playable | P24a.2 | Offen |
| FR-024-004 | Track Sequencing — The system SHALL sequence tracks using algorithms for:<br>- **Energy Flow**: Sort by energy level, apply smoothing<br>- **Harmonic**: Pathfinding through Camelot wheel connections<br>- **BPM**: Gradual BPM changes with configurable step size<br>- **Genre**: Group by genre, smooth transitions at boundaries<br>- **Mood**: Clustering by mood attributes with transition optimization | P24a.2 | Offen |
| FR-024-005 | Energy Flow Patterns — The system SHALL implement four energy patterns:<br>- **Buildup**: Ascending sort from low to high energy<br>- **Cooldown**: Descending sort from high to low energy<br>- **Plateau**: Group tracks within narrow energy range<br>- **Wave**: Alternating high-low pattern with gradual changes | P24a.3 | Offen |
| FR-024-006 | Track Recommendations — The system SHALL generate recommendations by:<br>- Analyzing current track attributes (BPM, key, genre, energy)<br>- Scoring library tracks against current track (0-100)<br>- Applying weighted scoring: key (30%), BPM (25%), energy (20%), genre (15%), rating (10%)<br>- Filtering out recently played tracks<br>- Limiting results to top 10 matches<br>- Providing match reasons (e.g., "Same key", "Similar BPM", "Energy match") | P24a.3 | Offen |
| FR-024-007 | Match Indicators — The system SHALL flag recommendations with:<br>- Harmonic match badge (key compatible)<br>- BPM match badge (within ±5 BPM)<br>- Energy match badge (similar energy level)<br>- Genre match badge (same genre)<br>- Overall match score (0-100 scale) | P24a.3 | Offen |
| FR-024-008 | Playlist Generation — The system SHALL generate playlists by:<br>- Applying all filter criteria<br>- Running appropriate sequencing algorithm<br>- Calculating playlist metadata (duration, avg BPM, energy)<br>- Storing playlist in database<br>- Assigning unique playlist ID<br>- Displaying generation timestamp | P24b.1 | Offen |
| FR-024-009 | Playlist Storage — The system SHALL store for each playlist:<br>- Unique ID<br>- Name and description (optional)<br>- Playlist type<br>- Creation timestamp<br>- Criteria used for generation<br>- Ordered list of track IDs<br>- Calculated metadata (duration, avgBpm, energyFlow array)<br>- Auto-update settings (if applicable) | P24b.1 | Offen |
| FR-024-010 | Playlist Display — The system SHALL display playlists with:<br>- Playlist name and type icon<br>- Track count and total duration<br>- Average BPM<br>- Type-specific metadata<br>- Mini energy flow graph (for energy playlists)<br>- Creation/update timestamp<br>- Edit and delete actions | P24b.2 | Offen |
| FR-024-011 | Track List View — For selected playlist, the system SHALL display:<br>- Ordered track list with sequence numbers<br>- Track title and artist<br>- BPM, key, and genre for each track<br>- Rating stars<br>- Load to deck buttons (A/B)<br>- Color coding for transitions (green = smooth, yellow = caution, red = difficult) | P24b.2 | Offen |
| FR-024-012 | Playlist Regeneration — The system SHALL allow users to:<br>- Regenerate playlist with same criteria<br>- Update playlist with new library additions<br>- Refresh based on current filters<br>- Preserve playlist name and settings<br>- Update generation timestamp | P24b.2 | Offen |
| FR-024-013 | Playlist Analysis — The system SHALL analyze playlists for:<br>- Energy profile (min/max/average/flow pattern)<br>- BPM profile (min/max/average/variance)<br>- Transition quality predictions (0-100 per transition)<br>- Estimated set duration<br>- Genre diversity score<br>- Weak transition identification | P24b.3 | Offen |
| FR-024-014 | Transition Quality Prediction — The system SHALL predict transition quality based on:<br>- BPM difference (closer = higher score)<br>- Key compatibility (harmonic = bonus)<br>- Energy level changes (gradual = higher score)<br>- Genre compatibility<br>- Historical transition success data (if available)<br>- Score range: 0-100 | P24b.3 | Offen |
| FR-024-015 | Playlist Deletion — The system SHALL support:<br>- Delete playlist with confirmation dialog<br>- Preserve library tracks (only delete playlist reference)<br>- Remove from playlist list immediately<br>- Cleanup associated metadata | P24b.3 | Offen |
| SC-024-001 | Generation Speed — Smart playlist generation completes within 3 seconds for 1000-track library | Q24 | Offen |
| SC-024-002 | Recommendation Relevance — Track recommendations score 80%+ relevance based on user feedback (accept/reject tracking) | Q24 | Offen |
| SC-024-003 | Transition Quality — Generated playlists have 85%+ smooth transitions (quality score ≥70) | Q24 | Offen |
| SC-024-004 | Energy Flow Accuracy — Energy-based playlists follow selected pattern with 95% adherence to intended flow | Q24 | Offen |
| SC-024-005 | Harmonic Accuracy — Harmonic playlists maintain 90%+ key-compatible transitions following Camelot rules | Q24 | Offen |
| SC-024-006 | Storage Efficiency — Playlist metadata uses less than 5KB per playlist on average | Q24 | Offen |
| SC-024-007 | Update Performance — Playlist regeneration completes within 2 seconds for existing playlists | Q24 | Offen |
| SC-024-008 | User Adoption — DJs create and use 3+ smart playlists on average within first month | Q24 | Offen |

## Spec 25: 25-enhanced-harmonic-mixing

Quelle und zusätzliche Abnahmeszenarien: [25-enhanced-harmonic-mixing/spec.md](25-enhanced-harmonic-mixing/spec.md). Zuordnung: FR → P25a/P25b (genaue Teilmengen und Ausführungsslices im Plan); SC → Q25. Gemeinsame Grundlagen und Abhängigkeiten siehe [Plan](IMPLEMENTATION_PLAN.md).

| ID | Originalanforderung / Erfolgskriterium | Zuständiger Slice | Nachweisstatus |
|---|---|---|---|
| FR-025-001 | Camelot Wheel Data Model — The system SHALL maintain Camelot wheel mapping with:<br>- 24 positions (12 major + 12 minor keys)<br>- Camelot codes (1A-12A for minor, 1B-12B for major)<br>- Musical key names (C, C#, D, Eb, E, F, F#, G, Ab, A, Bb, B with major/minor)<br>- Visual coordinates for 600x600px wheel rendering<br>- Color assignments per position for visual distinction | P25a.1 | Offen |
| FR-025-002 | Wheel Visualization — The system SHALL render interactive Camelot wheel with:<br>- SVG-based scalable graphics<br>- 600x600 pixel default size<br>- Outer circle radius: 260px (major keys)<br>- Inner circle radius: 160px (minor keys)<br>- Clickable key positions (28px radius circles)<br>- Connection lines between adjacent keys<br>- Responsive hover states<br>- Selected key highlighting (35px radius)<br>- Current deck highlighting with pulsing animation | P25a.1 | Offen |
| FR-025-003 | Key Compatibility Rules — The system SHALL implement Camelot key compatibility rules:<br>- **Perfect (Easy)**: Same key, ±1 position on wheel, relative major/minor<br>- **Energy Boost (Medium)**: +1 semitone (e.g., 8B / C major → 3B / C♯ major)<br>- **Energy Drop (Medium)**: -1 semitone (e.g., 8B / C major → 1B / B major)<br>- **Dramatic (Hard)**: ±2+ semitones or opposite wheel positions<br>- Major/minor switches within same Camelot number<br>- When classifications overlap, **Perfect (Easy)** takes precedence over **Dramatic (Hard)**. Display the actual semitone interval as additional information; a Camelot wheel step is not a semitone step. | P25a.2 | Offen |
| FR-025-004 | Transition Classification — For each key pair, the system SHALL determine:<br>- Transition type: 'perfect' &#124; 'energy-boost' &#124; 'energy-drop' &#124; 'dramatic'<br>- Difficulty level: 'easy' &#124; 'medium' &#124; 'hard'<br>- Musical relationship (e.g., "Perfect fifth", "Relative minor")<br>- Mixing guidance description<br>- Compatibility score (implied by type/difficulty) | P25a.2 | Offen |
| FR-025-005 | Compatible Keys List — When a key is selected, the system SHALL generate list of:<br>- 6-7 most compatible keys (prioritizing easy transitions)<br>- Each entry containing: target key, transition type, difficulty, description<br>- Sorted by difficulty (easy first, then medium, then hard)<br>- Visual indicators matching Camelot wheel colors | P25a.3 | Offen |
| FR-025-006 | Track Finder — The system SHALL provide track finder functionality:<br>- Accepts source deck selection (A or B)<br>- Retrieves source track's key<br>- Generates list of compatible keys using Camelot rules<br>- Filters library tracks matching compatible keys<br>- Excludes source track from results<br>- Sorts by difficulty (easy transitions first)<br>- Limits results to reasonable count (50-100 tracks) | P25a.3 | Offen |
| FR-025-007 | Track Finder Display — For each found track, the system SHALL display:<br>- Track title and artist name<br>- Musical key with colored badge<br>- BPM with colored badge<br>- Genre label<br>- Transition difficulty badge (easy/medium/hard)<br>- Transition type label (perfect/energy-boost/energy-drop)<br>- Transition description<br>- Load to Deck A/B action buttons | P25a.3 | Offen |
| FR-025-008 | Deck Integration — The system SHALL integrate with deck state:<br>- Monitor current track on Deck A<br>- Monitor current track on Deck B<br>- Display current keys in header panel<br>- Highlight corresponding keys on Camelot wheel<br>- Update track finder source based on selected deck<br>- Provide deck-specific color coding (blue/orange) | P25b.1 | Offen |
| FR-025-009 | Key Distribution Analytics — The system SHALL analyze library and display:<br>- Track count per key (all 24 keys)<br>- Percentage of library per key<br>- Visual bar charts for distribution<br>- Color-coded bars matching Camelot wheel<br>- Top 5 most common keys ranked list<br>- Balanced/unbalanced library indicators | P25b.1 | Offen |
| FR-025-010 | Key Analyzer View — The system SHALL provide key analyzer view with:<br>- 6x4 grid (or 8x3) showing all keys<br>- Per-key statistics: name, Camelot code, count, percentage, bar chart<br>- Color-coded borders matching wheel colors<br>- Interactive selection (clicking loads track finder for that key)<br>- Distribution visualization<br>- Library balance score (0-100) | P25b.2 | Offen |
| FR-025-011 | Mixing Strategy Tips — The system SHALL display four recommendation categories:<br>- **Perfect Match** (green card): Same key, adjacent positions, relative major/minor<br>- **Energy Shift** (yellow card): Semitone up/down for energy changes<br>- **Major/Minor Switch** (blue card): Inner/outer circle for mood variation<br>- **Advanced Technique** (red card): Complex jumps requiring skill<br>- Each with icon, title, and practical guidance text | P25b.2 | Offen |
| FR-025-012 | View Modes — The system SHALL support three view modes:<br>- **Wheel**: Camelot wheel visualization with key info panel<br>- **Finder**: Track finder with filtering and deck selection<br>- **Analyzer**: Key distribution analytics and library statistics<br>- Toggle buttons to switch between modes<br>- Persistent view state during session | P25b.2 | Offen |
| FR-025-013 | Interactive Elements — The system SHALL support user interactions:<br>- Click key on wheel → select key and show info<br>- Click track in finder → option to load to deck A or B<br>- Hover over key → visual feedback (opacity change)<br>- Deck selection toggle (A/B) in finder mode<br>- View mode switching buttons | P25b.3 | Offen |
| FR-025-014 | Color Coding — The system SHALL use consistent color coding:<br>- Deck A: Blue (#3b82f6)<br>- Deck B: Orange (#f97316)<br>- Perfect transitions: Green<br>- Energy boost: Yellow/Orange<br>- Energy drop: Purple<br>- Dramatic shifts: Red<br>- Each Camelot position: Unique color from spectrum | P25b.3 | Offen |
| FR-025-015 | Performance Optimization — The system SHALL optimize performance by:<br>- Memoizing track finder results for same source key<br>- Caching key distribution analytics<br>- Using efficient SVG rendering for wheel<br>- Debouncing wheel interactions<br>- Lazy loading track lists (virtualization for 100+ tracks) | P25b.3 | Offen |
| SC-025-001 | Visual Clarity — 95% of test users can identify their current track's key position on the Camelot wheel within 5 seconds | Q25 | Offen |
| SC-025-002 | Recommendation Accuracy — Compatible key suggestions result in 90%+ harmonically smooth transitions as rated by DJs | Q25 | Offen |
| SC-025-003 | Response Time — Track finder returns results within 300ms for libraries up to 5000 tracks | Q25 | Offen |
| SC-025-004 | Key Detection Accuracy — Key distribution analytics accurately reflect library composition with 100% precision (no counting errors) | Q25 | Offen |
| SC-025-005 | User Comprehension — 80% of novice DJs understand Camelot wheel concept after 10 minutes of use | Q25 | Offen |
| SC-025-006 | Interaction Performance — Wheel interactions (clicks, hovers) respond within 50ms with no perceived lag | Q25 | Offen |
| SC-025-007 | Mixing Improvement — DJs using harmonic mixing feature report 40% increase in transition smoothness after 1 month | Q25 | Offen |
| SC-025-008 | Feature Adoption — 60% of users enable and regularly use harmonic mixing features within first week | Q25 | Offen |
