"""Smoke-test an installer on a clean native machine, including the packaged decoder."""

import argparse
import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import struct
import subprocess
import tempfile

from desktop import APP, ROOT


def verify_exe(exe):
    info = json.loads(subprocess.check_output([exe, "--crash-build-info"], timeout=20))
    assert info["profile"] == "release"
    if os.environ.get("GITHUB_SHA"):
        assert info["commit"] == os.environ["GITHUB_SHA"]
    report = subprocess.check_output([exe, "--analyze", ROOT / "tests/fixtures/tone-9s.mp3"],
                                     timeout=60, text=True)
    assert "frames=" in report and "rate=" in report, report


def windows(msi, work, upgrade):
    import winreg

    folder = Path(os.environ["LOCALAPPDATA"]) / APP
    exe = folder / f"{APP}.exe"
    menu = subprocess.check_output(["powershell", "-NoProfile", "-Command",
                                    "[Environment]::GetFolderPath('Programs')"], text=True).strip()
    shortcut = Path(menu) / f"{APP}.lnk"
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, r"Software\Dr. DJ Pro\Installer"):
            raise RuntimeError("An existing installation is registered")
    except FileNotFoundError:
        pass
    if folder.exists() or shortcut.exists():
        raise RuntimeError("Test requires a clean profile without an existing Dr. DJ Pro installation")
    subprocess.run(["msiexec", "/i", msi, "/qn", "/norestart", "/l*v", work / "install.log"], check=True)
    installed = msi
    try:
        if upgrade:
            subprocess.run(["msiexec", "/i", upgrade, "/qn", "/norestart", "/l*v", work / "upgrade.log"], check=True)
            installed = upgrade
        assert shortcut.is_file() and (folder / "Licenses.txt").is_file()
        assert (folder / "vcruntime140.dll").is_file()
        target = subprocess.check_output(["powershell", "-NoProfile", "-Command",
                    "(New-Object -ComObject WScript.Shell).CreateShortcut($env:DRDJ_TEST_SHORTCUT).TargetPath"],
                    env={**os.environ, "DRDJ_TEST_SHORTCUT": str(shortcut)}, text=True).strip()
        assert Path(target).samefile(exe)
        with exe.open("rb") as f:
            f.seek(0x3c)
            pe, = struct.unpack("<I", f.read(4))
            f.seek(pe + 24 + 68)
            assert struct.unpack("<H", f.read(2))[0] == 2, "Expected Windows GUI subsystem"
        verify_exe(exe)
    finally:
        subprocess.run(["msiexec", "/x", installed, "/qn", "/norestart", "/l*v", work / "uninstall.log"], check=True)
    assert not exe.exists() and not shortcut.exists()


def mac(dmg, work):
    mount = work / "mounted"
    mount.mkdir()
    subprocess.run(["hdiutil", "attach", "-readonly", "-nobrowse", "-mountpoint", mount, dmg], check=True)
    try:
        # Verify the same operation as dragging the application into Applications.
        app = work / f"{APP}.app"
        shutil.copytree(mount / app.name, app, symlinks=True)
        assert (mount / "Applications").readlink() == Path("/Applications")
        info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
        assert info["CFBundleDisplayName"] == APP
        assert (app / "Contents/Resources/AppIcon.icns").is_file()
        subprocess.run(["codesign", "--verify", "--strict", app], check=True)
        exe = app / "Contents/MacOS" / info["CFBundleExecutable"]
        header = subprocess.check_output(["vtool", "-show-build", exe], text=True)
        minimum = re.search(r"\bminos\s+(\d+)\.(\d+)", header)
        assert minimum and tuple(map(int, minimum.groups())) <= (14, 0), header
        verify_exe(exe)
    finally:
        subprocess.run(["hdiutil", "detach", mount], check=True)


def linux(deb):
    # Only run on an ephemeral CI image, where this package has never been installed.
    if not os.environ.get("GITHUB_ACTIONS"):
        raise RuntimeError("Run the Debian installation test on the hosted test runner")
    subprocess.run(["sudo", "apt-get", "install", "-y", str(deb)], check=True)
    try:
        subprocess.run(["desktop-file-validate", "/usr/share/applications/dr-dj-pro.desktop"], check=True)
        assert Path("/usr/share/icons/hicolor/512x512/apps/dr-dj-pro.png").is_file()
        verify_exe(Path("/usr/bin/dr-dj-pro"))
    finally:
        subprocess.run(["sudo", "dpkg", "--remove", "dr-dj-pro"], check=True)
    assert not Path("/usr/bin/dr-dj-pro").exists()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("package", type=Path)
    parser.add_argument("--upgrade", type=Path, help="Second MSI with the same version")
    args = parser.parse_args()
    package = args.package.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="drdj-test-") as directory:
        if package.suffix == ".msi":
            windows(package, Path(directory), args.upgrade.resolve(strict=True) if args.upgrade else None)
        elif package.suffix == ".dmg":
            mac(package, Path(directory))
        elif package.suffix == ".deb":
            linux(package)
        else:
            raise ValueError("Unknown package format")
