# G01 – nachprüfbare Bestandsaufnahme

Arbeitsstand: 08.11.2026; untersuchter HEAD `b783a985bab38cb81c41b33f5d5226b9d5597647`. Quellen sind die aktuelle Rust-Implementierung, Tests im Repository und die zugehörigen Specs. Dies ist die erste begrenzte Teilprüfung, kein vollständiger Nachweis aller 618 Anforderungen. Ein vorhandener Test belegt zunächst nur, was sein Aufbau und Ergebnis tatsächlich abdecken; nicht ausgeführte Tests sind unten ausdrücklich als Quellbefund bezeichnet.

Prüflauf auf diesem Arbeitsstand: `cargo test --workspace --locked --quiet`: 504 Tests bestanden, sechs ignoriert. Die Factory-Validierung und neun Factory-Tests bestanden ebenfalls. Diese automatisierten Ergebnisse ersetzen keine Musik-Corpus-, Hardware-, Hör- oder Nutzerabnahme.

## Erste Befunde

| Spec / Anforderung | Beleg | Einordnung und verbleibende Prüfung |
|---|---|---|
| 3 / FR-001, FR-006 | [`audio_analyzer.rs`](../../src/audio_analyzer.rs): `MIN_BPM` 70, `MAX_BPM` 180 und `estimate_bpm_cancellable`; [`tempo.rs`](../../src/tempo.rs) enthält einen separaten Vorschlag für 40–240 BPM. | Zwei unterschiedliche Verfahren. Die in der Hauptanalyse vorgefundene Suche erfüllt nicht den gesamten geforderten Bereich 40 bis über 200 BPM. Prüfen, welches Ergebnis der Benutzer tatsächlich übernimmt und persistiert; keine Genauigkeitsquote behaupten. |
| 3 / FR-002, SC-004 | [`beatgrid.rs`](../../src/beatgrid.rs) beschreibt ein benutzerdefiniertes Grid ohne automatische Beat-/Tempoerkennung. | Berechnete Rasterlinien sind kein Nachweis automatisch detektierter einzelner Beats. Beatpositionen an annotierter Musik messen. |
| 3 / SC-001, SC-005 | `detects_120_bpm` und `detects_128_bpm` in [`audio_analyzer.rs`](../../src/audio_analyzer.rs). | Synthetische Tests sind als Quellbefund vorhanden; weder 95 % Genauigkeit an Musik noch 85 % Oktavkorrektur sind dadurch nachgewiesen. |
| 4 / FR-001–004, FR-015 | [`main.rs`](../../src/main.rs) enthält die MC4000-Erkennung und erneute Gerätesuche; [`mc4000.rs`](../../src/mc4000.rs) besitzt MIDI-Eingang, -Ausgang und LED-Ausgabe. | Ein konkreter Controllerpfad ist vorhanden. Damit sind generische Geräteerkennung und alle spezifizierten MIDI-Typen nicht pauschal nachgewiesen; physische Wiederverbindung und LEDs separat testen. |
| 6 / FR-001, FR-002, FR-013–016 | `estimate_key`, Camelot-Zuordnung und Tests `detects_key_from_profile`, `camelot_mapping_is_correct` in [`audio_analyzer.rs`](../../src/audio_analyzer.rs). | Relevante Analysebausteine und synthetische Tests vorhanden; vollständige UI-, Persistenz- und Algorithmusparität sowie SC-001 (85 % an Musik) bleiben gesondert zu prüfen. |
| 13 / FR-011 | `record_play` in [`library.rs`](../../src/library.rs); SQLite-Schema und `playback_history_delta_survives_restart` in [`library_store.rs`](../../src/library_store.rs). | Korrektur der früheren Bestandsaufnahme: `play_count` und `last_played_ms` werden geführt und persistiert. Ob alle in der Spec geforderten Bedien- und Anzeigefälle erfüllt sind, ist damit nicht vollständig belegt. |
| 17 / FR-004–006, FR-008 | `set_bpm`, `anchor_here`, `nudge` und Quantisierungsfunktionen in [`beatgrid.rs`](../../src/beatgrid.rs); Phase-Ausrichtung in [`phase_sync.rs`](../../src/phase_sync.rs). | Grid- und Ausrichtungsbausteine existieren; die Spec-Schrittweiten, Bedienwege und tatsächliche Syncgenauigkeit erfordern gesonderte Nachweise. Der Test `phase_sync_rejects_variable_tempo_grids_until_local_tempo_matching_exists` zeigt eine bestehende Einschränkung für variable Grids. |
| 19 / FR-001, FR-005, SC-005 | `banks: [Bank; 4]` und `sampler_four_voices_headroom_routes_retrigger_stop_eof_and_level` in [`sampler.rs`](../../src/sampler.rs). | Vier statt acht Stimmen; der Test beschreibt vier Stimmen, nicht die geforderten acht gleichzeitigen Samples. |
| 19 / FR-004, SC-001, SC-004 | [`sampler.rs`](../../src/sampler.rs), [`GUI_COMPONENTS.md`](../GUI_COMPONENTS.md). | Implementierung und synthetische Tests belegen keine gemessene End-to-End-Trigger-/Stop-Latenz unter 10 ms mit Hardware. |
| 20 / FR-003, FR-009 | [`mixer_dsp.rs`](../../src/mixer_dsp.rs) enthält EQ-Filter und den Test `eq_frequency_response_trim_and_neutral_are_measured`; [`cue_bus.rs`](../../src/cue_bus.rs) enthält Cue-Steuerung. | Vorhandene Zweideck-Bausteine sind kein Nachweis für alle Anforderungen des Vierdeckmixers oder die Gesamtfunktion jedes Kopfhörer-Routings. |
| 21 / FR-007–009, FR-014 | `track_hot_cues`, `track_grids` und Tests `hot_cues_restart_sparse_slots_clear_and_atomic_retry` und `fractional_grid_and_nanosecond_anchor_survive_restart_and_explicit_clear` in [`library_store.rs`](../../src/library_store.rs); Cue-Editor in [`controller.rs`](../../src/controller.rs). | Cue- und Grid-Persistenz sowie Editorpfade sind vorhanden; es ist getrennt zu prüfen, ob alle Spezifikationsfälle in der Oberfläche und mit realem Audio erfüllt sind. |

## Messprotokoll für die Fortsetzung

Für jede Messung festhalten: Datum, Commit und Arbeitsbaum, Betriebssystem, CPU, RAM, Audiointerface und Treiber, Samplerate/Puffergröße, Testdaten und Lizenz, Kommando bzw. Geräteaufbau, Wiederholungen, Rohwerte, Auswertung und ursprünglichen Grenzwert aus der Spec. Offline-Analysezeiten, Callback-Rechenzeit, Ein-/Ausgabe-End-to-End-Latenz und UI-Framerate getrennt ausweisen. Für Corpus-Genauigkeit sind unabhängige, versionierte Referenzannotationen und eine vorab festgelegte Auswertung notwendig. Fehlende Daten oder Hardware bleiben **offen**, nicht bestanden.

## Entscheidungen für die Fortsetzung

Der fortgeschriebene Arbeitsvertrag und die Messstrecken stehen in [G01_DECISIONS_AND_MEASUREMENT.md](G01_DECISIONS_AND_MEASUREMENT.md); die Einzelabgleiche für [Spec 3](G01_SPEC03_BPM_AUDIT.md), [Spec 4](G01_SPEC04_MIDI_AUDIT.md), [Spec 6](G01_SPEC06_KEY_AUDIT.md), [Spec 7](G01_SPEC07_WAVEFORM_AUDIT.md), [Spec 13](G01_SPEC13_LIBRARY_AUDIT.md), [Spec 16](G01_SPEC16_PADS_AUDIT.md), [Spec 17](G01_SPEC17_SYNC_AUDIT.md), [Spec 19](G01_SPEC19_SAMPLER_AUDIT.md), [Spec 20](G01_SPEC20_MIXER_AUDIT.md) und [Spec 21](G01_SPEC21_PREPARATION_AUDIT.md) enthalten alle jeweiligen FR und SC. Dort ausdrücklich als offen bezeichnete Akzeptanzfragen sind weiterhin nicht entschieden.

Für zehn weitere Bereiche dokumentiert [G01_EXTERNAL_FEATURES_AUDIT.md](G01_EXTERNAL_FEATURES_AUDIT.md) die einzelnen Anforderungen, für die bislang kein vollständiger End-to-End-Pfad belegt ist; mögliche bereits vorhandene Teilbausteine sind dort ausdrücklich benannt. Diese Ersteinstufung ersetzt keine spätere Detailprüfung jedes Teilpunkts.

Die verbleibenden fünf Bereiche Mastering, Neural Sync, Mix History, Smart Playlists und harmonisches Mixing sind in [G01_ADVANCED_FEATURES_AUDIT.md](G01_ADVANCED_FEATURES_AUDIT.md) einzeln anhand ihrer FR-IDs eingeordnet. Auch dort bleiben Unterpunkte und Messungen ohne vollständigen Nachweis offen.

Die maschinenlesbare Ersteinstufung jeder einzelnen FR und SC samt Verweis auf den zugehörigen Befund steht in [G01_AUDIT_STATUS.json](G01_AUDIT_STATUS.json). Sie ist eine Momentaufnahme, keine automatische Abschlussbestätigung; den Factory-Status G01 ändert sie nicht.

Für alle 25 Specs sind ergänzend einzelne Messstrecken für die jeweils acht Erfolgskriterien in den `G01_SPEC*_MEASUREMENT_PLAN.md`-Dateien beschrieben. Die Zuordnung wurde auf vollständige SC-IDs und gültige Links geprüft. Dies sind Messpläne, keine ausgeführten Messungen oder erfüllten Erfolgskriterien; Definitionen und konkrete Testdaten müssen teils noch festgelegt werden.

- Camelot: Wheel-Nachbarschaft und tatsächliche Halbtonintervalle getrennt modellieren. Der Nutzer hat die Korrektur der Beispiele sowie den Vorrang von Perfect vor Dramatic und die zusätzliche Anzeige des Halbtonintervalls bestätigt. Die Implementierung und vollständige Akzeptanzfälle bleiben offen.
- Cloud: Last-write-wins mit Änderungen an verschiedenen Feldern sowie konkurrierenden Änderungen am selben Feld getrennt testen. Ein garantierter Erhalt aller gleichzeitigen Änderungen folgt daraus nicht; das 95-%-Ziel benötigt ein vorher definiertes Szenario und bleibt offen.
- FPS: UI-Playhead, Video-Ausgabe, Effektlatenz und Stream-Overlay getrennt messen; unterschiedliche Zielwerte nicht gegeneinander verrechnen.
- Native Architektur: die vorhandenen Rust-/Slint-Grenzen sind Ausgangspunkt; Ersetzung einer konkreten Browsertechnologie erfordert pro Feature einen nachprüfbaren Entwurf. Diese Festlegung ist keine Freigabe neuer Abhängigkeiten und keine abgeschlossene globale Architekturabnahme.

## Noch offen

Alle 25 Specs besitzen eine erste FR-Einstufung; insbesondere bei den zusammengefassten externen und fortgeschrittenen Bereichen sind Unterpunkte und User Stories noch tiefer mit Quellcode, konkreten Tests und gegebenenfalls Laufzeit-/Hardwareevidenz abzugleichen. Corpus, Hardwarematrix, Kohortendesign und Ergebnisse der Entscheidungstore sind nicht vollständig. G01 bleibt daher **in Arbeit**; das Factory-Register darf abhängige Pakete nicht als abgeschlossen freigeben.
