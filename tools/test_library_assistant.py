import asyncio
import importlib.util
import json
import sqlite3
import sys
import tempfile
import unittest
from contextlib import closing
from pathlib import Path
from unittest.mock import patch

import library_assistant as assistant
from library_similarity import (
    WORDS,
    build_dictionary,
    cosine,
    descriptors,
    embed,
    text_projection,
)


class LibraryAssistantTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.path = Path(self.tmp.name) / "DJ Library.sqlite3"
        db = sqlite3.connect(self.path)
        db.executescript("""
          CREATE TABLE tracks(id INTEGER PRIMARY KEY,path TEXT);
          CREATE TABLE track_metadata(track_id INTEGER PRIMARY KEY,title TEXT,artist TEXT,duration_ms INTEGER);
          CREATE TABLE track_analysis(track_id INTEGER PRIMARY KEY,result TEXT);
          CREATE TABLE track_grids(track_id INTEGER PRIMARY KEY,bpm REAL,anchor_ns INTEGER,tempo_changes TEXT DEFAULT '[]',warp_markers TEXT DEFAULT '[]');
          CREATE TABLE track_hot_cues(track_id INTEGER,slot INTEGER,name TEXT,position_ns INTEGER);
          CREATE TABLE track_history(track_id INTEGER,last_played_ms INTEGER,play_count INTEGER);
          INSERT INTO track_history VALUES(3,1000,1),(1,2000,3),(2,3000,1);
          INSERT INTO track_grids(track_id,bpm,anchor_ns) VALUES(1,120,750000000);
          INSERT INTO track_hot_cues VALUES(1,3,'Outro',90000000000),(2,0,'Intro',1000000000);
        """)
        for i, bpm, key in [(1, 160, "8A"), (2, 121, "9A"), (3, 170, "3B"), (4, 0, "")]:
            analysis = {
                "bpm": bpm,
                "musical_key": key,
                "waveform_overview": [90] * 32,
                "is_corrupt": i == 4,
            }
            db.execute("INSERT INTO tracks VALUES(?,?)", (i, f"/private/{i}.wav"))
            db.execute(
                "INSERT INTO track_metadata VALUES(?,?,?,?)",
                (i, f"Track {i}", "DJ", 120000),
            )
            db.execute(
                "INSERT INTO track_analysis VALUES(?,?)", (i, json.dumps(analysis))
            )
        db.commit()
        db.close()

    def test_local_catalog_paging_readonly_and_saved_grid_precedence(self):
        with assistant.library(self.path) as db:
            self.assertEqual(
                [r["id"] for r in assistant.search(db, "dj", 1, 2)], [2, 3]
            )
            self.assertEqual(assistant.search(db, "' OR 1=1 --"), [])
            info = assistant.details(db, 1)
            self.assertEqual(
                (info["grid_source"], info["bpm"], info["beat_anchor_ms"]),
                ("saved grid", 120, 750),
            )
            self.assertNotIn("/private", json.dumps(info))
            with self.assertRaises(sqlite3.OperationalError):
                db.execute("DELETE FROM tracks")
            with self.assertRaises(ValueError):
                assistant.recommend(db, 1, 51)

    def test_rank_evidence_and_cues_with_and_without_dictionary(self):
        gram = [[float(i == j) for j in range(len(WORDS))] for i in range(len(WORDS))]
        self.assertAlmostEqual(cosine((0, 4), (0, 4), gram), 1.0)
        self.assertFalse(descriptors({}))
        with assistant.library(self.path) as db:
            self.assertEqual(
                assistant.compare_history(db, gram, 1),
                {"scored_pairs": 1, "tempo_hits": 1, "descriptor_hits": 1, "top_k": 1},
            )
            self.assertEqual(assistant.filename(r"C:\Music\Track.wav"), "Track.wav")
        for dictionary in (None, gram):
            with assistant.library(self.path) as db:
                ranked = assistant.recommend(db, 1, 5, dictionary, True)
                self.assertEqual([x["id"] for x in ranked], [2, 3])
                self.assertEqual(
                    (ranked[0]["exit_cue_ms"], ranked[0]["entry_ms"]), (90000, 1000)
                )
                self.assertTrue(ranked[0]["evidence"]["camelot_compatible"])
                self.assertIsNone(ranked[1]["entry_ms"])

    def test_variable_grid_cannot_claim_a_constant_transition_tempo(self):
        for maps in (('[{"beat":64,"bpm":160}]', "[]"), ("[]", '[{"beat":64}]')):
            with closing(sqlite3.connect(self.path)) as db, db:
                db.execute(
                    "UPDATE track_grids SET tempo_changes=?,warp_markers=? WHERE track_id=1",
                    maps,
                )
            with assistant.library(self.path) as db:
                info = assistant.details(db, 1)
                self.assertIsNone(info["bpm"])
                self.assertEqual(info["grid_source"], "variable saved grid")
                self.assertEqual(assistant.recommend(db, 1, 5, None, True), [])
                self.assertNotIn(
                    1, [r["id"] for r in assistant.recommend(db, 2, 5, None, True)]
                )

    def test_text_query_uses_dictionary_vectors_and_ranks_measured_tracks(self):
        dest = Path(self.tmp.name) / "dictionary.json"
        basis = [[float(i == j) for j in range(len(WORDS))] for i in range(len(WORDS))]
        with patch("library_similarity.embed", return_value=basis) as provider:
            build_dictionary("unused", dest)
            provider.assert_called_once_with("unused", WORDS)
        with patch("library_similarity.embed", return_value=[basis[5]]) as provider:
            gram, projection = text_projection(dest, "unused", "fast music")
            provider.assert_called_once_with("unused", ["fast music"])
        with assistant.library(self.path) as db:
            results = assistant.text_search(db, gram, projection, 5)
            self.assertEqual(results[0]["id"], 3)
            self.assertNotIn(4, [r["id"] for r in results])
        with (
            patch("library_similarity.embed", return_value=[[1]]),
            self.assertRaisesRegex(ValueError, "dimension"),
        ):
            text_projection(dest, "unused", "fast")
        with self.assertRaisesRegex(ValueError, "HTTPS"):
            embed("http://remote.invalid", ["fast"])

    @unittest.skipUnless(importlib.util.find_spec("mcp"), "requires optional MCP SDK")
    def test_mcp_stdio_handshake_tools_paging_and_errors(self):
        from mcp import ClientSession, StdioServerParameters
        from mcp.client.stdio import stdio_client

        async def exercise():
            params = StdioServerParameters(
                command=sys.executable,
                args=[
                    "-B",
                    str(Path(assistant.__file__)),
                    "--library",
                    str(self.path),
                    "serve",
                ],
            )
            async with (
                stdio_client(params) as (read, write),
                ClientSession(read, write) as session,
            ):
                hello = await session.initialize()
                self.assertEqual(hello.serverInfo.name, "DrDJ Pro Library")
                self.assertEqual(
                    {t.name for t in (await session.list_tools()).tools},
                    {
                        "search_tracks",
                        "similar_tracks",
                        "track_details",
                        "transition_suggestions",
                    },
                )
                for name, args in [
                    ("search_tracks", {"after_id": 1, "limit": 1}),
                    ("similar_tracks", {"track_id": 1}),
                    ("transition_suggestions", {"track_id": 1}),
                    ("track_details", {"track_id": 2}),
                ]:
                    result = await session.call_tool(name, args)
                    self.assertFalse(result.isError)
                    self.assertNotIn("/private", str(result.content))
                self.assertTrue(
                    (
                        await session.call_tool("track_details", {"track_id": 999})
                    ).isError
                )

        asyncio.run(asyncio.wait_for(exercise(), timeout=45))
