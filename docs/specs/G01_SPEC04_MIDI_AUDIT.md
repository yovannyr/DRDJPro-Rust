# G01 – Einzelabgleich Spec 4: MIDI Controller

Quellstand `c83d173`, statischer Abgleich am 08.11.2026. [Spec 4](4-midi-controller/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; spätere Commits änderten Dokumente. Physische Controller- und Latenzabnahme bleibt separat offen.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | `present`, `unique_port`, `Input::connect` in [mc4000.rs](../../src/mc4000.rs) erkennen einen MC4000; alle MIDI-Geräte sind damit nicht abgedeckt. |
| FR-002 | Teilweise | [main.rs](../../src/main.rs) prüft periodisch MC4000-Verfügbarkeit und Wiederverbindung; echte Hotplug-Latenz und andere Geräte prüfen. |
| FR-003 | Teilweise | MC4000-Decoder behandelt Note On/Off und Control Change. `Command::PitchBend` wird hier durch Noten ausgelöst und belegt keine allgemeine Verarbeitung des MIDI-Pitch-Bend-Status `0xE0`. |
| FR-004 | Teilweise | `MidiInputConnection`, optionaler `MidiOutputConnection` und LED-Ausgabe sind für MC4000 vorhanden; bidirektionales generisches MIDI ist nicht vollständig nachgewiesen. |
| FR-005 | Offen | Presets DDJ-400, DDJ-SB3 und Party Mix sind im untersuchten Controllerpfad nicht nachgewiesen. |
| FR-006 | Teilweise | `unique_port` wählt anhand des MC4000-Namens; allgemeine Modellerkennung und automatische Zuordnung aller geforderten Presets fehlen als Nachweis. |
| FR-007 | Offen | Ein generischer Mapping-Fallback ist nicht nachgewiesen. |
| FR-008 | Offen | Interaktives MIDI-Learn mit Zuweisung ist nicht nachgewiesen. |
| FR-009 | Offen | Ohne nachgewiesenes Learn kein belegter Zehnsekunden-Timeout. |
| FR-010 | Offen | Editor für benutzerdefinierte Mappings ist nicht nachgewiesen. |
| FR-011 | Offen | Speicherung frei erstellter Controller-Mappings ist nicht nachgewiesen. |
| FR-012 | Offen | Beliebige Zuordnung jedes MIDI-Controls zu jeder DJ-Funktion ist nicht nachgewiesen. |
| FR-013 | Teilweise | Der feste MC4000-Pfad behandelt gehaltene und absolute/relative Eingaben; ein generisch konfigurierbarer Vertrag für alle vier Modi ist nicht belegt. |
| FR-014 | Offen | Konfigurierbare lineare, logarithmische und exponentielle Antwortkurven sind nicht als Mappingfunktion nachgewiesen. |
| FR-015 | Teilweise | Zustandsabhängiges LED-Feedback und `feedback_failure_does_not_acknowledge_frame_and_reconnect_replays_state` in `mc4000.rs`; tatsächliche Farben und Latenz physisch prüfen. |
| FR-016 | Offen | Allgemeine MIDI-Aktivitätsanzeige ist nicht als UI-Funktion nachgewiesen. |
| FR-017 | Offen | `unique_port` verwirft mehrere gleichnamige MC4000-Ports als mehrdeutig; gleichzeitiger Betrieb mehrerer beliebiger Geräte ist nicht nachgewiesen. |
| FR-018 | Offen | 1000 Nachrichten pro Sekunde je Gerät ohne Verlust/Lag benötigen einen reproduzierbaren Lasttest. |

SC-001–008 bleiben bis zu physischer Geräte-/Presetmatrix, Input- und LED-Latenz, Mehrgeräte- und Lastmessungen sowie Learn-Persistenz und Nutzertest offen. Das Laufzeitverhalten des vorhandenen MC4000 ist nicht allein aus Decodertests abgenommen.

Der [gemeinsame Messplan](G01_SPEC04_17_MEASUREMENT_PLAN.md) beschreibt die MIDI- und Sync-Messungen.
