"""Contract checks for the deterministic stdlib-only test graph/reference."""
import hashlib
import json
from pathlib import Path
import unittest

import ml_fixture


class FixtureTests(unittest.TestCase):
    def test_checked_in_reference_matches_generator(self):
        path = Path(__file__).resolve().parents[1] / "crates/drdj-ml/tests/reference.json"
        self.assertEqual(json.loads(path.read_text(encoding="utf-8")), ml_fixture.manifest())

    def test_fixture_is_tiny_deterministic_and_sensitive_to_input(self):
        model = ml_fixture.model_bytes()
        self.assertLess(len(model), 1024)
        self.assertEqual(hashlib.sha256(model).hexdigest(), ml_fixture.manifest()["model_sha256"])
        results = [ml_fixture.reference(x) for x in ml_fixture.INPUTS]
        self.assertTrue(all(len(y) == 12 for y in results))
        self.assertNotEqual(results[0], results[1])
        self.assertNotEqual(results[1], results[2])
        self.assertRaises(ValueError, ml_fixture.varint, -1)


if __name__ == "__main__":
    unittest.main()
