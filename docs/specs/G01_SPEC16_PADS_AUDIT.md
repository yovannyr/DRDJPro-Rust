# G01 – Einzelabgleich Spec 16: Performance Pads

Quellstand `df4966f`, statischer Abgleich am 08.11.2026. [Spec 16](16-performance-pads/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; nachfolgend wurden Dokumente geändert. Hardware- und Hörabnahme bleiben getrennt.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | Acht GUI-/Keyboard-Hot-Cues je Deck sind vorhanden; [pad_mode.rs](../../src/pad_mode.rs) routet vier physische MC4000-Pads pro Deck. Acht Pads mit dem vollständigen Modusumfang sind nicht belegt. |
| FR-002 | Teilweise | `Mode` in `pad_mode.rs` enthält `HotCue` und `Sampler`; Roll, Slicer, Beat Jump und Saved Loop sind nicht als Padmodi nachgewiesen. |
| FR-003 | Teilweise | Hot-Cue-Setzen und -Recall in [controller.rs](../../src/controller.rs); Positionsgrenzen und Quantisierungszustand beachten. |
| FR-004 | Teilweise | Recallpfad und synthetische Tests sind vorhanden; physische Triggerlatenz unter 10 ms ist nicht belegt. |
| FR-005 | Teilweise | Löschen und modifizierte Hardwarepads existieren; die spezifizierten Rechtsklick-/Long-Press-Bedienwege gesondert testen. |
| FR-006 | Offen | Acht spezifizierte Roll-Längen sind nicht als Roll-Padmodus nachgewiesen. |
| FR-007 | Offen | Klickfreier Rollmodus muss an Schleifenübergängen mit Signalen und Hörprobe geprüft werden. |
| FR-008 | Offen | Kontinuierliche Hintergrundposition und korrekter Release eines Rollmodus sind nicht nachgewiesen. |
| FR-009 | Offen | Acht gleich große Slicer-Teile pro Takt sind nicht als Padmodus nachgewiesen. |
| FR-010 | Offen | Instant-Slicer-Trigger und Timing sind nicht nachgewiesen. |
| FR-011 | Teilweise | MC4000-Sampler-Modus routet vier Slots; acht spezifizierte Pads und vollständige Sampleranforderungen siehe [Spec 19](G01_SPEC19_SAMPLER_AUDIT.md). |
| FR-012 | Teilweise | Navigation-/Beat-Jump-Bausteine existieren; die acht vorgeschriebenen Distanzen als Padmodus sind nicht vollständig nachgewiesen. |
| FR-013 | Offen | Syncgenauigkeit über Beat-Jump-Aktionen benötigt einen gezielten Timing-/Audio-Nachweis. |
| FR-014 | Teilweise | Gespeicherte Loops mit GUI-Bedienung existieren; Saved-Loop-Padmodus ist nicht nachgewiesen. |
| FR-015 | Teilweise | Cue-Farben und zustandsbezogenes MC4000-LED-Feedback sind implementiert; alle sechs Modi und tatsächliche Hardwarefarben sind nicht abgenommen. |
| FR-016 | Offen | Velocity-abhängige Auslösung kompatibler MIDI-Geräte ist nicht als allgemeiner Vertrag nachgewiesen. |
| FR-017 | Offen | Reaktionsverhalten bei hoher CPU-Last ist mit realem Audio und Controller zu messen. |

SC-001–008 bleiben offen: Trigger- und GUI-Latenz, klickfreie Rollübergänge, ±5-ms-Beat-Jump, Slicer-Quantisierung, Last und Moduswechsel brauchen gezielte Tests und reale Geräte-/Hörmessungen. Persistenz vorhandener Hot Cues allein belegt nicht die gesamte Spec.

Der [gemeinsame Messplan](G01_SPEC07_16_MEASUREMENT_PLAN.md) beschreibt alle acht Wellenform- und acht Padmessungen.
