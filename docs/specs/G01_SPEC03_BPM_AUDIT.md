# G01 – Einzelabgleich Spec 3: BPM Detection

Quellstand `9bd35f45452934297194f5bd00c77f4c9937bb9e`, statische Nachprüfung am 08.11.2026. Referenz: [Spec 3](3-bpm-detection/spec.md), vollständiger Wortlaut und Slice-Zuordnung im [Register](SPEC_REQUIREMENTS_MATRIX.md). `Teilweise` bezeichnet einen nachweisbaren Baustein ohne vollständigen End-to-End-Nachweis; `offen` umfasst Fehlendes oder nicht Belegtes. Bestehende Tests im Quellcode wurden im letzten Workspace-Lauf auf `b783a98` ausgeführt; seitdem wurden laut Git nur Dokumente geändert. Das ist kein neuer Lauf auf diesem HEAD und keine Musik- oder Hardwareabnahme.

## Funktionale Anforderungen

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | `AudioAnalyzer::analyze_file` und `estimate_bpm_cancellable` in [audio_analyzer.rs](../../src/audio_analyzer.rs); Realmusik-Qualität und alle Eingangsfälle offen. |
| FR-002 | Offen | Das Ergebnis `TrackAnalysisResult` enthält keine Liste detektierter Beats; [beatgrid.rs](../../src/beatgrid.rs) erzeugt ein benutzerdefiniertes Raster. Einzelbeat-Erkennung und Musiktest fehlen als Nachweis. |
| FR-003 | Offen | `TrackAnalysisResult` enthält kein BPM-Confidence-Feld; Bewertung und Kalibrierung fehlen als nachgewiesener Benutzervertrag. |
| FR-004 | Offen | Keine Tempoklassifikation im Ergebnisvertrag nachgewiesen; Grenzen und Darstellung umsetzen und testen. |
| FR-005 | Offen | Die manuell konfigurierbare Taktart im Grid ist keine automatische Taktarterkennung. Testcorpus für 4/4, 3/4 und 6/8 erforderlich. |
| FR-006 | Teilweise | Hauptanalyse `MIN_BPM=70`, `MAX_BPM=180`; separater Schätzer in [tempo.rs](../../src/tempo.rs) deckt rechnerisch 40–240 ab. Einheitlichen Analyse-/UI-Vertrag und Ausgabe über 200 BPM nachweisen. |
| FR-007 | Teilweise | Die Hauptanalyse bewertet Autokorrelation und Harmonische mit Tempoprior; das garantiert keine korrekte Oktavwahl. Spezifizierte Korrektur an annotierten Half-/Double-Time-Fällen messen. |
| FR-008 | Teilweise | `set_bpm` im [Grid](../../src/beatgrid.rs) erlaubt manuelle BPM; prüfen, ob ein Track-Metadaten-Override unabhängig vom Grid gefordert und verfügbar ist. |
| FR-009 | Offen | Der gefundene Tap in [controller.rs](../../src/controller.rs) steuert FX-Timing; ein Track-Tap mit mindestens vier Taps ist nicht belegt. |
| FR-010 | Teilweise | `start_analysis`/`start_next_analysis` in [controller.rs](../../src/controller.rs) analysieren Bibliotheksziele; Test `whole_library_analysis_publishes_each_result_and_continues_after_failure`. Auswahlumfang und Bedienvertrag prüfen. |
| FR-011 | Teilweise | `update_analysis_progress` zeigt verarbeitete Tracks, Gesamtzahl und Fehler; der genannte Batch-Test prüft u. a. `1/3 processed`. Prozentanzeige oder Fortschritt innerhalb eines Tracks ist hier nicht belegt. |
| FR-012 | Teilweise | Ergebnis enthält BPM; `write_analysis` und `track_analysis_upgrade_delta_restart_clear_and_rollback` in [library_store.rs](../../src/library_store.rs) belegen den vorgesehenen Persistenzpfad als Quell- und Testbefund. Alle Dateiformate und Migrationsfälle nicht pauschal abgenommen. |
| FR-013 | Teilweise | Manuelles Grid-BPM vorhanden; `estimated_bpm_is_a_track_suggestion_and_never_automatically_applies_a_grid` in [controller.rs](../../src/controller.rs) testet bewusste Übernahme. Dauerhafter Override des gespeicherten Analysewerts ist gesondert nachzuweisen. |
| FR-014 | Offen | Keine Beatintervall-Varianz im Analyseergebnis nachgewiesen; Berechnung und Behandlung unregelmäßiger Tracks erforderlich. |
| FR-015 | Teilweise | `estimate_bpm_cancellable` bildet positive Differenzen der komprimierten Energiehüllkurve als Onset-Stärke; ein daraus abgeleiteter einzelner Beatfinder gemäß FR-002 ist damit nicht nachgewiesen. |
| FR-016 | Offen | Die geforderte 0–200-Hz-Tiefpassfilterung ist im untersuchten BPM-Pfad nicht nachgewiesen; ein 200-Hz-Abtastraster der Hüllkurve ist kein 200-Hz-Audiofilter. |
| FR-017 | Offen | Die Hauptanalyse verwendet normalisierte Autokorrelation und Tempoprior; das spezifizierte Histogramm-Clustering ist im untersuchten Pfad nicht nachgewiesen. |

## Erfolgskriterien

Alle acht SC sind **offen**. SC-001/003–005 benötigen annotiertes Musik-Corpus und definierte Auswertung; SC-002/006 reproduzierbare Zeitmessung auf dokumentierter Hardware; SC-007 einen getesteten Track-Tap; SC-008 ein breites Genre-/Fehlercorpus und Hängeerkennung. Synthetische `detects_120_bpm`/`detects_128_bpm` und ein bestandener Workspace-Test belegen keine der geforderten Prozentquoten oder Zeitgrenzen. Das [Messkonzept](G01_DECISIONS_AND_MEASUREMENT.md) hält Messstrecken und noch fehlende Voraussetzungen fest.

Der [BPM-Messplan](G01_SPEC03_MEASUREMENT_PLAN.md) konkretisiert alle acht Messungen und ihre Voraussetzungen.
