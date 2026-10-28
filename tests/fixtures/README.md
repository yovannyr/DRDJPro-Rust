# Test fixture

`tone-9s.mp3` is an original synthetic 440 Hz sine tone (9 seconds, stereo, 44.1 kHz), generated locally for codec/load regression tests. It contains no third-party music. Tests read it from the repository and do not need ffmpeg or network access.

Generation command (ffmpeg with libmp3lame):

```sh
ffmpeg -hide_banner -loglevel error -f lavfi -i "sine=frequency=440:sample_rate=44100:duration=9" -ac 2 -codec:a libmp3lame -b:a 32k tests/fixtures/tone-9s.mp3
```

WAV files are generated deterministically inside tests and removed through RAII.


The `format-aac.m4a`, `format-alac.m4a`, `format-flac.flac`, `format-aiff.aiff` and `format-vorbis.ogg` fixtures are original synthetic one-second 440 Hz tones at 48 kHz stereo. They carry the synthetic title `Fixture Tone` and artist `Fixture Artist` where the container supports these tags. They were generated locally with FFmpeg and contain no third-party music:

```sh
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=1" -ac 2 -metadata "title=Fixture Tone" -metadata "artist=Fixture Artist" -c:a aac tests/fixtures/format-aac.m4a
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=1" -ac 2 -metadata "title=Fixture Tone" -metadata "artist=Fixture Artist" -c:a alac tests/fixtures/format-alac.m4a
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=1" -ac 2 -metadata "title=Fixture Tone" -metadata "artist=Fixture Artist" -c:a flac tests/fixtures/format-flac.flac
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=1" -ac 2 -metadata "title=Fixture Tone" -metadata "artist=Fixture Artist" -c:a pcm_s16be tests/fixtures/format-aiff.aiff
ffmpeg -hide_banner -loglevel error -y -f lavfi -i "sine=frequency=440:sample_rate=48000:duration=1" -ac 2 -metadata "title=Fixture Tone" -metadata "artist=Fixture Artist" -c:a libvorbis tests/fixtures/format-vorbis.ogg
```
