# Audio output spike

This slice adds actual output-device playback independently of the Slint GUI.

## Try it on your own computer

```sh
cargo run --release -- --play /path/to/music.mp3
cargo run --release -- --analyze /path/to/music.mp3
```

The first command opens the OS default output device via Rodio/CPAL, decodes a local MP3 or WAV using Symphonia and blocks until playback ends. It requires a working local sound device. The second command decodes and analyzes audio without an audio device.

## What automated CI validates

* Native compilation for Windows x64, macOS ARM64 and Linux x64.
* Headless Rodio sink transport operations without requiring sound hardware.
* MP3 and WAV decoding and validation using the synthetic PCM fixture and short freely usable MP3 from Samplelib.
* Slint UI smoke tests and code formatting/lints.

**CI cannot verify audible output on physical equipment.** Perform a local listening test before declaring the real-time playback integration complete. Do not describe this proof-of-concept as a professional DJ audio engine: there is no deck-to-output routing, independent headphone cue, DVS, tempo adjustment or low-latency measurement. The GUI Play/Pause buttons still control a simulation; they are not connected to the audio device.

## Next slice

Implement a reusable audio engine with dual independently controlled decks, shared device output/mixing, seek and cue, and deterministic offline audio-mix tests before binding it to the Slint UI.
