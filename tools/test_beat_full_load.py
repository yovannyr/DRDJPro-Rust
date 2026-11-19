import json
import os
from pathlib import Path
import shutil
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch
import wave

import beat_full_load as load


class FullTrackLoadTests(unittest.TestCase):
    def test_rejects_invalid_output(self):
        for beats, downs in [([100, 100], []), ([2, 1], []), ([-1], []),
                             ([True], []), ([1.5], []), ([0, 5], [3])]:
            with self.subTest(beats=beats, downs=downs), self.assertRaises(ValueError):
                load.checked_events(json.dumps({"beats_ms": beats, "downbeats_ms": downs}))

    def test_accepts_ordered_beats_including_zero(self):
        events = {"beats_ms": [0, 500, 1000], "downbeats_ms": [0]}
        self.assertEqual(load.checked_events(json.dumps(events)), events)

    def test_summary_exposes_tempo_and_internal_gaps(self):
        self.assertEqual(load.track_summary({"beats_ms": [0, 500, 1000, 2500], "downbeats_ms": [0]}),
                         {"beats": 4, "downbeats": 1, "first_ms": 0, "last_ms": 2500,
                          "max_interval_ms": 1500, "median_bpm": 120})
        with self.assertRaises(ValueError):
            load.track_summary({"beats_ms": [100], "downbeats_ms": []})

    @unittest.skipUnless(shutil.which("ffmpeg"), "needs FFmpeg")
    def test_worker_consumes_pcm_through_eof_and_rejects_decoder_failure(self):
        with TemporaryDirectory() as directory:
            source = Path(directory) / "song.wav"
            with wave.open(str(source), "wb") as writer:
                writer.setparams((1, 2, 22050, 0, "NONE", ""))
                writer.writeframes(bytes(22050 * 2))
            tracker = Path(directory) / "probe.py"
            tracker.write_text(
                "import sys\nassert len(sys.stdin.buffer.read()) == 22050 * 4\n"
                "print('{\"beats_ms\":[0,500],\"downbeats_ms\":[0]}')\n", encoding="utf8")
            self.assertEqual(load.run_track(source, sys.executable, tracker)["beats_ms"], [0, 500])
            # A successful tracker must not mask FFmpeg's independent failure.
            tracker.write_text("import sys\nsys.stdin.buffer.read()\n"
                               "print('{\"beats_ms\":[],\"downbeats_ms\":[]}')\n", encoding="utf8")
            with self.assertRaisesRegex(RuntimeError, "incomplete"):
                load.run_track(source.with_suffix(".missing"), sys.executable, tracker)

    def test_completion_requires_every_track_and_excludes_interrupted_song(self):
        for expected in (1, 2):
            with self.subTest(expected=expected), TemporaryDirectory() as directory:
                control = Path(directory)
                paths = [control / name for name in ("first", "second")]
                for path in paths:
                    path.touch()
                manifest = control / "files.json"
                manifest.write_text(json.dumps(list(map(str, paths))), encoding="utf8")
                env = {"DRDJ_B_TRACK_LIST": str(manifest), "DRDJ_B_TRACKER": "tracker",
                       "DRDJ_BEAT_TEST_DIR": "weights"}
                events = {"beats_ms": [100, 600], "downbeats_ms": [100]}
                with patch.dict(os.environ, env), patch.object(
                    load, "run_track", side_effect=[events] * expected + [None]
                ) as analyse:
                    if expected < len(paths):
                        with self.assertRaisesRegex(RuntimeError, "full track list"):
                            load.main(control)
                        self.assertFalse((control / "completed").exists())
                    else:
                        load.main(control)
                        self.assertEqual((control / "completed").read_text(), str(expected))
                    self.assertEqual([call.args[0] for call in analyse.call_args_list],
                                     [str(paths[i % len(paths)]) for i in range(expected + 1)])
