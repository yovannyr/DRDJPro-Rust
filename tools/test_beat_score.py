import unittest
from beat_score import score


class ScoreTests(unittest.TestCase):
    def test_duplicates_near_one_reference_are_false_positives(self):
        self.assertEqual(score([100], [30, 170]), {
            "matched": 1, "reference": 1, "predicted": 2, "f1": 2/3})

    def test_edges_misses_empty_and_invalid(self):
        self.assertEqual(score([100, 500], [171, 570])["f1"], 0.5)
        self.assertIsNone(score([], [])["f1"])
        for invalid in ([2, 1], [0, 0], [-1], [float("nan")]):
            with self.assertRaises(ValueError):
                score(invalid, [])
