# G01 – Einzelabgleich Spec 19: Sampler

Quellstand `7928de5`, statischer Abgleich am 08.11.2026. [Spec 19](19-sampler-section/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der letzte dokumentierte Workspace-Test lief auf `b783a98` mit 504 bestandenen und sechs ignorierten Tests; spätere Commits änderten Dokumente. Das Ergebnis ist weder eine erneute Prüfung dieses HEAD noch eine Hardware-/Hörabnahme.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | [sampler.rs](../../src/sampler.rs) enthält vier Banken, [app.slint](../../ui/app.slint) beschreibt einen Vier-Slot-Sampler; acht Pads fehlen. |
| FR-002 | Teilweise | One-Shot und Retrigger sind in GUI/Engine vorhanden; ein Loopmodus pro Pad ist nicht nachgewiesen. |
| FR-003 | Offen | `set_gain` und GUI-Slider steuern den gemeinsamen Samplerpegel; Einzelpegel je Slot sind nicht nachgewiesen. |
| FR-004 | Offen | Die spezifizierte Trigger-Latenz unter 10 ms ist mit realer Eingabe, Audioausgang und dokumentiertem Buffer zu messen. |
| FR-005 | Teilweise | `sampler_four_voices_headroom_routes_retrigger_stop_eof_and_level` prüft vier Stimmen; acht gleichzeitige Stimmen sind nicht vorhanden. |
| FR-006 | Offen | Die Ladeaktion ist kein nachgewiesener Samplebibliotheks-Browser mit den Funktionen der Spec. |
| FR-007 | Teilweise | Slotdarstellung und Ladepfade sind vorhanden; die vollständige Anzeige von Sample-Namen und Dauer für jedes Pad ist nicht belegt. |
| FR-008 | Teilweise | `stop_all` und GUI-Button STOP ALL existieren; Verhalten und Verzögerung auf echter Ausgabe separat messen. |
| FR-009 | Offen | Loop All ist nicht als Funktion nachgewiesen. |
| FR-010 | Offen | Farbzuordnung nach Sampleeigenschaften ist nicht als vollständiger Padvertrag nachgewiesen. |
| FR-011 | Offen | Ein Trigger-Animations-/Glow-Vertrag und dessen Reaktionszeit sind nicht nachgewiesen. |
| FR-012 | Teilweise | `restore_sampler`, Speicherpfad und Test `sampler_gui_midi_recovery_persistence_missing_and_geometry` in [controller.rs](../../src/controller.rs) zeigen Persistenzpfade für die bestehende Viererbank. Fehlende Dateien und Neustartfälle gesondert abnehmen. |
| FR-013 | Offen | Kategorien für Samples sind nicht als Bibliotheksfunktion nachgewiesen. |
| FR-014 | Teilweise | `clear` und GUI CLEAR existieren für die bestehende Bank; alle acht geforderten Pads sind noch nicht vorhanden. |
| FR-015 | Teilweise | Fester Headroom je Stimme ist im bestehenden Bus vorgesehen; damit ist Clippingfreiheit des gesamten Masters unter beliebiger gleichzeitiger Deck-/Samplerlast nicht bewiesen. |
| FR-016 | Offen | Acht Sampler-Shortcuts 1–8 sind nicht als Funktion nachgewiesen; bestehende Hot-Cue-Shortcuts nicht damit gleichsetzen. |
| FR-017 | Teilweise | LOAD erlaubt lokale Dateien; Format-, Größen- und Sampleratenbeschränkungen sowie die Spec-Bedienung bleiben zu prüfen. |

Alle acht Erfolgskriterien **SC-001–008 bleiben offen**: echte Trigger-/Stop-Latenz, Lade-/Pegel- und GUI-Reaktionszeiten, acht gleichzeitige Samples und Last mit 500+ Bibliothekseinträgen sind nicht mit passenden Daten und Zielhardware belegt. Die bisherigen Tests ersetzen die Messungen nicht.

Der [Messplan](G01_SPEC19_20_21_MEASUREMENT_PLAN.md) konkretisiert die acht Sampler-Kriterien.
