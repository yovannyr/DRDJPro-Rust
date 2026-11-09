# Track-Ladezeit – Messung und Korrektur (2026-11-09)

## Ursache und Änderung

Der tatsächliche Worker lädt vollständiges PCM, berechnet Overview, Detail-Peaks
und Frequenzfarben, schätzt optional das Tempo und liest Metadaten. Erst danach
übernimmt der Controller das Ergebnis ins Deck. Die Metadaten lesen Header/Tags;
sie führen keine zweite vollständige PCM-Dekodierung aus.

Die Hauptkosten entstehen in den unoptimierten Debug-Schleifen von Decoder und
Farbaufbereitung. Derselbe Code benötigt bei einer vierminütigen Stereo-MP3
17,72 s im bisherigen Debug-Profil, aber 0,44 s im Release-Testprofil. Der BPM-Hint
spart nur etwa 0,16 s und beseitigt diesen Engpass nicht. Ein neuer algorithmischer
Engpass durch die Layout-Migration ist anhand der Quellvergleiche nicht belegt;
welches Binary früher verwendet wurde, ist nicht nachgewiesen.

`Cargo.toml` optimiert jetzt gezielt das App-Paket mit Level 1 und Rodio,
Symphonia-Codecs/-Container sowie Opus mit Level 2 im Dev-Profil. Debugsymbole,
Assertions und Overflow-Prüfungen bleiben erhalten; Release-Konfiguration,
Dependency-Versionen, Features und PCM-Grenzen bleiben unverändert. Die normale
Cargo-Zeile kann wegen des übergeordneten Profils weiterhin „unoptimized“ anzeigen;
die tatsächlich ausgeführten rustc-Aufrufe enthalten die Package-Overrides.

Keine geänderten Audioalgorithmen, zusätzlichen Decoder, Locks, Allokationen
oder Analysearbeit im Audiocallback. Der bisherige Worker und dessen Abbruch-/
Generationsregeln bleiben maßgeblich. Der Produktionstimer bleibt unverändert.

Ein Test-Build mit App-Level 2 stürzte im Compiler mit STATUS_ACCESS_VIOLATION ab.
Deshalb wurde für das App-Paket Level 1 gewählt und erfolgreich gebaut/getestet;
die kleinen Codec-Pakete bleiben auf Level 2. Dieser Versuch wird nicht als
erfolgreicher Check gewertet.

## Gemessene Nutzerdateien

Dateien wurden nur gelesen; kein Track wurde verändert oder abgespielt.
Der Vorherwert umfasst Decode, beide Waveform-Aufbereitungen, Tempo und Metadaten.
Die Nachherwerte messen den vollständigen Headless-Controllerpfad einschließlich
Worker, Tick, Library-Aktualisierung, Installation und veröffentlichtem Loaded-Zustand.

| Datei unter `E:/DJ_Music/08_DJ_Remixes/Deto` | Länge | Debug vorher | Deck A bereit | Deck B bereit |
| --- | ---: | ---: | ---: | ---: |
| `01 ritmo latino- Unbekannter Titel.mp3` | 246,82 s | 19,99 s | 3,43 s | 3,63 s |
| `02 -variado desconocido.mp3` | 193,97 s | 16,24 s | 2,95 s | 2,78 s |
| `bachata especial.mp3` | 233,95 s | 20,57 s | 3,35 s | 3,30 s |

Zusätzlich: generierte vierminütige Stereo-MP3 A/B 3,26/3,30 s und Stereo-WAV
3,24/3,25 s. Alle zehn Deck-Ladevorgänge bestanden die feste Fünf-Sekunden-Assertion.
Maximalwert dieses Laufs: 3,64 s. Die Messung verwendet BPM-Hint 0, also den
vollständigen Fallback inklusive Tempoanalyse. Decks bleiben pausiert; beim Laden
von B wird ausdrücklich geprüft, dass A geladen bleibt.

Das ist ein Nachweis für diese Dateien und diesen Rechner, keine universelle
Obergrenze für beliebig große Dateien oder extern blockierte Datenträger.
Headless-Polling verwendet 2 ms statt des echten 33-ms-Timers. Die erste native
Bildschirmdarstellung, Hör-/Hardwareabnahme und manuelle Bedienung werden auf
Nutzerwunsch später gemeinsam geprüft. Es wurde dafür keine GUI geöffnet.

## Reproduktion

`playback::tests::selected_file_loading_profile` gibt getrennte Phasenzeiten aus.
`controller::tests::selected_files_load_into_decks_within_five_seconds` prüft
Deck A/B über den echten asynchronen Controllerpfad. Ohne Variable verwenden
beide die vorhandene selbständige Test-Fixture.

```powershell
$env:DRDJ_LOAD_BENCH_FILES = 'E:\DJ_Music\08_DJ_Remixes\Deto\01 ritmo latino- Unbekannter Titel.mp3;E:\DJ_Music\08_DJ_Remixes\Deto\02 -variado desconocido.mp3;E:\DJ_Music\08_DJ_Remixes\Deto\bachata especial.mp3'
cargo test --bin drdjpro-ui-spike --locked selected_file -- --nocapture --test-threads=1
```

Timing erst nach dem Build bewerten und dabei keine weiteren Compiler-/Testjobs
starten. Protokolle: `C:/Temp/drdj-load-debug-baseline.log`,
`C:/Temp/drdj-load-user-debug-baseline.log`, `C:/Temp/drdj-load-release-baseline.log`,
`C:/Temp/drdj-load-optimized1.log`. Die finale Artefaktwiederholung wird im
Prüfstand der UI-Migration ergänzt.

## Finale Wiederholung nach allen Builds

Debug- und Release-App wurden mit `cargo build --locked` und
`cargo build --release --locked` erfolgreich gebaut (Exit 0). Anschließend ohne
weitere Compilerjobs gemessen:

| Datei | Deck A bereit | Deck B bereit |
| --- | ---: | ---: |
| `01 ritmo latino- Unbekannter Titel.mp3` | 3,42 s | 3,18 s |
| `02 -variado desconocido.mp3` | 2,57 s | 2,41 s |
| `bachata especial.mp3` | 3,15 s | 3,20 s |
| generierte 4-min-MP3 | 2,97 s | 3,06 s |
| generierte 4-min-WAV | 2,95 s | 2,95 s |
| `especia bachats.mp3` (310,36 s, Stereo 44,1 kHz) | 3,97 s | 3,94 s |

Alle zwölf Ladevorgänge bestanden die unveränderte Fünf-Sekunden-Assertion.
Höchstwert: 3,972 s (aufgerundet). Der vollständige erfolgreiche Messlauf samt
getrennten Phasen steht in `C:/Temp/drdj-load-final-verified.log`.
Die Laufzeitgrenzen und die noch offene native Abnahme gelten weiterhin.
