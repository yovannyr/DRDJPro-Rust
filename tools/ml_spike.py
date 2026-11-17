"""Explicit A0 runtime fetch and isolated CPU probe; Python stdlib only."""
import argparse
import ctypes
import hashlib
import json
import os
import queue
from pathlib import Path
import subprocess
import tempfile
import threading
import urllib.request
import zipfile

ORT_URL = "https://github.com/microsoft/onnxruntime/releases/download/v1.31.0/onnxruntime-win-x64-1.31.0.zip"
ORT_SHA256 = "3958d8a44984160692c1b4ffd0de3cb48f8c39f9d34bb31e433db06e30b82195"
ORT_BYTES = 82_887_349


def extract_verified_runtime(archive, destination):
    """Verify before extraction; extract only explicit CPU DLLs/license files."""
    if archive.stat().st_size != ORT_BYTES:
        raise ValueError("runtime archive size mismatch")
    with archive.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    if digest != ORT_SHA256:
        raise ValueError("runtime archive SHA256 mismatch")
    root = "onnxruntime-win-x64-1.31.0/"
    allowed = {root + name for name in ("LICENSE", "ThirdPartyNotices.txt",
                                       "lib/onnxruntime.dll", "lib/onnxruntime_providers_shared.dll")}
    with zipfile.ZipFile(archive) as zipped:
        selected = [entry for entry in zipped.infolist() if entry.filename in allowed]
        if not any(e.filename == root + "lib/onnxruntime.dll" for e in selected):
            raise ValueError("archive lacks CPU runtime")
        if sum(e.file_size for e in selected) > 200 * 1024 * 1024:
            raise ValueError("extracted runtime exceeds budget")
        destination.mkdir(parents=True, exist_ok=True)
        for entry in selected:
            # Flatten a fixed verified archive; never use extract/extractall paths.
            target = destination / Path(entry.filename).name
            target.write_bytes(zipped.read(entry))
    return destination / "onnxruntime.dll"


def fetch_runtime(destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="ml-download-", dir=destination.parent) as staging:
        archive = Path(staging) / "runtime.zip"
        request = urllib.request.Request(ORT_URL, headers={"User-Agent": "DRDJPro-A0"})
        with urllib.request.urlopen(request, timeout=60) as response, archive.open("wb") as output:
            size = 0
            while chunk := response.read(1024 * 1024):
                size += len(chunk)
                if size > ORT_BYTES:
                    raise ValueError("runtime download exceeds fixed byte count")
                output.write(chunk)
        runtime = extract_verified_runtime(archive, destination)
    return {"runtime": str(runtime.resolve()), "source": ORT_URL,
            "archive_sha256": ORT_SHA256, "archive_bytes": ORT_BYTES,
            "retained_bytes": sum(p.stat().st_size for p in destination.iterdir() if p.is_file())}


def peak_memory_windows(process):
    """Read exact process peak working set; no sampling-based peak estimate."""
    from ctypes import wintypes

    class Counters(ctypes.Structure):
        _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD),
                    *[(field, ctypes.c_size_t) for field in (
                        "PeakWorkingSetSize", "WorkingSetSize", "QuotaPeakPagedPoolUsage",
                        "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage", "QuotaNonPagedPoolUsage",
                        "PagefileUsage", "PeakPagefileUsage")]]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    psapi = ctypes.WinDLL("psapi", use_last_error=True)
    psapi.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    psapi.GetProcessMemoryInfo.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x0400 | 0x0010, False, process.pid)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        # Child waits for ACK after completing inference, so it is still alive.
        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
            raise ctypes.WinError(ctypes.get_last_error())
        return counters.PeakWorkingSetSize
    finally:
        kernel.CloseHandle(handle)


def probe(binary, backend, model, runtime):
    environment = os.environ.copy()
    environment["DRDJ_ML_PROBE_HOLD"] = "1"
    if runtime:
        environment["DRDJ_ORT_LIBRARY"] = str(runtime.resolve())
    rows = []
    for threads in (1, 2, 4):
        environment["OMP_NUM_THREADS"] = str(threads)
        environment["OMP_WAIT_POLICY"] = "PASSIVE"
        process = subprocess.Popen([str(binary.resolve()), backend, str(model.resolve()), str(threads)],
                                   stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   text=True, env=environment)
        try:
            ready = queue.Queue(maxsize=1)
            threading.Thread(target=lambda: ready.put(process.stdout.readline()), daemon=True).start()
            line = ready.get(timeout=120)
            if not line:
                _, stderr = process.communicate(timeout=120)
                raise RuntimeError(f"probe exited before reporting results ({process.returncode}): {stderr}")
            row = json.loads(line)
            peak = peak_memory_windows(process) if os.name == "nt" else None
            process.stdin.write("ACK\n")
            process.stdin.flush()
            stdout, stderr = process.communicate(timeout=120)
            if stdout.strip():
                raise RuntimeError("unexpected extra probe output")
        except BaseException:
            process.kill()
            process.communicate()
            raise
        if process.returncode:
            raise RuntimeError(f"{backend}/{threads} failed: {stderr}")
        row["process_peak_working_set_bytes"] = peak
        rows.append(row)
    return {"schema": 1, "host": os.name, "runtime": str(runtime) if runtime else None, "measurements": rows}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    fetch = commands.add_parser("fetch-ort")
    fetch.add_argument("--destination", type=Path, required=True)
    measure = commands.add_parser("probe")
    measure.add_argument("--binary", type=Path, required=True)
    measure.add_argument("--backend", choices=("ort", "tract"), required=True)
    measure.add_argument("--model", type=Path, required=True)
    measure.add_argument("--runtime", type=Path)
    measure.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if args.command == "fetch-ort":
        print(json.dumps(fetch_runtime(args.destination)))
    else:
        result = probe(args.binary, args.backend, args.model, args.runtime)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
        print(json.dumps(result))


if __name__ == "__main__":
    main()
