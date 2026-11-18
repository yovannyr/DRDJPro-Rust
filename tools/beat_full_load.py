"""Drive a real full-song Slice B load alongside the opt-in two-deck render test."""
import json
import os
from pathlib import Path
from statistics import median
import subprocess
import sys
import time


def checked_events(data):
    events = json.loads(data)
    for key in ("beats_ms", "downbeats_ms"):
        values = events[key]
        if not isinstance(values, list) or any(type(v) is not int or v < 0 for v in values):
            raise ValueError("invalid event times")
        if any(a >= b for a, b in zip(values, values[1:])):
            raise ValueError("unsorted event times")
    if not set(events["downbeats_ms"]).issubset(events["beats_ms"]):
        raise ValueError("downbeat outside beat set")
    return events


def track_summary(events):
    beats = events["beats_ms"]
    if len(beats) < 2 or not events["downbeats_ms"]:
        raise ValueError("insufficient beat or downbeat detections for dance track")
    intervals = [b - a for a, b in zip(beats, beats[1:])]
    return {"beats": len(beats), "downbeats": len(events["downbeats_ms"]),
            "first_ms": beats[0], "last_ms": beats[-1], "max_interval_ms": max(intervals),
            "median_bpm": round(60000 / median(intervals), 1)}


def run_track(path, binary, models, control=None):
    """Pipe decoded mono PCM into the actual example; reap both children on exit."""
    start = time.monotonic()
    decode = ["ffmpeg", "-nostdin", "-v", "error", "-xerror", "-i", str(path),
              "-map", "0:a:0", "-ac", "1", "-ar", "22050", "-f", "f32le", "-"]
    with subprocess.Popen(decode, stdout=subprocess.PIPE) as decoder:
        try:
            with subprocess.Popen([binary, models], stdin=decoder.stdout,
                                  stdout=subprocess.PIPE) as tracker:
                decoder.stdout.close()
                try:
                    if control is not None:
                        (control / "ready").touch()
                    while True:
                        try:
                            data, _ = tracker.communicate(timeout=0.25)
                            break
                        except subprocess.TimeoutExpired:
                            if control is not None and (control / "stop").exists():
                                return None
                            if time.monotonic() - start > 3600:
                                raise TimeoutError("track analysis exceeded one hour")
                    if tracker.returncode or decoder.wait(timeout=5):
                        raise RuntimeError("incomplete full-track analysis")
                    return checked_events(data)
                finally:
                    if tracker.poll() is None:
                        tracker.kill()
        finally:
            if decoder.poll() is None:
                decoder.kill()


def main(control):
    tracks = json.loads(Path(os.environ["DRDJ_B_TRACK_LIST"]).read_text(encoding="utf8"))
    if not isinstance(tracks, list) or not tracks or not all(
        isinstance(track, str) and Path(track).is_file() for track in tracks
    ):
        raise ValueError("expected a JSON array of existing audio paths")
    completed = 0
    while not (control / "stop").exists():
        idx = completed % len(tracks)
        start = time.monotonic()
        events = run_track(tracks[idx], os.environ["DRDJ_B_TRACKER"],
                           os.environ["DRDJ_BEAT_TEST_DIR"], control)
        if events is None:
            break
        print(json.dumps({"track": idx + 1, "seconds": round(time.monotonic() - start, 2),
                          **track_summary(events)}), flush=True)
        completed += 1
    if completed < len(tracks):
        raise RuntimeError("load interval did not cover the full track list")
    (control / "completed").write_text(str(completed), encoding="ascii")


if __name__ == "__main__":
    main(Path(sys.argv[1]))
