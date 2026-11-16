import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import crash_symbols as symbols

BUILD = {"commit": "a" * 40, "target": "x86_64-pc-windows-msvc", "profile": "release"}
HEADER = "MODULE windows x86_64 " + "A" * 32 + "1 app.pdb\n"


class SymbolTests(unittest.TestCase):
    def test_panic_location_must_belong_to_the_expected_frame(self):
        valid = "  5: app::panic_probe\n  at <redacted>/mod.rs:55:4\n  6: other"
        self.assertEqual(symbols.panic_source(valid, "panic_probe")["line"], 55)
        wrong = "  5: app::panic_probe\n  6: other\n at <redacted>/mod.rs:88"
        with self.assertRaises(ValueError):
            symbols.panic_source(wrong, "panic_probe")

    def test_module_paths_and_ids_cannot_escape_the_store(self):
        for line in [HEADER.replace("app.pdb", "../private"), HEADER.replace("A" * 32, "../secret"), HEADER.replace("app.pdb", ".."), "not symbols"]:
            with self.assertRaises(ValueError):
                symbols.module_header(line)

    def test_symbols_strip_personal_source_paths_and_keep_line_records(self):
        with tempfile.TemporaryDirectory() as root:
            raw = Path(root) / "raw.sym"
            raw.write_text(HEADER + "FILE 0 C:\\Users\\private\\source.rs\nFUNC 10 20 0 app::probe\n10 4 42 0\n", encoding="utf-8")
            module, relative = symbols.write_symbols(raw, Path(root) / "bundle")
            result = (Path(root) / "bundle" / relative).read_text(encoding="utf-8")
            self.assertNotIn("Users", result)
            self.assertNotIn("private", result)
            self.assertIn("FILE 0 source.rs\n", result)
            self.assertIn("10 4 42 0", result)
            self.assertEqual(module["debug_file"], "app.pdb")

    def test_identity_rejects_arbitrary_payloads(self):
        symbols.validate_build(BUILD)
        for key in BUILD:
            with self.assertRaises(ValueError):
                symbols.validate_build(dict(BUILD, **{key: "https://SECRET/Users/private"}))

    def analyze_fixture(self, transform=lambda walk: walk, modify=lambda bundle, manifest: None):
        with tempfile.TemporaryDirectory() as root:
            root = Path(root)
            bundle = root / "bundle"
            bundle.mkdir()
            dump = root / "fake.dmp"
            dump.write_bytes(b"mock parser input")
            raw = root / "raw.sym"
            raw.write_text(HEADER + "FILE 0 source.rs\n", encoding="utf-8")
            module, relative = symbols.write_symbols(raw, bundle)
            manifest = {"schema": 1, "module": module, "build": BUILD, "symbol_file": relative, "files": {relative: symbols.digest(bundle / relative)}}
            modify(bundle, manifest)
            (bundle / "manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
            walk = {"modules": [dict(module, loaded_symbols=True)], "crashing_thread": {"frames": [
                {"function": "app::native_probe", "file": "/home/private/source.rs", "line": 42, "trust": "context", "module": "C:\\private\\app.exe"}
            ]}, "threads": [{"frames": [{"function": "irrelevant_thread_probe", "file": "/secret/private.rs"}]}]}

            def run(args, output, **_):
                self.assertNotIn("--symbols-url", args)
                self.assertNotIn("--use-local-debuginfo", args)
                Path(output).write_text(json.dumps(transform(walk)), encoding="utf-8")

            with patch.object(symbols, "run_tool", run):
                return symbols.analyze(dump, bundle, root)

    def test_only_matched_crash_thread_is_reduced_and_paths_are_removed(self):
        result = self.analyze_fixture()
        self.assertEqual(result["frames"][0]["file"], "source.rs")
        self.assertEqual(result["frames"][0]["module"], "app.exe")
        text = json.dumps(result)
        self.assertNotIn("private", text)
        self.assertNotIn("irrelevant_thread", text)

    def test_wrong_module_id_missing_symbols_and_missing_crash_thread_fail(self):
        def mismatch(walk):
            walk["modules"][0]["debug_id"] = "B" * 33
            return walk

        def missing(walk):
            walk["modules"][0]["loaded_symbols"] = False
            return walk

        def no_crash(walk):
            del walk["crashing_thread"]
            return walk

        for transform in [mismatch, missing, no_crash]:
            with self.assertRaises(ValueError):
                self.analyze_fixture(transform)

    def test_modified_missing_and_traversal_symbol_files_fail_before_tools(self):
        def tamper(bundle, manifest):
            (bundle / manifest["symbol_file"]).write_text("changed", encoding="utf-8")

        def missing(bundle, manifest):
            (bundle / manifest["symbol_file"]).unlink()

        def traversal(_, manifest):
            manifest["files"]["../private"] = "a" * 64

        def uncovered(_, manifest):
            manifest["files"].clear()

        def wrong_header(bundle, manifest):
            path = bundle / manifest["symbol_file"]
            path.write_text(HEADER.replace("A" * 32, "B" * 32), encoding="utf-8")
            manifest["files"][manifest["symbol_file"]] = symbols.digest(path)

        for modify in [tamper, missing, traversal, uncovered, wrong_header]:
            with self.assertRaises(ValueError):
                self.analyze_fixture(modify=modify)

    def test_untrusted_frame_types_are_not_returned(self):
        def change(walk):
            frame = walk["crashing_thread"]["frames"][0]
            frame.update(function="/Users/secret/path", line="/secret", trust="/private")
            return walk

        frame = self.analyze_fixture(change)["frames"][0]
        self.assertEqual(frame["function"], "<redacted>")
        self.assertIsNone(frame["line"])
        self.assertIsNone(frame["trust"])

    def test_null_or_malformed_optional_parser_fields_fail_explicitly(self):
        for key, value in [("modules", None), ("crashing_thread", None), ("crashing_thread", {"frames": None}), ("crashing_thread", {"frames": [None]})]:
            def change(walk):
                walk[key] = value
                return walk
            with self.assertRaises(ValueError):
                self.analyze_fixture(change)

    def test_malformed_manifest_types_are_rejected_before_processing(self):
        for key, value in [("module", []), ("files", []), ("build", []), ("schema", True)]:
            def change(_, manifest):
                manifest[key] = value
            with self.assertRaises(ValueError):
                self.analyze_fixture(modify=change)


if __name__ == "__main__":
    unittest.main()
