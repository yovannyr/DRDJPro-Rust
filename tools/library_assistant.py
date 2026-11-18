"""Read the DrDJ SQLite library locally. Run with --help for CLI and MCP modes."""

import argparse
import heapq
import json
import re
import sqlite3
from contextlib import contextmanager
from functools import lru_cache
from itertools import pairwise
from pathlib import Path

from library_similarity import (
    build_dictionary,
    cosine,
    descriptors,
    load_dictionary,
    text_projection,
)


@contextmanager
def library(path):
    db = sqlite3.connect(
        Path(path).resolve(strict=True).as_uri() + "?mode=ro", uri=True, timeout=2
    )
    db.row_factory = sqlite3.Row
    try:
        db.create_function("filename", 1, filename, deterministic=True)
        db.execute("PRAGMA trusted_schema=OFF")
        db.execute("PRAGMA query_only=ON")
        db.execute("BEGIN")
        yield db
    finally:
        db.close()


def limit_rows(limit):
    if type(limit) is not int or not 1 <= limit <= 50:
        raise ValueError("limit must be between 1 and 50")
    return limit


def filename(path):
    return path.replace("\\", "/").rsplit("/", 1)[-1]


SELECT = """SELECT t.id,t.path,m.title,m.artist,m.duration_ms,a.result,
                   g.bpm manual_bpm,g.anchor_ns,g.tempo_changes,g.warp_markers FROM tracks t
            LEFT JOIN track_metadata m ON m.track_id=t.id
            LEFT JOIN track_analysis a ON a.track_id=t.id
            LEFT JOIN track_grids g ON g.track_id=t.id """


def metadata(row):
    return {
        "id": row["id"],
        "title": row["title"] or filename(row["path"]),
        "artist": row["artist"] or "",
        "duration_ms": row["duration_ms"],
    }


def measured(row):
    a = json.loads(row["result"]) if row["result"] else {}
    if a.get("is_corrupt"):
        a = {}
    if row["manual_bpm"] and (
        json.loads(row["tempo_changes"]) or json.loads(row["warp_markers"])
    ):
        a.update(
            bpm=0,
            beat_anchor_ms=row["anchor_ns"] // 1_000_000,
            grid_source="variable saved grid",
        )
    elif row["manual_bpm"]:
        a.update(
            bpm=row["manual_bpm"],
            beat_anchor_ms=row["anchor_ns"] // 1_000_000,
            grid_source="saved grid",
        )
    elif a.get("beat_anchor_ms") is not None:
        a["grid_source"] = "neural grid proposal"
    else:
        a["grid_source"] = "classical estimate"
    return a


def search(db, query="", after=0, limit=20):
    limit_rows(limit)
    if len(query) > 256 or after < 0:
        raise ValueError("invalid search query or cursor")
    # Keyset paging stays stable when the app imports higher track ids.
    rows = db.execute(
        SELECT
        + """WHERE t.id>? AND (instr(lower(coalesce(m.title,'')),lower(?))>0
        OR instr(lower(coalesce(m.artist,'')),lower(?))>0 OR instr(lower(filename(t.path)),lower(?))>0)
        ORDER BY t.id LIMIT ?""",
        (after, query, query, query, limit),
    )
    return [metadata(row) for row in rows]


def get_row(db, track_id):
    row = db.execute(SELECT + "WHERE t.id=?", (track_id,)).fetchone()
    if row is None:
        raise ValueError("unknown track id")
    return row


def cues(db, track_id):
    return [
        dict(row)
        for row in db.execute(
            "SELECT slot,name,position_ns/1000000 position_ms FROM track_hot_cues WHERE track_id=? ORDER BY position_ns",
            (track_id,),
        )
    ]


def details(db, track_id):
    row = get_row(db, track_id)
    a = measured(row)
    return dict(
        metadata(row),
        bpm=a.get("bpm") or None,
        key=a.get("musical_key"),
        grid_source=a["grid_source"] if a.get("bpm") or row["manual_bpm"] else None,
        beat_anchor_ms=a.get("beat_anchor_ms"),
        cues=cues(db, track_id),
    )


def compatible(a, b):
    if not all(re.fullmatch(r"(?:[1-9]|1[0-2])[AB]", k or "") for k in (a, b)):
        return None
    return a[:-1] == b[:-1] or (
        a[-1] == b[-1] and (int(a[:-1]) - int(b[:-1])) % 12 in (1, 11)
    )


def recommend(db, track_id, limit, gram=None, transition=False):
    limit_rows(limit)
    source = measured(get_row(db, track_id))
    source_features = descriptors(source)
    if not source_features:
        return []

    source_exit = None
    intros = {}
    if transition:
        source_exit = next(
            (
                c["position_ms"]
                for c in reversed(cues(db, track_id))
                if c["name"].casefold() == "outro"
            ),
            None,
        )
        intros = dict(
            db.execute(
                "SELECT track_id,min(position_ns)/1000000 FROM track_hot_cues WHERE name='Intro' COLLATE NOCASE GROUP BY track_id"
            )
        )

    @lru_cache(maxsize=4096)
    def similarity(features):
        return cosine(source_features, features, gram)

    def ranked():
        for row in db.execute(
            SELECT + "WHERE t.id<>? AND (a.result IS NOT NULL OR g.bpm IS NOT NULL)",
            (track_id,),
        ):
            candidate = measured(row)
            features = descriptors(candidate)
            if not features:
                continue
            a, b = source.get("bpm", 0), candidate.get("bpm", 0)
            delta = 100 * abs(a - b) / a if a and b else None
            harmonic = compatible(
                source.get("musical_key"), candidate.get("musical_key")
            )
            semantic = similarity(features) if gram is not None else None
            if (transition or semantic is None) and delta is None:
                continue
            tempo = max(0.0, 1 - delta / 20) if delta is not None else 0.0
            score = semantic if semantic is not None else tempo
            if transition:
                # Renormalize when key or embedding evidence is missing.
                terms = [(0.5, tempo)]
                if source_exit is not None:
                    terms.append((0.15, float(row["id"] in intros)))
                if harmonic is not None:
                    terms.append((0.3, float(harmonic)))
                if semantic is not None:
                    terms.append((0.2, semantic))
                score = sum(w * v for w, v in terms) / sum(w for w, _ in terms)
            evidence = {
                "tempo_change_pct": round(delta, 2) if delta is not None else None,
                "camelot_compatible": harmonic,
                "descriptor_cosine": semantic,
                "source_grid": source["grid_source"],
                "target_grid": candidate["grid_source"],
            }
            yield (
                score,
                -row["id"],
                dict(metadata(row), score=round(score, 6), evidence=evidence),
            )

    results = [item for _, _, item in heapq.nlargest(limit, ranked())]
    if transition:
        for item in results:
            entry = intros.get(item["id"])
            target = measured(get_row(db, item["id"]))
            item.update(
                exit_cue_ms=source_exit,
                entry_ms=entry if entry is not None else target.get("beat_anchor_ms"),
                entry_source="Intro cue"
                if entry is not None
                else target["grid_source"]
                if target.get("beat_anchor_ms") is not None
                else None,
            )
    return results


def text_search(db, gram, projection, limit):
    """Cosine of a query against each song's pooled descriptor embedding."""
    limit_rows(limit)

    @lru_cache(maxsize=4096)
    def score(features):
        norm = sum(gram[i][j] for i in features for j in features)
        return (
            max(-1.0, min(1.0, sum(projection[i] for i in features) / norm**0.5))
            if norm > 0
            else -1.0
        )

    def rows():
        for row in db.execute(
            SELECT + "WHERE a.result IS NOT NULL OR g.bpm IS NOT NULL"
        ):
            features = descriptors(measured(row))
            if features:
                value = score(features)
                yield value, -row["id"], dict(metadata(row), score=round(value, 6))

    return [item for _, _, item in heapq.nlargest(limit, rows())]


def compare_history(db, gram, limit):
    """Compare adjacent known plays after the last possible hidden repeat."""
    limit_rows(limit)
    rows = list(
        db.execute(
            "SELECT track_id,last_played_ms,play_count FROM track_history ORDER BY last_played_ms,track_id"
        )
    )
    cutoff = max((r[1] for r in rows if r[2] > 1), default=-1)
    counts = {"scored_pairs": 0, "tempo_hits": 0, "descriptor_hits": 0, "top_k": limit}
    for i, (a, b) in enumerate(pairwise(rows)):
        if (
            a[1] < cutoff
            or not 0 < b[1] - a[1] <= 600_000
            or (i > 0 and rows[i - 1][1] == a[1])
            or (i + 2 < len(rows) and rows[i + 2][1] == b[1])
        ):
            continue
        if not all(measured(get_row(db, r[0])).get("bpm") for r in (a, b)):
            continue
        counts["scored_pairs"] += 1
        for name, dictionary in (("tempo_hits", None), ("descriptor_hits", gram)):
            counts[name] += int(
                any(r["id"] == b[0] for r in recommend(db, a[0], limit, dictionary))
            )
    return counts


def serve(path, gram):
    from mcp.server.fastmcp import FastMCP
    from mcp.types import ToolAnnotations

    app = FastMCP("DrDJ Pro Library")
    annotations = ToolAnnotations(
        readOnlyHint=True, destructiveHint=False, openWorldHint=False
    )

    @app.tool(annotations=annotations)
    def search_tracks(
        query: str = "", after_id: int = 0, limit: int = 20
    ) -> list[dict]:
        """Search titles, artists and filenames; page using the last returned ID."""
        with library(path) as db:
            return search(db, query, after_id, limit)

    @app.tool(annotations=annotations)
    def track_details(track_id: int) -> dict:
        """Read one track's preparation metadata, grid origin and named cues."""
        with library(path) as db:
            return details(db, track_id)

    @app.tool(annotations=annotations)
    def similar_tracks(track_id: int, limit: int = 10) -> list[dict]:
        """Rank local tracks by acoustic descriptors, with a tempo fallback."""
        with library(path) as db:
            return recommend(db, track_id, limit, gram)

    @app.tool(annotations=annotations)
    def transition_suggestions(track_id: int, limit: int = 5) -> list[dict]:
        """Explain candidate transitions and return supported cue positions."""
        with library(path) as db:
            return recommend(db, track_id, limit, gram, True)

    app.run(transport="stdio")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--library", type=Path)
    parser.add_argument("--dictionary", type=Path)
    subs = parser.add_subparsers(dest="command", required=True)
    build = subs.add_parser("build-dictionary")
    build.add_argument("endpoint")
    for name in ("details", "similar", "transition"):
        sub = subs.add_parser(name)
        sub.add_argument("track_id", type=int)
        if name != "details":
            sub.add_argument("--limit", type=int, default=5)
            sub.add_argument("--summary", action="store_true")
    lookup = subs.add_parser("search")
    lookup.add_argument("query", nargs="?", default="")
    lookup.add_argument("--after", type=int, default=0)
    lookup.add_argument("--limit", type=int, default=20)
    text = subs.add_parser("text")
    text.add_argument("query")
    text.add_argument("--endpoint", required=True)
    text.add_argument("--limit", type=int, default=5)
    compare = subs.add_parser("compare-history")
    compare.add_argument("--limit", type=int, default=50)
    subs.add_parser("serve")
    args = parser.parse_args()
    if args.command == "build-dictionary":
        if not args.dictionary:
            parser.error("specify --dictionary")
        build_dictionary(args.endpoint, args.dictionary)
        return
    if not args.library:
        parser.error("specify --library")
    gram = load_dictionary(args.dictionary) if args.dictionary else None
    if args.command == "serve":
        serve(args.library, gram)
        return
    if args.command == "text":
        if not args.dictionary:
            parser.error("text search requires --dictionary")
        gram, projection = text_projection(args.dictionary, args.endpoint, args.query)
    with library(args.library) as db:
        if args.command == "compare-history":
            if gram is None:
                parser.error("compare-history requires --dictionary")
            result = compare_history(db, gram, args.limit)
        elif args.command == "text":
            result = text_search(db, gram, projection, args.limit)
        elif args.command == "search":
            result = search(db, args.query, args.after, args.limit)
        elif args.command == "details":
            result = details(db, args.track_id)
        else:
            result = recommend(
                db, args.track_id, args.limit, gram, args.command == "transition"
            )
    if getattr(args, "summary", False):
        if not result:
            print("No suitable analyzed track")
            return
        r = result[0]
        evidence = r["evidence"]
        parts = [f"{r['artist'][:160]} {r['title'][:160]}".strip()]
        if evidence["tempo_change_pct"] is not None:
            parts.append(f"tempo difference {evidence['tempo_change_pct']:.1f}%")
        if evidence["camelot_compatible"] is not None:
            parts.append(
                "compatible keys"
                if evidence["camelot_compatible"]
                else "different keys"
            )
        if evidence["descriptor_cosine"] is not None:
            parts.append(f"descriptor similarity {evidence['descriptor_cosine']:.3f}")
        if r.get("exit_cue_ms") is not None:
            parts.append(f"exit {r['exit_cue_ms'] / 1000:.1f}s")
        if r.get("entry_ms") is not None:
            parts.append(f"entry {r['entry_ms'] / 1000:.1f}s")
        print(" | ".join(parts))
    else:
        print(json.dumps(result, ensure_ascii=True, allow_nan=False))


if __name__ == "__main__":
    main()
