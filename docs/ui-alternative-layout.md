# UI Alternative Layout - Modern-Ansicht

## Ausgangsstand

- **Branch:** `Layout_Refinement2`
- **Repository:** C:\repo\DRDJPro-Rust (yovannyr/DRDJPro-Rust)
- **Layout-Paket:** C:\repo\drdjpro-layout
- **Datum:** 2026-11-10

## Ziel

Zusätzliche Modern-Performance-Ansicht als Ergänzung zur bestehenden Classic-GUI.
Beide Ansichten zur Laufzeit auswählbar, Classic bleibt Standard.

## Bestandsaufnahme

### Bestehende UI-Dateien

| Datei | Zeilen | Beschreibung |
|-------|--------|-------------|
| `ui/app.slint` | ~960 | AppWindow, Hauptansicht (Classic), ~90 Properties, ~40 Callbacks |
| `ui/theme.slint` | - | DjTheme, DeckSlotData, LoopRange, HotCueRow, etc. |
| `ui/controls.slint` | - | VirtualPlatter, ChannelMeter, HotCuePad, NudgeHold, etc. |
| `ui/waveform.slint` | - | WaveformStrip, DetailWaveform |
| `ui/deck.slint` | - | CompactDeck, DeckPlaceholder |
| `ui/library.slint` | - | CratePanel, TrackLibrary |
| `ui/editors.slint` | - | EqStrip, FxStrip |

### Bestehende Rust-Module (`src/`)

| Modul | Beschreibung |
|-------|-------------|
| `controller.rs` | 914 KB, Hauptcontroller, DualDeckEngine, AppWindow-Bindung |
| `playback.rs` | DeckId, DualDeckEngine, LoopEdge, MixerLevels |
| `deck_model.rs` | Deck-Modell |
| `deck_render.rs` | RenderControl für Waveforms |
| `waveform.rs` | DetailPeaks, Waveform-Rendering |
| `waveform_color.rs` | Farben |
| `waveform_overlay.rs` | Overlays |
| `mixer_channel.rs` | Mixer-Kanal |
| `mixer_dsp.rs` | DSP |
| `sync_control.rs` | Sync |
| `pitch_control.rs` | Pitch |
| `transport.rs` | Transport |
| `looping.rs` | Loops |
| `hot_cues.rs` | Hot Cues |
| `beat_grid_engine.rs` | Beat Grid |
| `beat_grid_ui.rs` | Beat Grid UI |
| `midi_controller.rs` | MIDI |
| `mc4000.rs` | MC4000 Hardware |

### AppWindow-Vertrag

**Properties (Auswahl):**
- `four-decks` (bool): 2 vs 4 Slots
- `deck-c-data`, `deck-d-data` (DeckSlotData): C/D Platzhalter
- `deck-c-waveform`, `deck-d-waveform` ([float]): C/D Waveforms
- `deck-c-colors`, `deck-d-colors` ([color]): C/D Farben
- `left-*`, `right-*`: Deck A/B Daten (title, bpm, pitch, etc.)
- `crossfader`, `channel-a`, `channel-b`: Mixer
- `eq-open`, `fx-open`, `sampler-open`, `monitor-open`, `audio-device-open`: Panels

**Callback-Hauptgruppen:**
- Platteneingabe: `virtual-platter-pointer`, `controller-jog`, `controller-seek-hold`
- Transport: `temporary-cue`, `stutter`, `controller-sync`
- Loops: `controller-four-bar-loop`, `loop-in/out/activate/reloop`
- Hot Cues: `controller-hot-cue`, `hold-hot-cue-left/right`
- Pitch: `controller-pitch-fader`, `pitch-down/up-left/right`
- Library: `load-library-left/right`, `select-library`, `library-search`, etc.
- MIDI/External: `controller-browse`, `external-drop-*`

### Modern-Package Struktur

| Paket-Datei | Beschreibung |
|-------------|-------------|
| `preview/ui/state.slint` | DeckAction, HoldAction, ControlKind, ModernDeckData |
| `preview/ui/modern-theme.slint` | ModernTheme Farben |
| `preview/ui/views/modern-view.slint` | ModernPair Hauptkomponente |
| `preview/ui/modern-deck.slint` | PerformanceDeck mit Jogwheel |
| `preview/ui/mixer.slint` | ChannelStrip mit EQ/Fader |
| `preview/ui/input.slint` | HoldButton, ValueRail, ModernKnob |
| `preview/ui/modern-waveform.slint` | PerformanceWaveform |
| `preview/ui/modern-2-decks-mockup.png` | Design-Mockup |
| `preview/ui/modern-4-slots-mockup.png` | 4-Slots Mockup |
| `docs/source-reference.png` | Visuelle Referenz |
| `docs/Data-contract.md` | Datenvertrag |
| `docs/Integration.md` | Integrationshinweise |
| `docs/Acceptance.md` | Abnahmekriterien |

## Implementierung und Daten-/Aktionsmapping

- **Ausgangscommit:** `c37dd1d` auf `Layout_Refinement2`. Der Checkout war bereits
  geändert; fremde bzw. vorhandene Änderungen wurden nicht zurückgesetzt.
- **Ein AppWindow und ein Controller:** `ui/app.slint` behält die Classic-Decks,
  Detailwellenformen und gemeinsamen Mixer-, Editor- und Library-Bereiche. Die
  Modern-Komposition ersetzt nur die Classic-Deck-/Detailwellenform-Fläche.
- **Ansichtsauswahl:** Die Combobox schaltet zwischen `Classic` und `Modern`;
  Classic bleibt Standard. 2 Decks/4 Slots sind davon unabhängig. Die Auswahl
  ist absichtlich nur zur Laufzeit aktiv, da keine vorhandene passende
  Layoutpräferenz-Konfiguration gefunden wurde.
- **Datenquelle:** A/B verwenden die bestehenden AppWindow-Werte. Die
  Modern-Komponenten erhalten Overview-/Detailpeaks, Farben, Playheads, BPM,
  Zeiten, Pitchgrenzen, Hot-Cue-Daten, Beat-/Warpmarker, Loop-Overlay,
  EQ-/Filterwerte, Fader, Meter und Cue-Monitorstatus. Artist und analysierter
  Key werden aus den Metadaten des geladenen Library-Tracks gesetzt; unbekannte
  Werte bleiben leer statt durch Beispieldaten ersetzt zu werden.
- **A/B-Aktionen:** Play/Pause, LOAD, gehaltenes CUE/Hot-Cue/Nudge, Sync,
  Keylock, Quantize, Loop-Activate/Exit/Half/Double, PFL, FX/Grid-Editor,
  Seek, Scratch/Jog und Detailzoom rufen die vorhandenen AppWindow-Callbacks
  auf. EQ, Filter, Channel- und Crossfaderwerte werden in die Einheiten der
  vorhandenen Regler zurücktransformiert und über die bisherigen
  `eq-changed`-/`mixer-changed`-Pfade an den Controller gegeben.
- **Externe Datei-Drops:** A/B-Hit-Testing und Weiterleitung an die bestehenden
  Drop-Callbacks funktionieren in beiden Ansichten; die Modern-Ansicht nimmt
  C/D weiterhin nicht als Audioziele an.
- **Library und Panels:** Es bleibt dieselbe `TrackLibrary`-/`CratePanel`-
  Instanz mit Suche, Sortierung, Auswahl, Crates und Laden. Modern kann die
  bestehende Library-Fläche ein-/ausblenden; die gemeinsamen EQ-, FX-,
  Sampler-, Monitor-, Audio- und Grid-Editoren bleiben verfügbar.
- **4 Slots:** C/D sind als nicht unterstützt/read-only gekennzeichnet; ihre
  Transport-/Mixer-/Cue-/Scratch-Eingaben sind deaktiviert. Die Adapter-
  Callback-Grenze nimmt Aktionen nur für Deckindizes 0 und 1 entgegen.
- **Umschalten:** Layout- und Slotwechsel beenden gehaltene Nudge-, Scratch-,
  Censor-, Such-, Loop-Jog-, temporäre Cue- und Hot-Cue-Gesten. Sie laden
  keinen Track neu und erzeugen keinen Audiostream. Transport-/Mixerzustand
  bleibt im gemeinsamen Controller.

| Zustand/Aktion | Classic | Modern | Prüfpfad |
|---|---|---|---|
| A/B Transport und Cue | bestehende Deck-Callbacks | dieselben Callbacks, gehaltene Cue-Semantik | Controller-Regression + vollständige Tests |
| Externe Datei-Drops | A/B-Hit-Testing und bestehende Drop-Callbacks | A/B-Zielbereiche der Modern-Decks | Classic-/Modern-Drop-Regression + vollständige Tests |
| Sync/Keylock/Quantize/Loops | bestehende Deck-/Loop-Callbacks | dieselben Callbackpfade | bestehende Controller-Tests |
| Pitch, EQ, Filter und Fader | bestehende Regler | normalisierte UI-Werte zurück in bestehende Skalen übersetzt | Workspace-Compile und Controller-Tests |
| Overview/Detail, Beat-/Warpmarker, Cues/Loops | bestehende Waveforms | dieselben Peaks, Marker und Quellzeitfenster | UI-Compile; manuelle Sichtprüfung offen |
| Suche, Crates, Trackauswahl und LOAD | bestehende Library | dieselbe Library-Instanz bleibt erreichbar | bestehende Library-Tests |
| C/D | deaktivierte Platzhalter | als unsupported/read-only angezeigt | Slint-Gating und Indexgrenze |
| Layoutwechsel | Classic-Standard | Modern zur Laufzeit | `modern_layout_actions_preserve_transport_and_cancel_held_nudge` |

## Bedienung und sicherer Rückweg

1. Start mit `cargo run --locked` (alternativ das vorhandene Startprofil).
2. In der oberen Leiste `Classic` oder `Modern` wählen. Die Umschaltung erhält
   den aktuellen A/B-Zustand; `Classic` ist der Standard beim Neustart.
3. `2 DECKS`/`4 SLOTS` steuert unabhängig davon die Slotdarstellung. C/D sind
   keine zusätzlichen Audiodecks.
4. Im Modern-Modus zeigt `SHOW LIBRARY`/`HIDE LIBRARY` dieselbe Produktions-
   Library. Die gemeinsamen Panelknöpfe öffnen die vorhandenen Editors.
5. Bei Darstellungs- oder Bedienproblemen im Selector zu `Classic` wechseln.
   Das beendet gehaltene Gesten; es stoppt oder lädt keine Decks neu.

## Prüfungen

| Befehl | Ergebnis |
|---|---|
| `cargo fmt --all -- --check` | Erfolgreich |
| `cargo check --workspace --all-targets --locked` | Erfolgreich |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Erfolgreich |
| `cargo test --workspace --locked` | Erfolgreich; 0 fehlgeschlagen, 6 ignoriert |
| `cargo build --release --locked` | Erfolgreich |

Kein Screenshot der laufenden App wurde erstellt. Paket-Mockups und
Referenzbilder bleiben Designvorlagen und sind keine Laufzeitscreenshots.

## Offene/native Abnahmen

- Native GUI-Abnahme und Classic-vs-Modern-Sichtvergleich
- Umschalten während realer Wiedergabe/Loops/Sync mit tatsächlichem Audiogerät
- MC4000/MIDI-Aktions-, Fokusverlust- und Controller-Hardwareprüfung
- FPS-/Eingabelatenz-Messung
- DPI-, Fenstergrößen- und kleine-Fenster-Prüfung

Headless-Tests und erfolgreicher Compile ersetzen diese Abnahmen nicht.
