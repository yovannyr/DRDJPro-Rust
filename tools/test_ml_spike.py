"""No-network regressions for the explicitly requested runtime fetch."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

import ml_spike


class RuntimeTests(unittest.TestCase):
    def test_incomplete_verified_wheel_leaves_destination_untouched(self):
        with tempfile.TemporaryDirectory() as folder:
            archive = Path(folder) / "runtime.whl"
            destination = Path(folder) / "runtime"
            with zipfile.ZipFile(archive, "w") as zipped:
                zipped.writestr("onnxruntime/capi/onnxruntime.dll", b"fixture")
            data = archive.read_bytes()
            with patch.object(ml_spike, "ORT_BYTES", len(data)), patch.object(
                    ml_spike, "ORT_SHA256", hashlib.sha256(data).hexdigest()):
                with self.assertRaisesRegex(ValueError, "DLL and license set"):
                    ml_spike.extract_verified_runtime(archive, destination)
            self.assertFalse(destination.exists())

    def test_verified_archive_extracts_only_fixed_files_and_preserves_neighbors(self):
        with tempfile.TemporaryDirectory() as folder:
            base = Path(folder)
            archive = base / "runtime.zip"
            prefix = "onnxruntime/"
            with zipfile.ZipFile(archive, "w") as zipped:
                zipped.writestr(prefix + "capi/onnxruntime.dll", b"fixture")
                zipped.writestr(prefix + "LICENSE", b"license")
                zipped.writestr(prefix + "ThirdPartyNotices.txt", b"notices")
                zipped.writestr(prefix + "capi/onnxruntime_providers_shared.dll", b"provider")
                zipped.writestr(prefix + "capi/../../escape.dll", b"reject")
                zipped.writestr(prefix + "capi/unexpected.dll", b"reject")
            data = archive.read_bytes()
            destination = base / "runtime"
            with patch.object(ml_spike, "ORT_BYTES", len(data)), patch.object(
                    ml_spike, "ORT_SHA256", hashlib.sha256(data).hexdigest()):
                result = ml_spike.extract_verified_runtime(archive, destination)
                self.assertEqual(result.read_bytes(), b"fixture")
                self.assertEqual({p.name for p in destination.iterdir()}, {"onnxruntime.dll", "onnxruntime_providers_shared.dll",
                                  "LICENSE", "ThirdPartyNotices.txt"})
                archive.write_bytes(b"x" * len(data))
                with self.assertRaisesRegex(ValueError, "SHA256"):
                    ml_spike.extract_verified_runtime(archive, destination)
                self.assertEqual(result.read_bytes(), b"fixture")
                archive.write_bytes(b"x")
                with self.assertRaisesRegex(ValueError, "size"):
                    ml_spike.extract_verified_runtime(archive, destination)
            self.assertFalse((base / "escape.dll").exists())


if __name__ == "__main__":
    unittest.main()
