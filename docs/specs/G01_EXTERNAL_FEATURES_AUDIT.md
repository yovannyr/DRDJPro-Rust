# G01 – Abgleich von zehn Funktionsbereichen ohne belegten End-to-End-Pfad

Quellstand `cd2261e`, statischer Abgleich am 08.11.2026. Grundlage sind die 25 Original-Specs, die [bisherige Analyse](MISSING_FEATURES_ANALYSIS.md), die Projektdateien in `src/`, `ui/`, `Cargo.toml` und die bereichsspezifischen Referenzen unten. Eine Suche nach eindeutigen Provider-, Timecode-, Stem-, Video-, Aufnahme-, Streaming- und Automixbegriffen ergab keinen vollständigen Funktionspfad. Eine Textsuche kann verdeckte Implementierung nicht ausschließen.

**Status „offen“:** kein vollständiger End-to-End-Nachweis für diese konkrete Anforderung; dies ist keine Behauptung, dass jeder Teilbaustein fehlt. Original-User-Stories und Edge Cases bleiben zusätzlich verbindlich. Alle acht SC pro Bereich sind ohne die jeweiligen Laufzeit-, Provider-, Nutzer-, Hardware- oder Langzeitmessungen offen. Der letzte dokumentierte Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; danach wurden Dokumente geändert.

Der [Messplan für Specs 1, 2 und 5](G01_SPEC01_02_05_MEASUREMENT_PLAN.md) konkretisiert deren 24 Erfolgskriterien und Voraussetzungen.

Der [Messplan für Specs 8, 14 und 15](G01_SPEC08_14_15_MEASUREMENT_PLAN.md) konkretisiert die Ziele für Video, Aufnahme und Livestream.

Der [Messplan für Specs 9, 22, 24 und 25](G01_SPEC09_22_24_25_MEASUREMENT_PLAN.md) konkretisiert die Ziele für Empfehlungen und intelligente Übergänge.

Der [Messplan für Specs 10, 11 und 12](G01_SPEC10_11_12_MEASUREMENT_PLAN.md) konkretisiert Mastering-, DVS- und Stem-Kriterien.

Der [Messplan für Specs 18 und 23](G01_SPEC18_23_MEASUREMENT_PLAN.md) konkretisiert Live-Remix- und Session-History-Kriterien.


## Spec 1: Auto-Updates

Kein Release-Prüf-/Installerpfad in den untersuchten Einstiegspunkten und Abhängigkeiten nachgewiesen. Quellen: [main.rs](../../src/main.rs), [Cargo.toml](../../Cargo.toml); vollständiger Solltext: [Spec](1-auto-updates/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST check for updates automatically on application startup after a 3-second delay | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST display update notifications with version number and release notes | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST allow users to download updates with real-time progress display | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST install downloaded updates when the application quits | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST allow users to manually trigger update checks by clicking the version display | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST show current version number in the UI at all times | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST support background downloads without blocking the UI | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST handle download failures with appropriate error messages | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST integrate with GitHub Releases for update hosting | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST verify update file integrity before installation | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST preserve user data during update installation | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST log all update-related events for debugging | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST support multiple update channels (stable, beta) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

**Vertiefung zu Spec 1:** Ein Entwurf von `deepseek-v4-flash-sovereign` ordnet die noch offenen Nachweise in drei ausführbare Gruppen ein. (1) FR-001, FR-002, FR-005, FR-006, FR-009, FR-013: automatischer Check nach drei Sekunden, manueller Check, Versions-/Release-Notes-Anzeige, GitHub-Releases-Abfrage und Auswahl von Stable/Beta; Netzwerkfehler, leere Releases und falsche Kanalzuordnung prüfen. (2) FR-003, FR-004, FR-007, FR-010, FR-011: Download im Hintergrund mit Fortschritt, Installation beim Beenden, Integritätsprüfung und Erhalt von Nutzerdaten; Unterbrechung, Speicherplatzmangel, beschädigte Datei und fehlende Schreibrechte prüfen. (3) FR-008, FR-012: verständliche Fehlermeldungen und Update-Ereignisprotokoll, einschließlich fehlgeschlagenem Download und Installation prüfen. Die Zuordnung und die Fehlerfälle wurden gegen die Originalspec abgeglichen; sie sind Testvorbereitung und kein Nachweis einer bereits vorhandenen Implementierung.

## Spec 2: Cloud-Sync

Lokale SQLite-Daten und History existieren; ein kontobezogener geräteübergreifender Sync ist damit nicht belegt. Quellen: [library_store.rs](../../src/library_store.rs), [library.rs](../../src/library.rs); vollständiger Solltext: [Spec](2-cloud-sync/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST sync track library metadata (title, artist, album, BPM, key, duration, file path) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST sync all user playlists and playlist contents | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST sync cue points for each track (hot cues with positions, colors, labels) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST sync beat grid analysis data for each track | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST sync all application settings and preferences | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST display real-time sync status (syncing, synced, error, offline) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST support manual sync trigger via UI button | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST support automatic sync on a configurable interval | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST allow selective sync by category (library, playlists, settings, cue points, beat grids) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST track last sync timestamp per device | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST identify each device uniquely for conflict resolution | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST implement last-write-wins conflict resolution strategy | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST work offline and queue changes for next sync | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST authenticate users before allowing sync operations | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST isolate data per user (no cross-user data access) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST handle sync errors gracefully with user-friendly messages | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST report item counts after successful sync (e.g., "Synced 1,234 tracks") | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

**Vertiefung zu Spec 2:** Ein Entwurf von `deepseek-v4-flash-sovereign` und der Abgleich mit der Originalspec ergeben vier Prüfgruppen. (1) FR-001, FR-002, FR-003, FR-004, FR-012: Bibliothek, Playlists, Cues und Beatgrids über zwei Geräte; unabhängige Änderungen und Last-write-wins sowie den spezifizierten Vorrang konkurrierender Löschungen getrennt prüfen. (2) FR-006, FR-007, FR-008, FR-013, FR-016, FR-017: sichtbarer Status, manueller und periodischer Sync, persistente Offline-Warteschlange, verständliche Fehler und korrekte Erfolgszahlen; Netzabbrüche, Wiederholungen und doppelte Zustellung prüfen. (3) FR-010, FR-011, FR-014, FR-015: Geräteidentität, letzter Sync, Authentifizierung und serverseitige Nutzerisolation vor Datenzugriff; unauthentifizierte und fremde Zugriffe müssen abgewiesen werden. (4) FR-005, FR-009: Einstellungen und selektive Kategorien; prüfen, dass nicht gewählte Kategorien unverändert bleiben und keine Zugangsdaten unbeabsichtigt synchronisiert werden. Die Details sind Testvorbereitung, kein Nachweis einer vorhandenen Cloud-Implementierung; die Konfliktsemantik und offene Fragen stehen in [D2](G01_DECISIONS_AND_MEASUREMENT.md).

## Spec 5: Streaming-Integration

Lokale Mediaeingabe und Audio-Decoder existieren; Provider-OAuth und lizenzierte Deckwiedergabe sind damit nicht belegt. Quellen: [media_input.rs](../../src/media_input.rs), [Cargo.toml](../../Cargo.toml); vollständiger Solltext: [Spec](5-streaming-integration/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST support OAuth 2.0 authentication for Spotify, SoundCloud, and TIDAL | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST securely store and encrypt authentication tokens | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST automatically refresh expired authentication tokens | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST allow users to disconnect from any service | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST provide unified search across all connected services | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST display search results with service source indicators | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST load streaming tracks directly to decks for playback | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST extract and display track metadata (BPM, key, duration, genre, artwork) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST browse user's personal playlists from each service | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST stream audio at highest quality available per service (Spotify 320kbps, TIDAL HiFi/Master, SoundCloud Go+) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST handle preview-only tracks (30-second Spotify previews) and indicate limitations | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST show connection status for each service (connected, disconnected, error) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST handle API rate limiting gracefully with user feedback | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST handle region-locked and unavailable tracks with clear messages | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST support searching with filters (genre, BPM range, key) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST cache frequently accessed data (playlists, metadata) for performance | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST support multiple services connected simultaneously | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

**Vertiefung zu Spec 5:** Entwurf von `deepseek-v4-flash-sovereign`, gegen die Spec abgeglichen: (1) FR-001, FR-002, FR-003, FR-004, FR-012, FR-017: Provider-Authentifizierung, sichere Tokenverwaltung, Refresh, Trennung und parallele Verbindungen; abgelaufene Tokens und falschen Verbindungsstatus prüfen. (2) FR-005, FR-006, FR-008, FR-015: dienstübergreifende Suche, Herkunft, Metadaten und Filter; fehlende Providerantworten und fehlende Metadaten prüfen. (3) FR-007, FR-010, FR-011, FR-014: Deckwiedergabe, Qualität, Preview-Beschränkung und regionale Verfügbarkeit; nicht verfügbare Tracks und Fehleranzeige prüfen. (4) FR-009, FR-013, FR-016: Providerplaylists, Rate-Limits und Cache; veraltete Daten und verständliche Rückmeldung prüfen. Rechte und tatsächlicher Zugriff sind vor realen Tests zu klären.

## Spec 8: Video Mixing

Audio-Decoder bzw. Audio-Crossfader sind kein Nachweis einer Video-Deck-/Compositing-Pipeline. Quellen: [audio_formats.rs](../../src/audio_formats.rs), [render_bus.rs](../../src/render_bus.rs); vollständiger Solltext: [Spec](8-video-mixing/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST support loading MP4, WebM, and MOV video files | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST provide two independent video decks (A and B) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST support playback controls per deck (play, pause, seek, skip ±5 seconds) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST display video thumbnails for loaded videos | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST show video metadata (resolution, FPS, duration, format) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST implement video crossfader for mixing between decks | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST display crossfader position as percentage (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST support per-deck effects: Brightness (0-200%), Contrast (0-200%), Saturation (0-200%), Hue Rotation (0-360°), Blur (0-10px), Sepia (0-100%), Grayscale (0-100%), Invert (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST support master effects applied to final output | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST implement chroma key (green screen) with color picker, tolerance (0-100%), and edge smoothing (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST provide pre-programmed transitions: Fade, Wipe, Slide, Zoom, Dissolve | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST support configurable transition duration (0.5s to 5s) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST support transition easing functions (linear, ease-in, ease-out, ease-in-out) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST implement video layouts: Single (crossfader-controlled), Split Screen (50/50), Picture-in-Picture, Side-by-Side | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST support aspect ratios: 16:9, 4:3, 1:1, Auto | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST render output at 1920×1080 resolution (Full HD) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST render at 30 FPS minimum for smooth playback | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-018 | System MUST extract and sync audio from video files with DJ audio engine | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-019 | System MUST support frame capture (screenshot current output) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-020 | System MUST provide real-time preview of final output | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

**Vertiefung zu Spec 8:** Entwurf von `deepseek-v4-flash-sovereign`, gegen die Spec abgeglichen: (1) FR-001, FR-002, FR-003, FR-004, FR-005: Dateiformate, zwei Decks, Wiedergabesteuerung, Thumbnail und Metadaten; beschädigte und nicht unterstützte Dateien prüfen. (2) FR-008, FR-009, FR-010: Deck-/Mastereffekte und Chroma Key; Parametergrenzen und falsch entfernte Farben prüfen. (3) FR-006, FR-007, FR-011, FR-012, FR-013, FR-014, FR-015: Crossfader, Übergänge und Layouts; Positions- und Bildfehler prüfen. (4) FR-016, FR-017, FR-018, FR-019, FR-020: Ausgabeauflösung, Framerate, Audio-Sync, Capture und Preview; Framedrops und Desynchronisation prüfen.

## Spec 9: AI Auto-Mix

BPM-/Key-Analyse und manueller Deck-Sync sind Grundlagen; Planung und automatische Übergänge sind dadurch nicht belegt. Quellen: [audio_analyzer.rs](../../src/audio_analyzer.rs), [sync_control.rs](../../src/sync_control.rs); vollständiger Solltext: [Spec](9-ai-automix/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST analyze tracks for BPM, key, energy level, and genre | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST calculate compatibility scores between track pairs (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST consider BPM compatibility (±6 BPM preferred, ±10 maximum) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST consider key compatibility using Camelot wheel rules | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST analyze energy levels (low/medium/high) from amplitude and frequency analysis | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST generate mix plans with track sequencing and transition points | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST support multiple energy flow modes: Build Up, Build Down, Peak & Valley, Steady | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST execute automated transitions with configurable lengths (15-60 seconds) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST support automatic BPM syncing during transitions | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST allow manual track selection override during auto-mix | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST provide detailed compatibility breakdowns (BPM %, Key %, Energy %, Genre %) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST allow mix plan preview and manual editing before execution | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST support genre-aware mixing (stay within genre or allow cross-genre) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST handle tracks with missing metadata gracefully | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST save and load mix plans for later execution | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST learn from user overrides to improve future suggestions | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST support manual transition triggering during auto-mix | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

**Vertiefung zu Spec 9:** Entwurf von `deepseek-v4-flash-sovereign`, gegen die Spec abgeglichen: (1) FR-001, FR-002, FR-003, FR-004, FR-005, FR-014: Analyse und Kompatibilität samt fehlenden Metadaten; unbestimmte BPM/Key und Scoregrenzen prüfen. (2) FR-006, FR-007, FR-011, FR-013, FR-015: Mixplanung, Energieverlauf, Scoreerklärung, Genre und Persistenz. (3) FR-008, FR-009, FR-010, FR-017: automatische Übergänge, BPM-Sync, Auswahl-Override und manueller Übergang; insbesondere sichere Übernahme während laufender Wiedergabe prüfen. (4) FR-012, FR-016: Vorschau/Editor und Lernen aus Overrides; leere oder geänderte Pläne prüfen.

## Spec 11: DVS

MC4000-Jog und Pitch-Bedienung existieren; Audio-Timecode-Eingang und -Decoder sind dadurch nicht belegt. Quellen: [mc4000.rs](../../src/mc4000.rs), [audio_devices.rs](../../src/audio_devices.rs); vollständiger Solltext: [Spec](11-dvs-control/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST decode timecode audio signals to extract position and velocity | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST support Serato CV02.5 timecode format | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST support Traktor MK2/MK3 timecode formats | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST support Rekordbox Control Vinyl timecode | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST track vinyl position with <5ms latency | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST detect and respond to scratches, backspins, and speed changes | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST implement automatic calibration on startup | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST allow manual calibration adjustments (gain, offset, phase) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST display signal quality meter (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST display tracking accuracy score | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST auto-recover from temporary signal loss (<500ms) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST support relative mode (scratch like vinyl) and absolute mode (scratch and seek) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST provide visual platter simulation with rotation and cue points | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST support CDJ mode (jog wheel emulation) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST track playback direction (forward/reverse/stopped) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST calculate and display vinyl RPM | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST support pitch range adjustment (±8%, ±16%, ±50%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

**Vertiefung zu Spec 11:** Entwurf von `deepseek-v4-flash-sovereign`, gegen die Spec abgeglichen: (1) FR-001, FR-002, FR-003, FR-004, FR-005, FR-015: Timecode-Dekodierung, vier genannte Formatanforderungen, Positionslatenz und Richtung; Signalausfall und ungültigen Timecode prüfen. (2) FR-007, FR-008, FR-009, FR-010, FR-011: automatische/manuelle Kalibrierung, Qualitäts-/Genauigkeitsanzeigen und Recovery. (3) FR-006, FR-012, FR-013, FR-014: Scratch, Relative-/Absolute-Modus, Platteranzeige und CDJ-Modus. (4) FR-016, FR-017: RPM und Pitchbereiche. Die rechtmäßige Formatunterstützung und reale Hardwareabnahme bleiben Voraussetzungen.

## Spec 12: Stem Control

Deck-Audiopfad und Offline-Analyse existieren; vier synchron getrennte Stems und Separation sind dadurch nicht belegt. Quellen: [deck_render.rs](../../src/deck_render.rs), [audio_analyzer.rs](../../src/audio_analyzer.rs); vollständiger Solltext: [Spec](12-stem-control/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST separate tracks into 4 stems: Vocals, Drums, Bass, Other (melodic instruments) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST provide progress indication during stem extraction (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST support stem extraction cancellation mid-process | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST cache separated stems for instant reloading | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST provide independent volume control per stem (0-100%) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST provide mute/solo buttons per stem | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST apply effects independently to each stem | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST support stem preset combinations (save/recall) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST display waveforms for each individual stem | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST support headphone cueing individual stems | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST maintain stem synchronization during playback | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST handle cache storage limits gracefully | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST support stem export to individual audio files | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST provide stem quality indicators | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST support batch stem extraction for multiple tracks | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST estimate separation time before starting | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST use AI/ML-based separation algorithms (Spleeter, Demucs, or similar) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

## Spec 14: Recording

Master-/Deck-Ausgabe existiert; ein Audio-Recording- und Encoderpfad ist dadurch nicht belegt. Quellen: [render_bus.rs](../../src/render_bus.rs), [Cargo.toml](../../Cargo.toml); vollständiger Solltext: [Spec](14-recording-system/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST record audio in WAV, MP3, and FLAC formats | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST provide quality settings: Low (128kbps), Medium (256kbps), High (320kbps), Lossless (WAV/FLAC) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST support recording from master output, Deck A, or Deck B | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST display real-time recording duration (hours:minutes:seconds) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST calculate and display estimated file size during recording | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST pause and resume recording without audio gaps | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST support auto-split recording at configurable intervals (15-120 minutes) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST save recordings with metadata: name, duration, file size, date, source | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST display list of all recordings with sortable columns | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST allow renaming recordings | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST allow deleting recordings with confirmation prompt | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST calculate and display total storage used by recordings | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST tag audio files with ID3 tags (for MP3) or metadata (for WAV/FLAC) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST prevent source changes during active recording | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST warn when disk space is low (< 500MB remaining) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST provide "Stop and Save" button to end recording | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST allow downloading recordings to local file system | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

## Spec 15: Live Streaming

Audioausgabe ist vorhanden; RTMP-/Video-Encoder, Chat und Plattform-Workflow sind dadurch nicht belegt. Quellen: [render_bus.rs](../../src/render_bus.rs), [Cargo.toml](../../Cargo.toml); vollständiger Solltext: [Spec](15-live-streaming/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST support Twitch, YouTube Live, Facebook Live, and Custom RTMP streaming | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST provide quality presets: Low (480p, 1.5Mbps), Medium (720p, 3Mbps), High (1080p, 6Mbps), Ultra (1080p60, 9Mbps) | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST accept stream keys from platform dashboards | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST accept custom RTMP server URLs for non-standard platforms | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST display real-time viewer count | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST display real-time connection stats: bitrate, FPS, frames dropped, connection quality | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST test connection before going live | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST integrate platform chat (Twitch, YouTube) with in-app chat panel | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST allow sending chat messages as broadcaster | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST support stream overlays: Now Playing, Logo, Social Media Links, Viewer Count, Chat Messages | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST allow positioning and resizing overlays | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST allow pausing and resuming stream | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST save stream sessions with metadata: platform, duration, peak viewers, average bitrate, dropped frames | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST provide stream session history with analytics | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST set stream title, description, category/game, and tags | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST warn when connection quality degrades | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST automatically attempt reconnection on temporary disconnects | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).

## Spec 18: Live Remix

Looping und Effekte sind vorhanden; frei arrangierbare Slices und Performance-Replay sind dadurch nicht belegt. Quellen: [looping.rs](../../src/looping.rs), [fx_chain.rs](../../src/fx_chain.rs); vollständiger Solltext: [Spec](18-live-remix-engine/spec.md).

| ID | Anforderung aus der Spec | G01-Befund |
|---|---|---|
| FR-001 | System MUST slice tracks into audio segments based on beat grid | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-002 | System MUST provide 4 slicing modes: Beats (4/8/16), Bars (4/8/16), Phrases (16/32), Custom | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-003 | System MUST allow selecting multiple slices simultaneously | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-004 | System MUST support drag-and-drop slice reordering | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-005 | System MUST play arrangements seamlessly without audio gaps | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-006 | System MUST maintain slice timing accuracy within ±5ms | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-007 | System MUST save slice arrangements as named patterns | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-008 | System MUST allow loading and applying saved patterns | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-009 | System MUST record live slice triggering with timestamps | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-010 | System MUST playback recorded performances exactly | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-011 | System MUST apply effects independently to each slice | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-012 | System MUST support slice effects: Reverb, Delay, Filter, Pitch Shift, Reverse | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-013 | System MUST clone, merge, and delete slices | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-014 | System MUST display waveform with slice boundaries | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-015 | System MUST save complete remix setups as presets | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-016 | System MUST export/import remix data as JSON | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |
| FR-017 | System MUST visualize playback position across slices | Offen: vollständiger Pfad und spezifikationsbezogener Nachweis fehlen. |

SC-001–008: offen; Originalgrenzwerte und benötigte Messungen im [Anforderungsregister](SPEC_REQUIREMENTS_MATRIX.md).
