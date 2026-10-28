# DRDJPro-Rust

## Audio-Dateien

Der lokale Import und beide Decks unterstützen MP3, WAV, M4A mit AAC-LC oder ALAC, FLAC, AIFF/AIF Ogg/Vorbis (OGG/OGA) und Ogg/Opus (OPUS/OGG/OGA, Mono oder Stereo). Die tatsächliche Unterstützung hängt vom enthaltenen Codec ab; beschädigte, verschlüsselte oder anders codierte Dateien können beim Laden abgelehnt werden. Verkettete oder multiplexierte Opus-Streams und Opus mit mehr als zwei Kanälen werden derzeit abgelehnt. Die Codecs wurden mit synthetischen Fixtures headless geprüft; eine Hör- oder Geräteabnahme ist damit nicht verbunden.
