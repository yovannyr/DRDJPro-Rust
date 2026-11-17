# Spec 27 — Anforderungsnachweis

Automatisierte Abnahme der Serialisierung und vorhandenen Domain-Konvertierungen.
Alle unten aufgeführten finalen Gates sind bestanden. Native visuelle/manuelle
Abnahme bleibt separat offen; die Größenqualifikation für SC-001 bleibt bestehen.

## Finale Gates — Windows, 2026-11-11

- `cargo test -p drdj-proto --locked`: 21 Tests bestanden.
- `cargo bench -p drdj-proto --bench codec --locked`: alle drei Laufzeitziele
  bestanden; finale Messwerte in [IMPLEMENTATION.md](IMPLEMENTATION.md).
- `cargo clippy -p drdj-proto --all-targets --locked -- -D warnings`: bestanden.
- `cargo clippy -p drdjpro-ui-spike --all-targets --locked --no-deps -- -D warnings`:
  bestanden; die neue Abhängigkeit wurde zusätzlich separat strikt geprüft.
- `cargo test -p drdjpro-ui-spike --lib --bins protobuf --locked --config
  'profile.test.package.drdjpro-ui-spike.opt-level=0'`: alle 7 nativen
  Track-/Playlist-Tests bestanden. Die lokale Testoption beschleunigt nur die
  Kompilierung dieser Funktionstests; App-/Cargo-Profile bleiben unverändert.
- `cargo fmt -p drdj-proto --check`, gezieltes `rustfmt --check` für die
  geänderten Root-Dateien und `git diff --check`: bestanden.
- Unabhängiges Review: separater GPT-6.1-Sol-Agent, Denkaufwand Mittel,
  einschließlich Nachreviews der Befunde und Pflichtfeldoptimierung;
  keine offenen blockierenden Codebefunde.

Das sind gezielte Spec-27-Tests und strikte Compile-/Clippy-Prüfungen aller
App-Targets, kein vollständiger erneuter Lauf sämtlicher App-Regressionstests.

| Anforderung | Implementierung / echter Test |
|---|---|
| FR-001 | `proto/drdj.proto`: Track, Playlist, BeatGrid, Loop, Cue, SyncState plus native Snapshots, Sync und Pads |
| FR-002 | `build.rs`: `prost-build`, mitgeliefertes protoc und Deskriptoren |
| FR-003 | `pbjson-build`: Serde für alle eigenen generierten Typen; Binary-/JSON-Rundläufe |
| FR-004 | Native `library::Track`, `BeatGrid`, Cue-/Loop-Slots und vorhandenes Playlist-Modell; `library::protobuf::tests`, `protobuf_playlist::tests` |
| FR-005 | `representative_six_message_types_reduce_payloads_by_forty_percent`, Track: 73,61 % |
| FR-006 | `native_track_roundtrip_preserves_all_fields_and_sparse_nanosecond_slots`, sämtliche Playlist-Arten, Wire-Float-Bits |
| FR-007 | Unveränderliche Tags, additive Entwicklung und Regeln unten; `additive_schema_evolution_preserves_older_fields` |
| FR-008 | `ForwardMessage`; unbekannte Felder auch innerhalb Grid und Sync-Updates werden binär erhalten |
| FR-009 | Option-Präsenz, wiederholte Daten und Sparse-Slots in Schema und Rundläufen |
| FR-010 | Bytes im Wire-Format; Base64 im JSON; auch native Pfad-Codeeinheiten |
| FR-011 | `From<Track>`/`From<&Track>`; validierte Hülle `Into<Track>`; unvalidierter Import ist absichtlich `TryFrom` |
| FR-012 | Rohe und geprüfte Binary-/JSON-Codecs; Rundlauf- und Fehlerprüfungen |
| FR-013 | `Contract`, schemaabhängige Pflichtfeldprüfung, negative native und verschachtelte Tests |
| FR-014 | `diagnostics_identify_fields_and_deprecation_without_echoing_private_values` |
| SC-001 | Alle sechs Haupttypen mit dokumentierten kompakten JSON-Fixtures: 66–83 %; kein Universalversprechen für jede mögliche Instanz |
| SC-002 | Release-Benchmark, validierter Track-Encode mit 500 Punkten: 8,998 µs Median |
| SC-003 | Release-Benchmark, vollständiger validierter Track-Decode: 31,969 µs Median |
| SC-004 | Native Feld-/Option-/Nanosekunden-/Slot-Rundläufe, Playlist-Felder und Float-Bits |
| SC-005 | Alte Deskriptorversion ohne Feld 12 und aktuelle generierte Version; unveränderte ältere Felder |
| SC-006 | Altes Schema akzeptiert neue Felder; Relay bewahrt sie; `key_change()` hat ohne neues Feld Wert null |
| SC-007 | 1000 Track- und 1000 Pad-Rundläufe |
| SC-008 | Generierung über prost-build; striktes Clippy für neue Crate und alle App-Targets ohne Warnungsunterdrückung |

## Ergänzende User-Story- und Grenzprüfungen

- US1: Track mit 500 Punkten, Größenvergleich und exakter Binary-/JSON-Rundlauf.
- US2: 100 Metadaten-Updates unter 200 KB, Revision 42 wiederhergestellt,
  Versionsfehler, native Tracks und bestehende Playlists durch denselben Vertrag,
  unbekannte Sync-/Track-Felder im Relay.
- US3: 16 Pads unter 1 KB, 1000 Zyklen; Cue-Position/Farbe und
  Loop-Position/Quantisierung bleiben erhalten; Pad-Encode-Laufzeit gemessen.
- US4: additive Schema-Evolution mit Deskriptoren; Standardwert für neues
  optionales Feld; Laufzeitwarnung für den abgekündigten Fingerprint.
- Exakt 4 MiB erlaubt, größere Nachrichten abgewiesen; malformed JSON,
  Varints, Längen, Group-Paare und zu tiefe Groups zurückgewiesen.
- Differentialprüfung der schnellen Pflichtfeldprüfung gegen Prost-Reflexion,
  einschließlich zweier jeweils unvollständiger singulärer Fragmente und zweier
  unvollständiger wiederholter Geschwister, die nicht verschmelzen dürfen.
- Native Importe weisen Duplikate, falsche Slots, Kodierungen, ungültige
  Metadaten, RGB-Farben und verlustbehaftete Zeitstempel ab.

## Schema-Regeln

`drdj.v1` ist der erste echte Wire-Vertrag; die alte handgeschriebene,
verlustbehaftete Nachbildung war kein kompatibles Releaseformat. Innerhalb
dieses Vertrages werden Tags und Wire-Typen nicht geändert oder neu vergeben.
Neue Felder werden optional oder wiederholt ergänzt. Entfernte Felder müssen
mit Nummer und Namen als `reserved` geschützt werden; es wurden hier keine
veröffentlichten Felder entfernt. Eine inkompatible Änderung benötigt eine
neue Vertragsversion und einen expliziten Versionsfehler statt stillen Datenverlusts.

`analysis_hash`, Tag 9, ist auf Vertragsebene abgekündigt, bleibt aber lesbar.
Die geprüften Codecs warnen ohne den Hashinhalt zu protokollieren. Diese
Transport-Abkündigung ist von einer Rust-API-Abkündigung zu unterscheiden.
Wire-Relay bewahrt unbekannte Felder; Konvertierung in ältere Domain-Typen
oder JSON ist ausdrücklich kein unbekannte-Felder-Archiv.

## Grenzen

Die erzeugten Rust-Typdefinitionen und ihre zugeordneten Serde-Implementierungen
wurden gemeinsam gezählt: maximal 315 Zeilen für `TrackMessage`; alle 20
generierten Typen einschließlich Enum bleiben unter 500 Zeilen. Gezählt wurden
die erzeugten Quelldateien, nicht die interne Rust-Makroexpansion.

Keine zusätzliche Cloud-Infrastruktur, gRPC-Anbindung, Audio-PCM-Übertragung
oder Migration bestehender SQLite-/JSON-Dateien. Keine OS-Treiberänderung.
Die 4-MiB-Grenze begrenzt Wire-Eingaben, nicht den gesamten Heapbedarf.
Die Release-Messung erfasst den warmen Workerpfad, keinen kalten Prozessstart.
Windows ist lokal geprüft; Linux/macOS benötigen ihre Plattform-CI bzw.
native Abnahme. Visuelle/manuelle Tests sind auf Nutzerwunsch vertagt.
