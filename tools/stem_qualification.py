"""Measure per-source waveform SDR on a controlled stereo mix, including a model-window seam."""

import argparse
import math
import subprocess
import tempfile
import time
import wave
from array import array
from pathlib import Path

RATE = 44100
SECONDS = 14


def reference(path):
    pcm = subprocess.check_output(
        [
            "ffmpeg",
            "-v",
            "error",
            "-nostdin",
            "-stream_loop",
            "-1",
            "-i",
            str(path),
            "-t",
            str(SECONDS),
            "-ar",
            str(RATE),
            "-ac",
            "1",
            "-f",
            "f32le",
            "-",
        ]
    )
    samples = array("f", pcm)
    if len(samples) != RATE * SECONDS or max(map(abs, samples)) == 0:
        raise ValueError("expected non-silent full-length reference")
    scale = 0.19 / max(map(abs, samples))
    return array("f", (s * scale for s in samples))


def sdr(ref, est):
    error = sum((x - y) ** 2 for x, y in zip(ref, est, strict=True))
    return 10 * math.log10(sum(x * x for x in ref) / max(error, 1e-20))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("binary", type=Path)
    p.add_argument("speech", type=Path)
    p.add_argument("drums", type=Path)
    p.add_argument(
        "--panned",
        action="store_true",
        help="second arrangement with distinct stereo positions",
    )
    args = p.parse_args()
    speech, drums = reference(args.speech), reference(args.drums)
    bass, keys = array("f"), array("f")
    for i in range(RATE * SECONDS):
        t = i / RATE
        bass.append(
            0.12
            * math.sin(2 * math.pi * (55, 55, 65, 49)[int(t) % 4] * t)
            * math.exp(-2 * (t % 0.5))
        )
        keys.append(
            0.05
            * sum(math.sin(2 * math.pi * f * t) for f in (330, 415, 494))
            * math.exp(-4 * (t % 1))
        )
    gains = (
        [(1.0, 0.5), (0.45, 1.0), (0.75, 0.75), (1.0, 0.3)]
        if args.panned
        else [(1.0, 1.0)] * 4
    )
    refs = [
        array("f", (x * g for x in ref for g in gain))
        for ref, gain in zip((speech, drums, bass, keys), gains)
    ]
    mixture = array("f", map(sum, zip(*refs)))
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / "reference.wav"
        with wave.open(str(path), "wb") as w:
            w.setparams((2, 2, RATE, 0, "NONE", ""))
            w.writeframes(array("h", (round(s * 32767) for s in mixture)).tobytes())
        total = array("f", [0.0]) * len(mixture)
        for i, ref in enumerate(refs):
            start = time.monotonic()
            data = subprocess.check_output(
                [
                    str(args.binary),
                    "--stem-mix",
                    str(path),
                    *[str(int(i == j)) for j in range(4)],
                ]
            )
            result = array("f", data)
            print(
                ("vocals", "drums", "bass", "other")[i],
                "SDR dB",
                round(sdr(ref, result), 2),
                "mix baseline",
                round(sdr(ref, mixture), 2),
                "wall seconds",
                round(time.monotonic() - start, 2),
                flush=True,
            )
            for j, x in enumerate(result):
                total[j] += x
        fidelity = sdr(mixture, total)
        print("reconstructed mix SDR dB", round(fidelity, 2))
        if fidelity < 60:
            raise ValueError("reconstruction lost alignment or content")


if __name__ == "__main__":
    main()
