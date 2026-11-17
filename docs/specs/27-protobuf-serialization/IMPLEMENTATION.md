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

## Schritt 2: vorhandene Domain und geprüfte Austauschverträge

`src/library/protobuf.rs` konvertiert den tatsächlich verwendeten nativen
`library::Track`, nicht einen erfundenen Ersatztyp. `NativeTrackMessage` bewahrt
ID, Name, Format, Verfügbarkeit, vollständige Metadaten und Analyse,
Option-Präsenz, das native Grid sowie alle acht festen Cue-/Loop-Slots.
Nanosekunden werden nicht auf Millisekunden gerundet. Native Pfade werden mit
expliziter OS-Kodierung gespeichert; auch nicht als Unicode darstellbare
Codeeinheiten bleiben erhalten. Import in ein anderes Pfadformat liefert einen
Fehler statt einer stillen Umwandlung. Der Suchindex wird aus den importierten
Werten neu aufgebaut; ein Snapshot löst keine Datei- oder Geräteprüfung aus.

`From<Track>` überträgt große Texte und Analysepuffer durch Besitzübergabe;
`From<&Track>` erstellt eine unabhängige Kopie. Unvertrauenswürdige Daten werden
mit `TryFrom<NativeTrackMessage>` validiert. `ValidatedTrackMessage` ermöglicht
anschließend das geforderte infallible `Into<Track>`, ohne Fehler zu verschlucken.

`src/protobuf_playlist.rs` konvertiert das bestehende öffentliche Playlist-Modell
einschließlich Entry-IDs, Reihenfolge, mehrfacher Track-Referenzen, Positionen,
Typ, Wiedergabestatus, Zeitstempeln und optionalen Beschreibungen/Covern.
Nicht darstellbare Zeitstempel oder unbekannte Enumwerte werden zurückgewiesen.
`SyncRequest`/`SyncResponse` transportieren sowohl diese Playlists als auch
native Track-Snapshots und kompakte Transport-Tracks. Bei einer inkompatiblen
Protokollversion gibt der Handler einen Versionsfehler und keine Updates zurück.
Das ist ein getesteter Austauschvertrag, kein neu bereitgestellter Cloud-Dienst.

An Anwendungsgrenzen werden `encode_checked`, `decode_checked`,
`to_json_checked` und `from_json_checked` verwendet. Sie prüfen Pflichtfelder
und Werte; native Importe verwenden dieselbe portable Wertevalidierung plus
die nativen Konstruktoren. Der Low-Level-Wire-Codec bleibt für bitgenaue
Transport-/Kompatibilitätstests verfügbar, auch für NaN-Payloads. Das ist keine
Freigabe nicht endlicher Werte für die Audio-/Library-Domain.

Die erste vollständige Decoder-Prüfung mit einem dynamischen Objektgraphen
benötigte 245,937 µs und verfehlte das Ziel. Deshalb prüft `presence.rs` die
Pflichtfeldpräsenz anhand einmalig vorbereiteter Deskriptoren mit Prost-Primitiven,
ohne einen zweiten Objektgraphen aufzubauen. Singuläre Unterobjektfragmente
werden zusammengeführt, wiederholte Objekte getrennt geprüft. Für zukünftige
gepackte, Group-, rekursive oder größere Schemas bleibt die sichere
Reflexionsprüfung als Rückfall erhalten. Prost dekodiert weiterhin die Werte.

Fehlerlogs enthalten Feldpfade; fehlerhafte JSON-Werte, Dateipfade, Hashwerte
und Binärpuffer werden nicht ins Log kopiert. `TrackMessage.analysis_hash`
ist auf Vertragsebene abgekündigt: Tag 9 bleibt lesbar und führt bei nicht leerem
Inhalt im geprüften Codec zu einer Laufzeitwarnung. Es werden keine
Compiler-/Clippy-Warnungen unterdrückt.

## Messungen und Abnahme

Windows, Intel Core i7-11850H, Release-Profil. Warme Messung über 31 Batches
zu je 100 Operationen nach 100 Aufwärmoperationen, inklusive Werte- und
Pflichtfeldprüfung. Die erste Deskriptorinitialisierung ist nicht enthalten.

| Operation | Median | p95 der Batchmittel | Ziel |
|---|---:|---:|---:|
| Track mit 500 Punkten kodieren | 8,998 µs | 9,868 µs | <50 µs |
| Track mit 500 Punkten dekodieren | 31,969 µs | 42,020 µs | <100 µs |
| 16 Pads kodieren | 0,510 µs | 0,548 µs | <10 µs |

Die reproduzierbaren Größen-Fixtures stehen in `tests/common/mod.rs` und
`tests/checked.rs`, ohne künstliche JSON-Einrückung:

| Nachricht | ProtoBuf | JSON | Reduktion |
|---|---:|---:|---:|
| Track, 500 Punkte | 7107 B | 26935 B | 73,61 % |
| BeatGrid, 500 Punkte | 6976 B | 26523 B | 73,70 % |
| Cue | 17 B | 68 B | 75,00 % |
| Loop | 13 B | 77 B | 83,12 % |
| Playlist | 51 B | 150 B | 66,00 % |
| SyncState | 18 B | 104 B | 82,69 % |

Das 40%-Ziel gilt hier für repräsentative Fixtures aller sechs Haupttypen,
nicht als mathematische Garantie für jeden möglichen Feldinhalt. Insbesondere
große reine Bytes-Payloads gegenüber Base64-JSON erreichen allein etwa 25 %.
Der 100-Track-Test verwendet Metadaten-Updates; er behauptet nicht, dass 100
vollständige 500-Punkt-Grids unter 200 KB passen.

Die vollständige Zuordnung der Anforderungen steht in [ACCEPTANCE.md](ACCEPTANCE.md).
Visuelle und manuelle Tests bleiben gemäß Nutzerwunsch für später offen;
kein nativer GUI-, Hör- oder Hardwaretest wird aus Unit-Tests abgeleitet.
