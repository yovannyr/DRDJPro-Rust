# Funktionsstand und Lücken gegenüber dem Serato-Vergleich

Stand: 2026-11-17. Basis dieser Fortschreibung: PR-9-Merge `466ef36` und die
Korrekturen in [AI_REVIEW_FIXES.md](reviews/AI_REVIEW_FIXES.md).

Die frühere Fassung nannte 89–93 % Abdeckung, vollständig implementierte
Kernfunktionen und nicht existierende Tests. Diese Aussagen hatten keinen
vollständigen Anforderungsabgleich als Grundlage und werden zurückgenommen.
Aus vorhandenen Modulen oder verwendeten Bibliotheken folgt keine funktionale
Gleichwertigkeit mit Serato. Der frühere REA-Bericht auf dem nicht verfügbaren
D:-Laufwerk wurde hier nicht erneut geprüft.

## Bedeutung der Statuswerte

- **Real mit Test**: Der benannte, begrenzte Softwarevertrag ist implementiert
  und besitzt konkrete automatisierte Tests. Das bestätigt weder vollständige
  Feature-Parität noch native Geräte-, Hör-, Performance- oder Nutzungsabnahme.
- **Hülle/Stub**: Datenmodell, Schnittstelle oder Teilfunktion vorhanden;
  das angegebene Gesamtverhalten fehlt oder liefert einen expliziten Fehler.
- **Fehlt**: Für die benannte Fähigkeit existiert kein ausführender Backend-Pfad.

Die Tabellen bewerten nur die aufgeführten Verträge. Nicht untersuchte Funktionen
sind unqualifiziert und werden weder als vollständig noch als fehlend gezählt.
Es gibt keinen aus dieser Bestandsaufnahme ableitbaren Gesamtprozentsatz.

## Analyse, Vorbereitung und KI

| Fähigkeit / Vertrag | Status | Konkreter Nachweis und Grenze |
|---|---|---|
| Reale Datei-Analyse: Decode, BPM/Key-Schätzung, begrenzte Waveform, Abbruch | Real mit Test | [AudioAnalyzer](../src/audio_analyzer.rs): `existing_mp3_fixture_produces_valid_estimates_without_modifying_the_file`, `detects_120_bpm`, `detects_key_from_profile`, `cancelled_analysis_and_invalid_inputs_are_explicit_errors`. Reale Controller-Anbindung; synthetische/Fixture-Tests sind keine allgemeine musikalische Genauigkeitsmessung. |
| Unbekannte Werte bei Stille | Real mit Test | [AudioAnalyzer](../src/audio_analyzer.rs): `real_silence_decode_has_honest_unknown_estimates_and_bounded_waveform`, `silence_has_no_bpm`, `flat_chroma_has_no_key`. Keine garantierte Erkennung auf jedem Titel. |
| Legacy `TrackAnalysisManager::analyze` und automatische Batch-Analyse | Hülle/Stub | [track_analysis.rs](../src/track_analysis.rs): `track_analysis_manager_analyze` erwartet `NotImplemented`; `failed_analysis_preserves_existing_manual_data` schützt vorhandene Daten. Kein angenommenes 128-BPM-Grid. |
| Legacy Prep-Key-Erkennung, Reanalyse, Analyse-Dateiformate | Hülle/Stub | [track_prep_engine.rs](../src/track_prep_engine.rs): `prep_engine_key_detection`, `prep_engine_re_analyze`, `prep_engine_save_analysis` prüfen Fehler und fehlende Seiteneffekte. Kein Ersatz für den realen Analyzer-/SQLite-Pfad. |
| Manuelle Tonartkorrektur und gelieferte Beat-Overlays im Prep-Modell | Real mit Test | [track_prep_engine.rs](../src/track_prep_engine.rs): `prep_engine_key_override`, `prep_engine_generate_beat_overlays`. In-memory-Vertrag, keine automatische Key- oder Phrasenerkennung und kein Nachweis eines UI-Workflows. |
| Automatische Stem-Separation | Hülle/Stub | [stems.rs](../src/stems.rs): `unavailable_methods_never_return_mix_copies_or_claim_completion` und Cache-/Fehler-Regressionen. BandSplit/PhaseSplit liefern `NotImplemented`, MLBased ohne Backend `ModelNotLoaded`. Keine Mixkopien als Instrumente und keine erfundene Qualitätszahl. Manuell gelieferte Stems sind davon getrennt. |
| Regelbasierte Sync-Vorschläge auf gelieferten Annotationen | Real mit Test | [neural_sync_engine.rs](../src/neural_sync_engine.rs): `sync_engine_generate_suggestions`, `energy_suggestion_does_not_populate_phrase_quality`. Keine neuronale Inferenz; keine Harmonie-Vorschläge ohne Tonartdaten. Vorschläge zählen nicht als erkannte Phrasen. |
| Automatische neuronale Phrasen-/Downbeat-Erkennung und ML-Inferenz | Fehlt | Kein Modell-Backend in den genannten Analyse-/Sync-Modulen; keine Runtime, Modell-Registry oder qualifizierten Gewichte implementiert. Ein historischer Modulname ist kein KI-Nachweis. |
| Legacy Sync-Settings-Persistenz | Hülle/Stub | [neural_sync_engine.rs](../src/neural_sync_engine.rs): `sync_engine_save_load_settings` erwartet Fehler ohne Zustandsänderung. |

Der nie eingebundene `analysis_scheduler.rs` mit simuliertem Fortschritt wurde
entfernt. Der reale Import-Worker in [controller.rs](../src/controller.rs) und
`AudioAnalyzer::analyze_cancellable` bleiben davon getrennt. Ein neuer Scheduler
oder eine Inferenz-Runtime ist zukünftige Arbeit, keine bestehende Fähigkeit.

## Specs 26–31: implementierte Verträge und offene Grenzen

Die ursprüngliche Tabelle mit sechs pauschal fehlenden Features war veraltet.
Die folgenden Nachweise beschreiben den vorhandenen Stand dieses gestapelten
Branches; sie bedeuten nicht, dass dieser Gesamtstand bereits in `main` liegt.

| Fähigkeit / Vertrag | Status | Nachweis und offene Grenze |
|---|---|---|
| Spec 26: Crash-Erfassung, Report-Store, HTTPS/gzip-Queue, Retention, Symbol-Artefakte | Real mit Test | [Implementierung](specs/26-crash-handler/IMPLEMENTATION.md), [Transporttests](../src/crash_handler/transport/tests.rs), darunter `real_https_gzip_ack_sends_only_redacted_text_and_preserves_dump`, [Prozesstests](../tests/crash_process.rs). Native/manuelle Abnahme separat in [MANUAL_ACCEPTANCE.md](specs/26-crash-handler/MANUAL_ACCEPTANCE.md); kein behaupteter Crashpad-Austausch durch bloße Bibliothekswahl. |
| Spec 27: versionierte ProtoBuf-Verträge, Grenzen, Migration/Roundtrip | Real mit Test | [Implementierung](specs/27-protobuf-serialization/IMPLEMENTATION.md), [wire.rs](../crates/drdj-proto/tests/wire.rs): `generated_messages_have_binary_and_json_roundtrips`, `additive_schema_evolution_preserves_older_fields`. Kein Nachweis eines Cloud-Sync-Dienstes. |
| Spec 28: libusb-Discovery, begrenztes Worker-I/O, HID-Verträge | Real mit Test | [Implementierung](specs/28-direct-libusb/IMPLEMENTATION.md), [io_worker.rs](../crates/drdj-usb/tests/io_worker.rs): `accepted_commands_run_in_order_off_caller_thread_and_tickets_are_one_shot`; weitere HID-Tests im selben Testverzeichnis. Native Profile/Controller-Qualifikation siehe [Abnahme](specs/28-direct-libusb/ACCEPTANCE.md). |
| Spec 28: optionaler xwax-DVS-Pfad | Real mit Test | Umfang und Nachweise in der [Implementierung](specs/28-direct-libusb/IMPLEMENTATION.md); [Lizenzentscheidung](specs/28-direct-libusb/DVS_DECISION.md). GPLv3-Build und reale Vinyl-/Audiointerface-Abnahme getrennt behandeln; kein pauschaler DVS-Hardware-Nachweis. |
| Spec 29: generisches 128×64-/128×32-Display, separates Fenster, Standard-MIDI-Pads | Real mit Test | [Implementierung](specs/29-akai-remote-screen/IMPLEMENTATION.md), Tests in [drdj-remote/src](../crates/drdj-remote/src). Nutzerziel ist ein generisches Monitorfenster; [native Abnahme](specs/29-akai-remote-screen/ACCEPTANCE.md) bleibt separat. |
| Akai MPC Touch / DisplayLink-Geräteprotokoll | Fehlt | Nicht das freigegebene generische Ziel von Spec 29; keine Akai-Geräteunterstützung aus Fenster- oder MIDI-Code ableiten. |
| Spec 30: Windows-Browser-/Offline-Help-Shell mit URL-Policy | Real mit Test | [Implementierung](specs/30-web-engine/IMPLEMENTATION.md), [web_policy.rs](../src/web_policy.rs): `only_https_and_our_help_origin_are_navigable`, [web_browser.rs](../src/web_browser.rs): headless Lifecycle-/Dispatchtests. WebView2-Darstellung, Sites und andere OS-Backends sind damit nicht abgenommen. |
| Beatport-Katalog-/Audio-Integration bis ins Deck | Fehlt | Kein Partner-/SDK-Zugang vorhanden. Website-Anmeldung und ein HTTP-Client ersetzen keine autorisierte Streaming-Integration; Browser-Audio wird nicht ins Deck geleitet. |
| Spec 31: lokaler iTunes/Music-XML-Import, Source-Katalog, inkrementelle Übernahme | Real mit Test | [Implementierung](specs/31-itunes-library/IMPLEMENTATION.md), [import.rs](../crates/drdj-itunes/tests/import.rs): `all_metadata_entities_empty_values_and_order_are_preserved`, `malformed_or_hostile_xml_aborts_and_cancellation_is_observed`; Store-/UI-Vertragstests ergänzen den Parser. Keine binären ITL-/Music-Datenbanken und kein DRM-Audio. |
| Apple-Music-Metadatenadapter | Real mit Test | Begrenzter Adapter und simulierte HTTP-/Store-Nachweise laut [Implementierung](specs/31-itunes-library/IMPLEMENTATION.md). Kein MusicKit-Token vorhanden; produktive API-/Netzwerkabnahme offen. Dies ist keine Apple-Music-Audiowiedergabe. |

## Prüfstand und verbleibende Arbeit

Die aktuellen Korrekturen und tatsächlich ausgeführten Befehle stehen in
[AI_REVIEW_FIXES.md](reviews/AI_REVIEW_FIXES.md). Verlinkte historische Nachweise
sind keine Aussage, dass jede dort genannte Integration in diesem Durchlauf
nochmals ausgeführt wurde. Visuelle und manuelle Tests führt der Nutzer selbst
durch; Hardware-, Audio- und Partnerdienst-Abnahmen bleiben eigenständige Nachweise.

Vor einer belastbaren Paritätsaussage sind ein versionsgebundener Funktionskatalog,
einzelne Anforderungstests und qualifizierte Ergebnisse für reale Musik, Geräte und
Betriebssysteme nötig. Laufzeit- und Gewichts-Lizenzen, Modellgröße, RAM-Budget,
Abbruch und gemessene Inferenzqualität sind vor einer zukünftigen ML-Integration
separat zu entscheiden. Der vorliegende Kritik-Fix beginnt diese Entwicklung nicht.
