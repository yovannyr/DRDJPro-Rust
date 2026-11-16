"""Release symbol bundles and explicit offline minidump analysis (stdlib only).

Analysis never changes reports, uploads data or supplies a symbol-server URL.
Only `fetch-tools` accesses the network, for fixed, hash-pinned upstream tools.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import time
import urllib.request
import zipfile

TOOLS = {
    "dump_syms": ("mozilla/dump_syms", "v2.3.9", {
        "x86_64-pc-windows-msvc": "bdf48486220708808a3e00aec78856ee1cce096189d47a1e2cb1c635f93bacc7",
        "x86_64-unknown-linux-gnu": "0fc852a86b00337407d9d423cc388a24c3b489ccaaedcf92623cad57af5ca8ad",
        "aarch64-apple-darwin": "4945b7e5de0d7ad403822635f0fb912b69990fc412741ff2b7b512e168c40a32",
    }),
    "minidump-stackwalk": ("rust-minidump/rust-minidump", "v0.27.0", {
        "x86_64-pc-windows-msvc": "f6f2d7f1665843c4a270cd13fcd1458fed3b19013ea5dd909e12bc3599b959f4",
        "x86_64-unknown-linux-gnu": "0020324c54cc359596e927ee907204f4c8d4da6718765536edcabaa1622122ad",
        "aarch64-apple-darwin": "5fcd4d25a5b0bbdc8faac6f4f81629bc67483c3eb458b86a8304733a7c6a2ca9",
    }),
}


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def tool(directory, name):
    return Path(directory).resolve() / (name + (".exe" if os.name == "nt" else ""))


def fetch_tools(directory, target):
    directory = Path(directory).resolve()
    directory.mkdir(parents=True, exist_ok=True)
    for name, (repo, version, hashes) in TOOLS.items():
        expected = hashes[target]
        ext = ".zip" if "windows" in target else ".tar.xz"
        url = f"https://github.com/{repo}/releases/download/{version}/{name}-{target}{ext}"
        with tempfile.TemporaryDirectory(dir=directory) as staging:
            archive = Path(staging) / ("download" + ext)
            with urllib.request.urlopen(url, timeout=60) as response, archive.open("wb") as out:
                size = 0
                while chunk := response.read(65536):
                    size += len(chunk)
                    if size > 32 * 1024 * 1024:
                        raise ValueError("Tool archive exceeds its download budget")
                    out.write(chunk)
            if digest(archive) != expected:
                raise ValueError("Tool archive checksum mismatch")
            filename = name + (".exe" if "windows" in target else "")
            binary = Path(staging) / filename
            # Read exactly the executable; never extract arbitrary archive paths,
            # links, installers or execute a remotely supplied shell script.
            if ext == ".zip":
                with zipfile.ZipFile(archive) as package:
                    matches = [i for i in package.infolist() if Path(i.filename).name == filename and not i.is_dir()]
                    if len(matches) != 1 or matches[0].file_size > 64 * 1024 * 1024:
                        raise ValueError("Unexpected tool archive contents")
                    with package.open(matches[0]) as source, binary.open("wb") as out:
                        shutil.copyfileobj(source, out, 65536)
            else:
                with tarfile.open(archive) as package:
                    matches = [i for i in package.getmembers() if Path(i.name).name == filename and i.isfile()]
                    if len(matches) != 1 or matches[0].size > 64 * 1024 * 1024:
                        raise ValueError("Unexpected tool archive contents")
                    with package.extractfile(matches[0]) as source, binary.open("wb") as out:
                        shutil.copyfileobj(source, out, 65536)
            binary.chmod(0o700)
            os.replace(binary, directory / filename)


def run_tool(args, output, timeout=120, budget=256 * 1024 * 1024):
    """File-backed, bounded-wait output; no huge stdout buffers or raw stderr logs."""
    with Path(output).open("wb") as out, tempfile.TemporaryFile() as errors:
        process = subprocess.Popen([str(a) for a in args], stdout=out, stderr=errors)
        until = time.monotonic() + timeout
        try:
            while process.poll() is None:
                if time.monotonic() >= until or os.fstat(out.fileno()).st_size > budget or os.fstat(errors.fileno()).st_size > 1024 * 1024:
                    raise ValueError("Symbol tool exceeded its time/output budget")
                time.sleep(0.02)
            if process.returncode != 0 or os.fstat(out.fileno()).st_size > budget:
                raise ValueError("Symbol tool failed; raw tool output is not published")
        finally:
            if process.poll() is None:
                process.kill()
                process.wait()


def basename(value):
    return value.replace("\\", "/").rsplit("/", 1)[-1][:256] if isinstance(value, str) else ""


def module_header(line):
    fields = line.strip().split(" ", 4)
    if len(fields) != 5 or fields[0] != "MODULE":
        raise ValueError("Missing Breakpad module header")
    _, system, cpu, debug_id, filename = fields
    if system.lower() not in ("windows", "linux", "mac") or cpu not in ("x86_64", "arm64") or not re.fullmatch(r"[A-Fa-f0-9]{33,48}", debug_id) or not re.fullmatch(r"[A-Za-z0-9_.-]{1,128}", filename) or filename in (".", ".."):
        raise ValueError("Unsafe module identity")
    return {"os": system, "cpu": cpu, "debug_id": debug_id.upper(), "debug_file": filename}


def write_symbols(raw, root):
    with Path(raw).open(encoding="utf-8") as source:
        first = source.readline(8192)
        module = module_header(first)
        relative = Path("symbols") / module["debug_file"] / module["debug_id"] / (module["debug_file"].removesuffix(".pdb") + ".sym")
        path = Path(root) / relative
        path.parent.mkdir(parents=True)
        with path.open("w", encoding="utf-8", newline="\n") as out:
            out.write(first)
            while line := source.readline(65537):
                if len(line) > 65536:
                    raise ValueError("Oversized symbol record")
                if line.startswith("FILE "):
                    fields = line.rstrip("\r\n").split(" ", 2)
                    if len(fields) != 3 or not fields[1].isdigit():
                        raise ValueError("Malformed source record")
                    line = f"FILE {fields[1]} {basename(fields[2])}\n"
                out.write(line)
    return module, relative.as_posix()


def create_bundle(binary, output, tools):
    binary, output = Path(binary).resolve(), Path(output).resolve()
    if not binary.is_file():
        raise ValueError("Release executable is missing")
    info = json.loads(subprocess.check_output([str(binary), "--crash-build-info"], text=True, timeout=10))
    validate_build(info)
    output.mkdir(parents=True, exist_ok=False)
    native = output / "native"
    native.mkdir()
    if binary.suffix == ".exe":
        symbols = binary.with_name(binary.stem.replace("-", "_") + ".pdb")
        if not symbols.is_file():
            raise ValueError("Matching PDB is missing")
        shutil.copy2(symbols, native / symbols.name)
    elif "darwin" in info["target"]:
        symbols = Path(str(binary) + ".dSYM")
        if not symbols.is_dir():
            raise ValueError("Packed dSYM is missing")
        shutil.copytree(symbols, native / symbols.name)
    else:
        symbols = binary  # Linux DWARF remains embedded; archive the exact ELF.
        shutil.copy2(binary, native / binary.name)
    with tempfile.TemporaryDirectory() as staging:
        raw = Path(staging) / "raw.sym"
        source = symbols / "Contents" / "Resources" / "DWARF" / binary.name if symbols.is_dir() else symbols
        run_tool([tool(tools, "dump_syms"), source], raw)
        module, relative = write_symbols(raw, output)
    manifest = {"schema": 1, "build": info, "binary": binary.name, "binary_sha256": digest(binary), "module": module, "symbol_file": relative,
                "tools": {name: version for name, (_, version, _) in TOOLS.items()},
                "files": {p.relative_to(output).as_posix(): digest(p) for p in sorted(output.rglob("*")) if p.is_file()}}
    (output / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def validate_build(info):
    if not isinstance(info, dict) or set(info) != {"commit", "target", "profile"} or not all(isinstance(info.get(k), str) and re.fullmatch(r"[A-Za-z0-9_-]{1,80}", info[k]) for k in ("target", "profile")) or (info["commit"] is not None and (not isinstance(info["commit"], str) or not re.fullmatch(r"[a-fA-F0-9]{40}", info["commit"]))):
        raise ValueError("Invalid build identity")


def read_manifest(bundle):
    path = Path(bundle) / "manifest.json"
    if path.stat().st_size > 1024 * 1024:
        raise ValueError("Oversized symbol manifest")
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(manifest, dict) or type(manifest.get("schema")) is not int or manifest["schema"] != 1:
        raise ValueError("Unsupported symbol bundle schema")
    validate_build(manifest.get("build"))
    module = manifest.get("module")
    files = manifest.get("files")
    if not isinstance(module, dict) or set(module) != {"os", "cpu", "debug_id", "debug_file"} or not all(isinstance(v, str) for v in module.values()) or not isinstance(files, dict) or not 1 <= len(files) <= 4096 or not all(isinstance(k, str) and isinstance(v, str) and re.fullmatch(r"[a-f0-9]{64}", v) for k, v in files.items()):
        raise ValueError("Malformed symbol manifest")
    return manifest


def analyze(dump, bundle, tools):
    """Resolve only with explicitly supplied local symbols; no automatic upload."""
    dump, bundle = Path(dump).resolve(), Path(bundle).resolve()
    if not dump.is_file() or dump.stat().st_size > 10_000_000:
        raise ValueError("Missing or oversized minidump")
    manifest = read_manifest(bundle)
    module = manifest["module"]
    # Revalidate even an edited manifest before using it for path construction.
    module_header(f"MODULE {module['os']} {module['cpu']} {module['debug_id']} {module['debug_file']}")
    expected_symbol = (Path("symbols") / module["debug_file"] / module["debug_id"] / (module["debug_file"].removesuffix(".pdb") + ".sym")).as_posix()
    if manifest.get("symbol_file") != expected_symbol or expected_symbol not in manifest.get("files", {}):
        raise ValueError("Required symbol file is not covered by the manifest")
    for relative, expected in manifest["files"].items():
        path = (bundle / relative).resolve()
        if not path.is_relative_to(bundle) or not path.is_file() or digest(path) != expected:
            raise ValueError("Symbol bundle is missing, modified or contains an unsafe path")
    with (bundle / expected_symbol).open(encoding="utf-8") as source:
        if module_header(source.readline(8192)) != module:
            raise ValueError("Symbol header does not match bundle identity")
    with tempfile.TemporaryDirectory() as staging:
        raw = Path(staging) / "walk.json"
        # No --symbols-url, symbol cache, local debuginfo feature or system symbol
        # lookup. This pinned release uses simple_symbol_supplier for local paths.
        run_tool([tool(tools, "minidump-stackwalk"), "--json", dump, bundle / "symbols"], raw, budget=32 * 1024 * 1024)
        walk = json.loads(raw.read_text(encoding="utf-8"))
    modules = walk.get("modules")
    if not isinstance(modules, list):
        raise ValueError("Dump has no module list")
    matches = [m for m in modules if isinstance(m, dict) and basename(m.get("debug_file")) == module["debug_file"] and m.get("debug_id") == module["debug_id"]]
    if len(matches) != 1 or not matches[0].get("loaded_symbols"):
        raise ValueError("Minidump module ID does not match usable bundle symbols")
    crashed = walk.get("crashing_thread")
    if not isinstance(crashed, dict):
        raise ValueError("Dump has no identified crash thread")
    raw_frames = crashed.get("frames")
    if not isinstance(raw_frames, list) or not raw_frames or not all(isinstance(f, dict) for f in raw_frames):
        raise ValueError("Dump has no usable crash-thread frames")
    frames = []
    for frame in raw_frames[:256]:
        function = frame.get("function")
        function = function[:512] if isinstance(function, str) and function else "<unresolved>"
        if "/" in function or "\\" in function:
            function = "<redacted>"
        line = frame.get("line")
        trust = frame.get("trust")
        frames.append({"function": function, "file": basename(frame.get("file")), "line": line if type(line) is int and 0 < line <= 0xFFFFFFFF else None, "trust": trust if trust in ("context", "cfi", "frame_pointer", "scan") else None, "module": basename(frame.get("module"))})
    return {"schema": 1, "module": module, "build": manifest["build"], "frames": frames, "unresolved": sum(f["function"] == "<unresolved>" for f in frames)}


def panic_source(stack, expected):
    """Require the source location attached to the probe, not another frame."""
    lines = stack.splitlines()
    for index, line in enumerate(lines):
        if expected in line:
            for following in lines[index + 1:]:
                if re.match(r"\s*\d+:", following):
                    break
                match = re.fullmatch(r"\s*at .*[/\\]mod\.rs:(\d+)(?::\d+)?\s*", following)
                if match and int(match[1]) > 0:
                    return {"function": expected, "file": "mod.rs", "line": int(match[1])}
    raise ValueError("Release Rust probe lacks its own source location")


def verify_release(binary, bundle, tools, output):
    """Controlled main/worker Rust panic and native fault in disposable stores."""
    binary = Path(binary).resolve()
    build = json.loads(subprocess.check_output([str(binary), "--crash-build-info"], text=True, timeout=10))
    if build["profile"] != "release":
        raise ValueError("Release verification requires an actual release executable")
    manifest = read_manifest(bundle)
    if build != manifest["build"] or digest(binary) != manifest["binary_sha256"]:
        raise ValueError("Packaged executable does not match the symbol bundle")
    evidence = []
    for mode, expected in [("panic", "panic_probe"), ("thread", "panic_probe"), ("native", "native_probe")]:
        with tempfile.TemporaryDirectory() as staging:
            root = Path(staging) / "reports"
            result = subprocess.run([str(binary), "--crash-probe", str(root), mode], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=25)
            if (result.returncode == 0) != (mode == "thread"):
                raise ValueError("Unexpected release probe exit")
            dumps = list(root.glob("*.dmp"))
            if len(dumps) != 1:
                raise ValueError("Release probe did not retain exactly one minidump")
            report = json.loads(dumps[0].with_suffix(".json").read_text(encoding="utf-8"))
            if mode != "native":
                # macOS panic snapshots intentionally have no exception stream;
                # validate their original in-process Rust stack, not a random thread.
                anchor = panic_source(report["stack"], expected)
                record = {"mode": mode, "source": "panic_hook", "resolved": True, "anchor": anchor, "build": report.get("build")}
            else:
                record = analyze(dumps[0], bundle, tools)
                if not any(expected in f["function"] and f["module"] == binary.name and f["trust"] == "context" and f["file"] == "mod.rs" and isinstance(f["line"], int) and f["line"] > 0 for f in record["frames"]):
                    raise ValueError("Native crash-thread stack lacks the expected function/source location")
                record["mode"] = mode
                anchor = next(f for f in record["frames"] if expected in f["function"] and f["trust"] == "context")
            print(json.dumps({"mode": mode, "anchor": anchor}))
            evidence.append(record)
    Path(output).write_text(json.dumps({"schema": 1, "cases": evidence}, indent=2) + "\n", encoding="utf-8")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    fetch = sub.add_parser("fetch-tools")
    fetch.add_argument("--directory", required=True)
    fetch.add_argument("--target", required=True, choices=next(iter(TOOLS.values()))[2])
    for name in ("bundle", "verify-release", "analyze"):
        command = sub.add_parser(name)
        command.add_argument("--tools", required=True)
        command.add_argument("--bundle", required=True)
        if name != "analyze":
            command.add_argument("--binary", required=True)
        if name == "verify-release":
            command.add_argument("--output", required=True)
        if name == "analyze":
            command.add_argument("--dump", required=True)
    args = parser.parse_args()
    if args.command == "fetch-tools":
        fetch_tools(args.directory, args.target)
    elif args.command == "bundle":
        create_bundle(args.binary, args.bundle, args.tools)
    elif args.command == "verify-release":
        verify_release(args.binary, args.bundle, args.tools, args.output)
    else:
        print(json.dumps(analyze(args.dump, args.bundle, args.tools), indent=2))


if __name__ == "__main__":
    try:
        main()
    except (ValueError, OSError, subprocess.SubprocessError, KeyError, json.JSONDecodeError):
        raise SystemExit("Crash symbol operation failed; check local inputs/tool versions (raw paths/payloads omitted)") from None
