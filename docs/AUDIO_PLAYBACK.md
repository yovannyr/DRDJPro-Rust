# Audio output spike

This slice adds actual output-device playback independently of the Slint GUI.

## Try it on your own computer

```sh
cargo run --release -- --play /path/to/music.mp3
cargo run --release -- --mix /path/to/deck-a.mp3 /path/to/deck-b.mp3
cargo run --release -- --analyze /path/to/music.mp3
```

The first command opens the OS default output device via Rodio/CPAL and plays one MP3 or WAV. The second creates two independent Rodio sinks, starts both music files simultaneously, and mixes them onto one output device at a fixed centered crossfader position. It requires a working local sound device. The third command decodes and analyzes audio without an audio device.

## What automated CI validates

* Native compilation for Windows x64, macOS ARM64 and Linux x64.
* Headless Rodio sink transport operations and linear crossfader gain math without requiring sound hardware.
* MP3 and WAV decoding and validation using the synthetic PCM fixture and short freely usable MP3 from Samplelib.
* Slint UI smoke tests and code formatting/lints.

**CI cannot verify audible output on physical equipment.** Perform a local listening test before declaring the real-time playback integration complete. Do not describe this proof-of-concept as a professional DJ audio engine: there is no independent headphone cue, DVS, tempo adjustment, device routing selector or low-latency measurement. The crossfader is a simple linear gain control, not a DJ-standard equal-power curve. The GUI Play/Pause buttons still control a simulation; they are not connected to the audio device.

## Next slice

Build a persistent application-owned dual-deck controller with reliable load/reload/seek/cue behavior, deterministic offline rendered mix tests and proper device-loss/error handling before binding it to the Slint UI.
