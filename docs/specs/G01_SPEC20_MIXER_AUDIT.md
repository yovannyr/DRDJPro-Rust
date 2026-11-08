# G01 – Einzelabgleich Spec 20: Multi-Deck Control

Quellstand `01b3eb1`, statischer Abgleich am 08.11.2026. [Spec 20](20-multi-deck-control/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der letzte dokumentierte Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; spätere Commits änderten Dokumentation. Hardware, Hörergebnis und Laufzeitgrenzen sind nicht abgenommen.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | Vorhandenes Programm verwendet Deck A/B; Vierdeckmodus und Umschaltung sind nicht nachgewiesen. |
| FR-002 | Teilweise | Faderpfade für zwei Decks in [controller.rs](../../src/controller.rs) und [app.slint](../../ui/app.slint); vier unabhängige Kanäle fehlen als Nachweis. |
| FR-003 | Teilweise | `EqSettings`, DSP und `eq_frequency_response_trim_and_neutral_are_measured` in [mixer_dsp.rs](../../src/mixer_dsp.rs); GUI-Test `eq_gui_routes_both_decks_resets_and_preserves_transport_and_faders`. Vier Kanäle und vollständige Spec-Skalierung prüfen. |
| FR-004 | Teilweise | Gainsteuerung und [cue_bus.rs](../../src/cue_bus.rs) sind für bestehende Decks vorhanden; vier unabhängige Kanäle und geforderte Skalierung nicht nachgewiesen. |
| FR-005 | Teilweise | Crossfader für bestehende Decks vorhanden; frei zuweisbares Left/Center/Right-Routing pro Deck ist nicht nachgewiesen. |
| FR-006 | Offen | Separater Mute-Button je Kanal ist nicht als vollständige Funktion nachgewiesen. |
| FR-007 | Offen | Separater Solo-Button je Kanal ist nicht als vollständige Funktion nachgewiesen; Kopfhörer-Cue ist nicht automatisch Solo. |
| FR-008 | Teilweise | Masterpegelsteuerung ist im bestehenden Zweideckmixer vorhanden; exakte Skalierung und Persistenz gesondert prüfen. |
| FR-009 | Teilweise | [output_routing.rs](../../src/output_routing.rs) enthält Cue/Master-Mix und Test `headphone_gain_mix_ramp_is_stereo_coherent_and_leaves_master_untouched`; reale vierkanalige Kopfhörerabnahme offen. |
| FR-010 | Teilweise | Deck-Meter und MC4000-Feedback sind für bestehende Decks vorgesehen; Vierkanalabdeckung und Hardwareverhalten prüfen. |
| FR-011 | Teilweise | Master-Meterpfad existiert; Genauigkeit, Aktualisierungsrate und tatsächliche Hardwareanzeige gesondert messen. |
| FR-012 | Offen | Ein spezifizierter Master-Limiter mit Clippingverhinderung ist nicht nachgewiesen; Headroom und Meter sind kein Limiter. |
| FR-013 | Teilweise | Bestehende Decks speisen den Mixer; frei zuordenbares Vierdeck-Kanalrouting ist nicht belegt. |
| FR-014 | Teilweise | Der Test `eq_gui_routes_both_decks_resets_and_preserves_transport_and_faders` betrifft EQ-Reset; vollständiger Kanalreset aller Spec-Parameter ist nicht nachgewiesen. |
| FR-015 | Offen | Eine Solozustandsverwaltung samt Clear Solos ist nicht nachgewiesen. |
| FR-016 | Offen | Die spezifizierte kombinierte Anzeige für spielend, nicht stumm und hörbar ist nicht vollständig nachgewiesen. |
| FR-017 | Offen | Persistenz sämtlicher geforderter Mixerparameter über Neustart und Migration ist nicht vollständig nachgewiesen. |

SC-001–008 bleiben offen, bis Umschalt-/Reglerzeiten, Meterrate, Vierdeck-CPU, Limiter und Crossfader mit dem definierten Aufbau gemessen und subjektive Übergänge abgehört sind. Bestehende DSP- und GUI-Tests sind ein Teilnachweis, kein Vierdeck- oder End-to-End-Latenznachweis.

Der [Messplan](G01_SPEC19_20_21_MEASUREMENT_PLAN.md) konkretisiert die acht Mixer-Kriterien.
