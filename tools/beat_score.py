"""Score sorted beat/downbeat events with one-to-one matches within 70 ms."""
import argparse
import json
import math
from pathlib import Path


def score(reference, predicted):
    for times in (reference, predicted):
        if any(not math.isfinite(t) or t < 0 for t in times) or any(
                a >= b for a, b in zip(times, times[1:])):
            raise ValueError("timestamps must be finite, nonnegative and strictly increasing")
    i = j = matches = 0
    while i < len(reference) and j < len(predicted):
        delta = predicted[j] - reference[i]
        if delta < -70:
            j += 1
        elif delta > 70:
            i += 1
        else:
            matches += 1
            i += 1
            j += 1
    total = len(reference) + len(predicted)
    return {"matched": matches, "reference": len(reference), "predicted": len(predicted),
            "f1": 2 * matches / total if total else None}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("reference", type=Path)
    parser.add_argument("prediction", type=Path)
    args = parser.parse_args()
    ref, pred = (json.loads(p.read_text(encoding="utf8")) for p in (args.reference, args.prediction))
    print(json.dumps({k: score(ref[k], pred[k]) for k in ("beats_ms", "downbeats_ms")}))
