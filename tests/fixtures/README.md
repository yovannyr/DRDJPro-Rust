# Test fixture

`tone-9s.mp3` is an original synthetic 440 Hz sine tone (9 seconds, stereo, 44.1 kHz), generated locally for codec/load regression tests. It contains no third-party music. Tests read it from the repository and do not need ffmpeg or network access.

Generation command (ffmpeg with libmp3lame):

```sh
ffmpeg -hide_banner -loglevel error -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=9" -ac 2 -codec:a libmp3lame -b:a 32k tests/fixtures/tone-9s.mp3
```

WAV files are generated deterministically inside tests and removed through RAII.
