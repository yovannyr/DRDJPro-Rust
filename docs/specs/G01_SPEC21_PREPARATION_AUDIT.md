# G01 – Einzelabgleich Spec 21: Track Preparation

Quellstand `c83d173`, statischer Abgleich am 08.11.2026. [Spec 21](21-track-preparation/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; danach wurden Dokumente geändert. Laufzeit-, Musik- und Hardwareakzeptanz sind nicht behauptet.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | [audio_analyzer.rs](../../src/audio_analyzer.rs) ermittelt BPM und Key; Energie- und Danceability-Werte sind im Ergebnisvertrag nicht nachgewiesen. |
| FR-002 | Offen | Manuelle Gridlinien sind keine erkannten Beatmarker mit Stärkewerten; siehe [BPM-Abgleich](G01_SPEC03_BPM_AUDIT.md). |
| FR-003 | Teilweise | `update_analysis_progress` in [controller.rs](../../src/controller.rs) zeigt verarbeitete Tracks und Fehler; kontinuierliche 0–100-%-Anzeige ist nicht nachgewiesen. |
| FR-004 | Teilweise | Grid-BPM-Texteditor und `set_bpm` in [beatgrid.rs](../../src/beatgrid.rs) existieren; die vollständige 0,01-BPM-Bedienung und Persistenz messen. |
| FR-005 | Teilweise | Grid-Nudge ist vorhanden; UI-Vertrag und 0,01-s-Schritte über den gesamten Bereich testen. |
| FR-006 | Teilweise | `anchor_here` erlaubt manuellen Grid-Anker; vollständigen Downbeat- und Neustartpfad prüfen. |
| FR-007 | Teilweise | `begin_cue_editor`, `save_cue_editor`, Hot-Cue-Aktionen und `cue_editor_cancel_conflict_source_lifetime_markers_and_restart` in `controller.rs`; alle Cue-Typen und Fälle nicht pauschal abgenommen. |
| FR-008 | Teilweise | Hot-Cue-Editor und Persistenz enthalten Name und Farbe; alle Spec-Bedienfälle und Farbeingaben prüfen. |
| FR-009 | Teilweise | Hot-Cue-Recall und Tests für beide Decks existieren; echte End-to-End-Latenz nicht gemessen. |
| FR-010 | Teilweise | Key-Schätzung vorhanden; ausgewiesener Confidence-Wert fehlt als Nachweis, siehe [Key-Abgleich](G01_SPEC06_KEY_AUDIT.md). |
| FR-011 | Offen | Manuelles Key-Override ist nicht als vollständiger persistenter Bedienpfad nachgewiesen. |
| FR-012 | Offen | Editor für alle aufgeführten Metadaten ist nicht nachgewiesen. |
| FR-013 | Teilweise | Cue- und Grid-Overlays in [app.slint](../../ui/app.slint); automatisch erkannte Einzelbeats und deren Stärke sind damit nicht belegt. |
| FR-014 | Teilweise | [library_store.rs](../../src/library_store.rs) persistiert Analyse, Grid und Hot Cues; alle geforderten Analysefelder existieren noch nicht. |
| FR-015 | Teilweise | Bibliotheksstatus zeigt analysiert; vollständiger Status einschließlich gesetzter Cues und Grid-Lock ist nicht nachgewiesen. |
| FR-016 | Teilweise | `whole_library_analysis_publishes_each_result_and_continues_after_failure` testet Batchpfad; 20+ Tracks ohne UI-Blockade separat messen. |
| FR-017 | Offen | Ein expliziter Re-Analyze-Button mit definiertem Umgang mit manuellen Overrides ist nicht nachgewiesen. |

SC-001–008 bleiben bis zu dokumentierten Messungen von Analysezeit/-genauigkeit, Grid-/Cue-Reaktionszeit, UI-Framerate, Batchverhalten und Persistenz offen. Das gemeinsame Zeitziel mit Spec 3 wurde nach Nutzerentscheidung auf innerhalb von 10 Sekunden festgelegt; siehe [Messkonzept](G01_DECISIONS_AND_MEASUREMENT.md).
