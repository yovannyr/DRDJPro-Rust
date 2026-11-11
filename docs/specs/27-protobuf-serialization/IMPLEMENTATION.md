# Spec 27 — Umsetzung

## Schritt 1: generierte Nachrichten und Wire-Codec

`crates/drdj-proto/proto/drdj.proto` ist die Quelle der proto2-Nachrichten.
`prost-build` erzeugt Rust und Deskriptoren zur Build-Zeit; ein mitgeliefertes
`protoc` vermeidet eine zusätzliche Systeminstallation. `pbjson-build` erzeugt
Serde für sämtliche eigenen Nachrichten. Bytes verwenden Base64 im JSON,
64-Bit-Ganzzahlen Dezimalstrings und Zeitstempel UTC/RFC3339.
`build.rs` korrigiert einen redundanten `&FIELDS`-Bezug im generierten
pbjson-0.7-Diagnosecode, damit striktes Clippy ohne Lint-Unterdrückung läuft.

Der bisherige, ungenutzte handgeschriebene Prototyp wird ersetzt. Insbesondere
waren dessen auf drei Nachkommastellen gerundete BPM und Millisekundenpositionen
nicht verlustfrei. Dieser Schritt definiert einen neuen Wire-Vertrag `drdj.v1`:
`double`/`float` bewahren binäre Gleitkommabits, Gridpositionen Nanosekunden.
Alte Prototyp-Bytes und dessen Rust-Helfer-API sind **nicht kompatibel**.
Im Repository gibt es keine Aufrufer oder persistierten ProtoBuf-Daten dieses
Prototyps. Bestehende JSON-/SQLite-Daten der Anwendung werden nicht verändert.

`encode` und `decode` begrenzen binäre Nachrichten explizit auf 4 MiB; das ist
eine Anwendungsgrenze und kein angenommener Prost-Standard. `from_json`
begrenzt ebenfalls die Eingabe. JSON kann wegen Base64 größer als binäre Daten
werden; die JSON-Ausgabe ist nicht begrenzt. Die Wire-Grenze ist keine harte
Grenze für den Speicherbedarf dekodierter Objekte. Der Codec verarbeitet
einzelne Nachrichten, keine gerahmten Streams.

Typisiertes Prost-Decoding verwirft unbekannte Felder. Zum verlustfreien
binären Weiterreichen ist deshalb `ForwardMessage` zu verwenden. Der
unveränderliche Relay-Typ bewahrt auch unbekannte Felder in Unterobjekten.
JSON ist kein Archiv unbekannter ProtoBuf-Felder. Audio-Callbacks verwenden
diese allokierenden APIs nicht.

Tests prüfen alle sechs Nachrichtentypen, genaue Gleitkommabits einschließlich
NaN-Payloads im Binärformat, Base64/UTC-JSON, abgeschnittene und ungültige
Wire-Daten, Größenbegrenzung, additive Schema-Evolution, unbekannte Felder auf
zwei Ebenen und 1000 aufeinanderfolgende Rundläufe.

Verifikation Schritt 1 (Windows, 2026-11-11):

- `cargo test -p drdj-proto --locked`: 8 Tests bestanden.
- `cargo clippy -p drdj-proto --all-targets --locked -- -D warnings`: bestanden.
- `cargo fmt -p drdj-proto --check` und `git diff --check`: bestanden.
- `cargo check -p drdjpro-ui-spike --all-targets --locked`: bestanden.
- Unabhängiges Review: GPT-6.1 Sol, Denkaufwand Mittel; inklusive Nachreview
  der Generierungskorrektur, keine offenen blockierenden Befunde.

Dies ist kein vollständiger App-Testlauf und keine native Nutzungsabnahme.

## Noch offen

- Domain-Konvertierungen mit vollständiger Feldzuordnung.
- Semantische Pflichtfeld-/Wertevalidierung und Fehlerlogging. Der aktuelle
  Wire-Codec ist eine Infrastruktur-API; allein `required` im Schema ist bei
  Prost kein Nachweis vollständiger Pflichtfeldvalidierung.
- SyncRequest/Response, Versionsfehler und Pad-Nachrichten.
- Nachweis der Größen- und Laufzeitziele mit repräsentativen Daten.
- Abkündigungsregeln und explizite Anforderungen-/Testmatrix.

Die Spec ist damit noch nicht abgeschlossen. Visuelle und manuelle Tests
bleiben gemäß Nutzerwunsch einem gemeinsamen späteren Termin vorbehalten.
