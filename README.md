# Dr. DJ Pro

## Installation

Die [Testversionen auf GitHub](https://github.com/yovannyr/DRDJPro-Rust/releases) enthalten Installationspakete für vier Systeme:

| System | Installation |
| --- | --- |
| Windows x64 | Die MSI-Datei öffnen. Danach **Dr. DJ Pro** im Startmenü wählen. Die Installation gilt für das aktuelle Benutzerkonto. |
| macOS ab 14, Intel oder Apple Silicon | Das passende DMG öffnen und **Dr. DJ Pro** nach **Programme** ziehen. Auf einem Intel-Mac mit Sequoia 15.2 das Intel-Paket wählen. |
| Ubuntu 24.04 amd64 | Das DEB-Paket mit der Paketverwaltung installieren und **Dr. DJ Pro** im Anwendungsmenü öffnen. |

Die Testpakete für Windows haben keine Herausgebersignatur. Die Mac-Pakete sind lokal signiert (Ad-hoc-Signatur) und nicht von Apple notarisiert. Bibliotheken und Musiksammlungen sind vom Installationsverzeichnis getrennt.

[![Made with Slint](https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png)](https://slint.dev/)

## Bibliotheks-Leistungsmessung

Mit `DRDJ_LIBRARY_PERF_PATH` kann ein neuer absoluter JSON-Berichtspfad für einen nativen Messlauf angegeben werden. Der Bericht wird beim regulären Beenden gespeichert; ohne die Variable bleibt der Messmodus aus. Voraussetzungen, Grenzwerte und Testablauf: [Bibliotheks-Leistungsprotokoll](docs/LIBRARY_STORAGE.md#repeatable-native-library-performance-protocol-2026-10-30). Native Hör-/Bedienabnahme und wiederholte Messläufe bleiben erforderlich.

## Audio-Dateien

Der lokale Import und beide Decks unterstützen MP3, WAV, M4A mit AAC-LC oder ALAC, FLAC, AIFF/AIF Ogg/Vorbis (OGG/OGA) und Ogg/Opus (OPUS/OGG/OGA, Mono oder Stereo). Die tatsächliche Unterstützung hängt vom enthaltenen Codec ab; beschädigte, verschlüsselte oder anders codierte Dateien können beim Laden abgelehnt werden. Verkettete oder multiplexierte Opus-Streams und Opus mit mehr als zwei Kanälen werden derzeit abgelehnt. Die Codecs wurden mit synthetischen Fixtures headless geprüft; eine Hör- oder Geräteabnahme ist damit nicht verbunden.

Die lokale Bibliothek kann über **ADD FILES** einzelne Dateien oder über **ADD FOLDER** einen Ordner einschließlich seiner Unterordner importieren. Der Ordner wird im Hintergrund durchsucht; nicht unterstützte Dateien werden übersprungen und im Ergebnis mitgezählt. Pro Ordnerimport gelten die Grenzen von 5.000 unterstützten Audiodateien und 100.000 Verzeichniseinträgen. Symlink-Verzeichnisse werden nicht verfolgt; die Quelldateien werden nicht verschoben oder verändert.
