# G01 – Einzelabgleich Spec 17: Beat Grid & Sync

Quellstand `6b7d8b0`, statischer Abgleich am 08.11.2026. [Spec 17](17-beat-grid-sync/spec.md) und [Register](SPEC_REQUIREMENTS_MATRIX.md) enthalten den vollständigen Wortlaut. Der letzte dokumentierte Workspace-Test auf `b783a98` ergab 504 bestandene und sechs ignorierte Tests; die folgenden Commits änderten Dokumentation. Echte Gerätelatenz und Langzeitstabilität sind damit nicht abgenommen.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | MATCH, PHASE und kontinuierliches SYNC in [app.slint](../../ui/app.slint), Steuerung in [controller.rs](../../src/controller.rs); Funktionsumfang und Timing mit echten Decks prüfen. |
| FR-002 | Teilweise | Kontinuierlicher Sync folgt laut GUI-Vertrag dem jeweils anderen Deck; eine allgemeine explizite Master-Auswahl für mehrere Decks ist damit nicht nachgewiesen. |
| FR-003 | Teilweise | `toggle_sync`/`begin_sync_now` und `maintain_sync` in `controller.rs` steuern einen Follower. Grenzen und Verhalten bei Änderungen separat validieren. |
| FR-004 | Teilweise | `phase_sync_once`, `phase_sync` und [phase_sync.rs](../../src/phase_sync.rs) bieten Ausrichtung; variable Tempo-Grids werden in `phase_sync_rejects_variable_tempo_grids_until_local_tempo_matching_exists` explizit abgewiesen. |
| FR-005 | Teilweise | `nudge` im [beatgrid.rs](../../src/beatgrid.rs) und Grid-Editor existieren; Bediengrenzen und gesamte ±0,5-s-Strecke mit 0,01-s-Schritten nachweisen. |
| FR-006 | Teilweise | `anchor_here` und Test `downbeat_anchor_is_bar_one_with_numbered_bars_before_and_after_it` bilden einen manuellen Anker ab; GUI-/Persistenzpfad vollständig prüfen. |
| FR-007 | Offen | Ein persistenter Grid-Lock gegen unbeabsichtigte Änderungen ist nicht als vollständige Funktion nachgewiesen; Editierzustand ist nicht automatisch Grid-Lock. |
| FR-008 | Teilweise | Globale Quantisierung und Snap-/Trigger-Einstellungen sind in `app.slint` vorhanden; alle betroffenen Aktionen und Grenzfälle gesondert prüfen. |
| FR-009 | Offen | Spezifizierte laufende Anzeige des Beats 1–4 ist nicht vollständig nachgewiesen. |
| FR-010 | Offen | Ein durchgängiger Phasenindikator zwischen synchronisierten Decks ist nicht nachgewiesen. |
| FR-011 | Teilweise | BPM-Schätzung in [audio_analyzer.rs](../../src/audio_analyzer.rs) und Anzeige in `app.slint`; Nutzerübernahme, Grenzen und Musikgenauigkeit siehe [Spec-3-Abgleich](G01_SPEC03_BPM_AUDIT.md). |
| FR-012 | Teilweise | Text-Eingabe `grid-draft-bpm` und `set_bpm` existieren; explizite 0,01-BPM-Schritte und Anzeige-/Persistenzgenauigkeit testen. |
| FR-013 | Offen | Gefundener Tap steuert FX-Zeitbasis; Track-Tap für Grid-BPM nicht als Funktion nachgewiesen. |
| FR-014 | Teilweise | `track_grids` und Test `fractional_grid_and_nanosecond_anchor_survive_restart_and_explicit_clear` in [library_store.rs](../../src/library_store.rs); vollständige GUI-Roundtrips mit allen Gridvarianten prüfen. |
| FR-015 | Offen | Echtzeit-BPM-Differenz zwischen den Decks ist nicht als Spec-Anzeige nachgewiesen. |
| FR-016 | Offen | Expliziter Re-Analyze-Bedienvertrag für erneute BPM-Erkennung ist nicht nachgewiesen. |
| FR-017 | Offen | 30 Minuten reale Wiedergabe mit Driftmessung und Audiointerface erforderlich; synthetische mathematische Tests sind kein Ersatz. |

Alle SC-001–008 bleiben offen, bis die spezifizierten Zeit-, Genauigkeits-, Anzeige- und Tap-Ziele im jeweiligen Messaufbau belegt sind. Der [Messvertrag](G01_DECISIONS_AND_MEASUREMENT.md) trennt interne Timingwerte von tatsächlicher Audio-End-to-End-Latenz.
