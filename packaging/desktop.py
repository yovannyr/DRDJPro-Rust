"""Build the native installer from this checkout's default-feature release executable."""

import argparse
import json
import os
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
APP = "Dr. DJ Pro"
ICON = ROOT / "res/branding/dr-dj-pro.png"
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text("utf-8"))["package"]["version"]


def output(*args, cwd=ROOT):
    return subprocess.check_output(args, cwd=cwd, encoding="utf-8").strip()


def notices(destination, target):
    metadata = json.loads(output("cargo", "metadata", "--locked", "--format-version", "1",
                                 "--filter-platform", target))
    packages = {p["id"]: p for p in metadata["packages"]}
    nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
    pending = [metadata["resolve"]["root"]]
    included = set()
    while pending:
        key = pending.pop()
        if key in included:
            continue
        included.add(key)
        pending.extend(d["pkg"] for d in nodes[key]["deps"]
                       if any(k["kind"] is None for k in d["dep_kinds"]))
    with destination.open("w", encoding="utf-8") as stream:
        def append(path):
            stream.write(path.read_text(encoding="utf-8", errors="replace") + "\n")

        for name in ["LICENSE", "THIRD_PARTY_NOTICES.md", "ui/layout-fonts/LICENSE.txt",
                     "crates/drdj-keylock/vendor/WRAPPER-LICENSE.md",
                     "crates/drdj-keylock/vendor/signalsmith-stretch/LICENSE.txt",
                     "crates/drdj-keylock/vendor/signalsmith-linear/LICENSE.txt"]:
            append(ROOT / name)
        for key in sorted(included):
            pkg = packages[key]
            base = Path(pkg["manifest_path"]).parent
            stream.write(f"\n{pkg['name']} {pkg['version']} — {pkg['license'] or ''}\n")
            files = {p for p in base.iterdir() if p.is_file() and
                     p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))}
            for directory in base.iterdir():
                if directory.is_dir() and directory.name.upper() in {"LICENSE", "LICENSES", "LICENCES"}:
                    files.update(p for p in directory.rglob("*") if p.is_file())
            if pkg["license_file"]:
                files.add(base / pkg["license_file"])
            for path in sorted(files):
                append(path)


def windows(binary, work, dest):
    vswhere = Path(os.environ["ProgramFiles(x86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
    vs = Path(output(vswhere, "-utf8", "-latest", "-products", "*", "-requires",
                     "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"))
    version = (vs / "VC/Auxiliary/Build/Microsoft.VCRedistVersion.default.txt").read_text().strip()
    runtime, = (vs / "VC/Redist/MSVC" / version / "x64").glob("Microsoft.VC*.CRT")
    libs = sorted(p.name for p in runtime.glob("*.dll"))
    if not {"msvcp140.dll", "vcruntime140.dll"}.issubset(libs):
        raise ValueError("Missing redistributable x64 C++ runtime")
    subprocess.run(["wix", "build", ROOT / "packaging/windows.wxs", "-arch", "x64",
                    "-d", f"Source={ROOT}", "-d", f"Version={VERSION}", "-d", f"Binary={binary}",
                    "-d", f"Notices={work / 'Licenses.txt'}", "-d", f"RuntimeDir={runtime}",
                    "-d", "RuntimeFiles=" + ";".join(libs), "-pdbtype", "none",
                    "-intermediatefolder", work / "obj", "-o", dest / f"Dr-DJ-Pro-{VERSION}-Windows-x64.msi"],
                   check=True)


def mac(binary, work, dest, arch):
    disk = work / "volume"
    app = disk / f"{APP}.app"
    contents = app / "Contents"
    resources = contents / "Resources"
    executable = contents / "MacOS" / APP
    executable.parent.mkdir(parents=True)
    resources.mkdir()
    shutil.copy2(binary, executable)
    shutil.copy2(work / "Licenses.txt", resources)
    iconset = work / "AppIcon.iconset"
    iconset.mkdir()
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            name = f"icon_{size}x{size}" + ("@2x" if scale == 2 else "") + ".png"
            subprocess.run(["sips", "-z", str(size * scale), str(size * scale), ICON,
                            "--out", iconset / name], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["iconutil", "-c", "icns", iconset, "-o", resources / "AppIcon.icns"], check=True)
    settings = {"CFBundleIdentifier": "com.drdjpro.desktop", "CFBundleName": APP,
                "CFBundleDisplayName": APP, "CFBundleExecutable": APP, "CFBundleIconFile": "AppIcon",
                "CFBundlePackageType": "APPL", "CFBundleVersion": VERSION,
                "CFBundleShortVersionString": VERSION, "NSHighResolutionCapable": True,
                "LSMinimumSystemVersion": "14.0", "LSApplicationCategoryType": "public.app-category.music"}
    (contents / "Info.plist").write_bytes(plistlib.dumps(settings))
    subprocess.run(["codesign", "--force", "--sign", "-", app], check=True)
    (disk / "Applications").symlink_to("/Applications", target_is_directory=True)
    subprocess.run(["hdiutil", "create", "-volname", APP, "-srcfolder", disk, "-format", "UDZO",
                    dest / f"Dr-DJ-Pro-{VERSION}-macOS-{arch}.dmg"], check=True)


def linux(binary, work, dest):
    pkg = work / "package"
    exe = pkg / "usr/bin/dr-dj-pro"
    desktop = pkg / "usr/share/applications/dr-dj-pro.desktop"
    icon = pkg / "usr/share/icons/hicolor/512x512/apps/dr-dj-pro.png"
    copyright_file = pkg / "usr/share/doc/dr-dj-pro/copyright"
    control = pkg / "DEBIAN/control"
    for path in (exe, desktop, icon, copyright_file, control):
        path.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(binary, exe)
    shutil.copy2(ICON, icon)
    shutil.copy2(work / "Licenses.txt", copyright_file)
    desktop.write_text("[Desktop Entry]\nType=Application\nName=Dr. DJ Pro\nExec=dr-dj-pro\n"
                       "Icon=dr-dj-pro\nTerminal=false\nCategories=AudioVideo;Audio;Mixer;\n")
    # Resolve symbol-level minimum system-library versions from the actual ELF.
    (work / "debian").mkdir()
    (work / "debian/control").write_text("Source: dr-dj-pro\n\nPackage: dr-dj-pro\nArchitecture: amd64\n")
    deps = output("dpkg-shlibdeps", "-O", "-e" + str(exe), cwd=work).removeprefix("shlibs:Depends=")
    control.write_text(f"Package: dr-dj-pro\nVersion: {VERSION}\nArchitecture: amd64\n"
                       f"Maintainer: Dr. DJ Pro <noreply@users.noreply.github.com>\n"
                       f"Depends: {deps}, libxkbcommon0, libgl1\nDescription: Dr. DJ Pro desktop DJ application\n")
    subprocess.run(["dpkg-deb", "--root-owner-group", "--build", pkg,
                    dest / f"Dr-DJ-Pro-{VERSION}-Ubuntu-amd64.deb"], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    info = json.loads(output(binary, "--crash-build-info"))
    system = platform.system()
    expected = {"Windows": "x86_64-pc-windows-msvc", "Linux": "x86_64-unknown-linux-gnu",
                "Darwin": f"{'aarch64' if platform.machine() == 'arm64' else 'x86_64'}-apple-darwin"}[system]
    if info["target"] != expected or info["profile"] != "release":
        raise ValueError(f"Expected a native {expected} release executable")
    dest = args.destination.resolve()
    dest.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="drdj-package-") as directory:
        work = Path(directory)
        notices(work / "Licenses.txt", expected)
        if system == "Windows":
            windows(binary, work, dest)
        elif system == "Darwin":
            mac(binary, work, dest, "Apple-Silicon" if platform.machine() == "arm64" else "Intel")
        else:
            linux(binary, work, dest)


if __name__ == "__main__":
    main()
