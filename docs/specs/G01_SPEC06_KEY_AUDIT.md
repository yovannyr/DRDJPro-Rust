# G01 – Einzelabgleich Spec 6: Key Detection

Quellstand `7928de5`, statische Nachprüfung am 08.11.2026. [Spec 6](6-key-detection/spec.md) und das [Register](SPEC_REQUIREMENTS_MATRIX.md) bleiben für den vollständigen Wortlaut maßgeblich. Die Einstufungen belegen keine vollständige End-to-End- oder Nutzerabnahme. Der letzte Workspace-Test auf `b783a98` bestand mit 504 Tests und sechs ignorierten Tests; die nachfolgenden Commits änderten Dokumente, keinen Produktcode.

| ID | Status | Nachweis und verbleibende Arbeit |
|---|---|---|
| FR-001 | Teilweise | [audio_analyzer.rs](../../src/audio_analyzer.rs): Chroma-Aufbau und `estimate_key` im Analysepfad; Genauigkeit für Musik und Eingabegrenzen sind gesondert zu messen. |
| FR-002 | Teilweise | `camelot` in `audio_analyzer.rs`, Bibliothekszeile und Analysezusammenfassung in [controller.rs](../../src/controller.rs) zeigen Camelot-Schätzungen; vollständige Darstellung aller Fälle und Bedienwege prüfen. |
| FR-003 | Offen | Traditionelle Anzeige wie C-Dur nicht als einstellbarer Anzeigevertrag gefunden. |
| FR-004 | Offen | Alternative Open-Key-Anzeige nicht nachgewiesen. |
| FR-005 | Offen | Pearson-Korrelation und ein interner Mindestschwellwert existieren; `TrackAnalysisResult` enthält keinen ausgegebenen Confidence-Prozentwert. Kalibrierung und Anzeige fehlen als Nachweis. |
| FR-006 | Offen | Kein interaktives 24-Positionen-Camelot-Rad nachgewiesen. |
| FR-007 | Offen | Ohne nachgewiesenes Wheel keine belegte interaktive Hervorhebung kompatibler Tonarten. |
| FR-008 | Offen | Live-Kompatibilitätsanzeige zwischen Deck-Tonarten nicht nachgewiesen. |
| FR-009 | Offen | Die geforderten vier Kompatibilitätsstufen sind nicht als vollständige Regelmatrix nachgewiesen; Regelkonflikte in [D1](G01_DECISIONS_AND_MEASUREMENT.md) klären. |
| FR-010 | Offen | Harmonischer Assistent mit Bibliotheksvorschlägen nicht nachgewiesen. |
| FR-011 | Offen | Anzeigen musikalischer Beziehungen sind nicht als Bedienfunktion nachgewiesen. |
| FR-012 | Offen | Manuelle Key-Eingabe und dauerhafter Override gegenüber der Analyse sind nicht als kompletter Pfad nachgewiesen. |
| FR-013 | Teilweise | `ChromaBuilder` in `audio_analyzer.rs` sammelt ein zwölfteiliges Pitch-Class-Profil; `chroma_builder_finds_c_major_triad` prüft ein synthetisches Signal. Genreabdeckung und Visualisierung gesondert prüfen. |
| FR-014 | Teilweise | `estimate_key` vergleicht das Chroma per Pearson-Korrelation mit 24 rotierten Krumhansl-Kessler-Profilen. Die Gleichwertigkeit zum konkret geforderten Krumhansl-Schmuckler-Verfahren und dessen Genauigkeit sind gesondert zu belegen. |
| FR-015 | Teilweise | Im Analysepfad werden Dur- und Mollprofile verglichen; `detects_key_from_profile` testet synthetische Profile. Reale Dur-/Moll-Erkennungsraten sind nicht belegt. |
| FR-016 | Teilweise | `TrackAnalysisResult.musical_key` und der Analyse-Persistenzpfad in [library_store.rs](../../src/library_store.rs) speichern den Camelot-Wert; Format- und Migrationsfälle umfassend prüfen. |
| FR-017 | Offen | Ein intern berechnetes Chroma-Profil ist keine nachgewiesene Visualisierung für Nutzer. |

Alle acht Erfolgskriterien **SC-001–008 bleiben offen**: Corpus und Messung für Erkennungsquote/Laufzeit/Confidence, Theorie- und UI-Messung für Wheel/Kompatibilität, verfügbare passende Tracks für den Assistenten, Override ohne Neuanalyse sowie geeigneter Nutzertest. Synthetische Profiltests ersetzen diese Abnahme nicht.
