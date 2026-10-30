# DRDJPro-Rust

## Bibliotheks-Leistungsmessung

Mit `DRDJ_LIBRARY_PERF_PATH` kann ein neuer absoluter JSON-Berichtspfad für einen nativen Messlauf angegeben werden. Der Bericht wird beim regulären Beenden gespeichert; ohne die Variable bleibt der Messmodus aus. Voraussetzungen, Grenzwerte und Testablauf: [Bibliotheks-Leistungsprotokoll](docs/LIBRARY_STORAGE.md#repeatable-native-library-performance-protocol-2026-10-30). Native Hör-/Bedienabnahme und wiederholte Messläufe bleiben erforderlich.

## Audio-Dateien

Der lokale Import und beide Decks unterstützen MP3, WAV, M4A mit AAC-LC oder ALAC, FLAC, AIFF/AIF Ogg/Vorbis (OGG/OGA) und Ogg/Opus (OPUS/OGG/OGA, Mono oder Stereo). Die tatsächliche Unterstützung hängt vom enthaltenen Codec ab; beschädigte, verschlüsselte oder anders codierte Dateien können beim Laden abgelehnt werden. Verkettete oder multiplexierte Opus-Streams und Opus mit mehr als zwei Kanälen werden derzeit abgelehnt. Die Codecs wurden mit synthetischen Fixtures headless geprüft; eine Hör- oder Geräteabnahme ist damit nicht verbunden.
