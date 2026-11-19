"""Local acoustic-description similarity for analyzed DrDJ tracks.

Only the fixed vocabulary below is embedded by the configured model. A song
is represented by the sum of its descriptor vectors. Their Gram matrix lets
us compute the exact cosine of these sums without storing vectors per song.
"""

import base64
import bisect
import json
import math
import os
import struct
import tempfile
import urllib.request
from pathlib import Path
from urllib.parse import urlsplit

MODEL = "qwen-3-vl-embedding-2b-sovereign"
TEMPOS = (
    "slow pulse below 80 BPM",
    "relaxed 80 to 99 BPM",
    "medium 100 to 114 BPM",
    "steady dance tempo 115 to 129 BPM",
    "fast pulse 130 to 149 BPM",
    "very fast 150 BPM or above",
)
LEVELS = ("quiet", "moderate", "loud")
WORDS = (
    *TEMPOS,
    "minor mode",
    "major mode",
    *(
        f"{part} section has {level} amplitude"
        for part in ("opening", "middle", "closing")
        for level in LEVELS
    ),
    "steady dynamics",
    "changing dynamics",
    "strong dynamic contrasts",
)


def descriptors(analysis):
    """Derive only descriptors supported by measurements in the library."""
    indices = []
    bpm = analysis.get("bpm", 0)
    if 40 <= bpm <= 240:
        indices.append(bisect.bisect_right((80, 100, 115, 130, 150), bpm))
    key = analysis.get("musical_key", "")
    if key and key[-1:] in ("A", "B"):
        indices.append(6 if key.endswith("A") else 7)
    peaks = analysis.get("waveform_overview", [])
    if len(peaks) >= 6:
        for i in range(3):
            part = peaks[i * len(peaks) // 3 : (i + 1) * len(peaks) // 3]
            indices.append(
                8 + 3 * i + bisect.bisect_right((85, 170), sum(part) / len(part))
            )
        mean = sum(peaks) / len(peaks)
        deviation = math.sqrt(sum((p - mean) ** 2 for p in peaks) / len(peaks))
        indices.append(17 + bisect.bisect_right((16, 45), deviation))
    return tuple(indices)


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args):
        return None


def embed(endpoint, texts):
    base = urlsplit(endpoint)
    if base.scheme != "https" and not (
        base.scheme == "http" and base.hostname in ("localhost", "127.0.0.1", "::1")
    ):
        raise ValueError("use HTTPS or a loopback embedding endpoint")
    if base.username or base.password or base.query or base.fragment:
        raise ValueError("invalid embedding endpoint")
    headers = {"Content-Type": "application/json"}
    if token := os.environ.get("DRDJ_EMBEDDING_API_KEY"):
        headers["Authorization"] = "Bearer " + token
    vectors = []
    client = urllib.request.build_opener(NoRedirect())
    for first in range(0, len(texts), 4):
        words = texts[first : first + 4]
        body = json.dumps(
            {"model": MODEL, "input": list(words), "encoding_format": "float"}
        ).encode()
        request = urllib.request.Request(
            endpoint.rstrip("/") + "/embeddings", body, headers
        )
        with client.open(request, timeout=45) as reply:
            data = json.loads(reply.read(2_000_001))["data"]
        if sorted(item["index"] for item in data) != list(range(len(words))):
            raise ValueError("incomplete embedding response")
        for item in sorted(data, key=lambda x: x["index"]):
            vector = item["embedding"]
            if not 1 <= len(vector) <= 8192 or (
                vectors and len(vectors[0]) != len(vector)
            ):
                raise ValueError("invalid embedding dimension")
            norm = math.sqrt(sum(x * x for x in vector))
            if not math.isfinite(norm) or norm <= 0:
                raise ValueError("invalid embedding norm")
            vectors.append([x / norm for x in vector])
    return vectors


def build_dictionary(endpoint, destination):
    vectors = embed(endpoint, WORDS)
    packed = [
        base64.b64encode(struct.pack(f"<{len(v)}e", *v)).decode("ascii")
        for v in vectors
    ]
    gram = [[sum(x * y for x, y in zip(a, b)) for b in vectors] for a in vectors]
    dest = Path(destination)
    with tempfile.NamedTemporaryFile(
        "w", encoding="utf8", dir=dest.parent, delete=False
    ) as f:
        tmp = Path(f.name)
        try:
            json.dump(
                {"model": MODEL, "words": WORDS, "gram": gram, "vectors": packed}, f
            )
            f.flush()
        except BaseException:
            f.close()
            tmp.unlink(missing_ok=True)
            raise
    try:
        os.replace(tmp, dest)
    finally:
        tmp.unlink(missing_ok=True)


def dictionary_data(path):
    with Path(path).open("rb") as f:
        raw = f.read(512_001)
    if len(raw) > 512_000:
        raise ValueError("oversized dictionary")
    return json.loads(raw)


def load_dictionary(path):
    data = dictionary_data(path)
    gram = data["gram"]
    if (
        data["model"] != MODEL
        or tuple(data["words"]) != WORDS
        or len(gram) != len(WORDS)
        or any(
            len(row) != len(WORDS)
            or any(not math.isfinite(x) or abs(x) > 1.00001 for x in row)
            for row in gram
        )
    ):
        raise ValueError("incompatible acoustic dictionary")
    return gram


def cosine(a, b, gram):
    if not a or not b:
        return None

    def dot(x, y):
        return sum(gram[i][j] for i in x for j in y)

    norm = dot(a, a) * dot(b, b)
    return max(-1.0, min(1.0, dot(a, b) / math.sqrt(norm))) if norm > 0 else None


def text_projection(path, endpoint, text):
    """Embed only the explicitly supplied query. Rank library tracks locally."""
    if not text.strip() or len(text) > 512:
        raise ValueError("query must contain 1 through 512 characters")
    gram = load_dictionary(path)
    packed = dictionary_data(path).get("vectors", [])
    if len(packed) != len(WORDS):
        raise ValueError("rebuild the dictionary to enable text queries")
    (query,) = embed(endpoint, [text])
    projection = []
    for encoded in packed:
        raw = base64.b64decode(encoded, validate=True)
        if len(raw) != 2 * len(query):
            raise ValueError("embedding dimension changed")
        vector = struct.unpack(f"<{len(query)}e", raw)
        if not all(math.isfinite(x) for x in vector):
            raise ValueError("invalid dictionary vector")
        projection.append(sum(a * b for a, b in zip(query, vector)))
    return gram, projection
