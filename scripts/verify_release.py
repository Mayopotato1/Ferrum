"""Verify actual final files/ZIP payload and hashes. Never run Setup or Ferrum GUI."""
import argparse
import hashlib
import json
import re
import zipfile
from pathlib import Path
from inspect_binary import inspect


def verify(directory, payload):
    manifest = directory / "SHA256SUMS.txt"
    checked = []
    for line in manifest.read_text(encoding="utf-8-sig").splitlines():
        expected, name = line.split("  ", 1)
        if Path(name).name != name:
            raise ValueError("Unsafe checksum filename")
        file = directory / name
        if hashlib.sha256(file.read_bytes()).hexdigest() != expected:
            raise ValueError("Final artifact checksum mismatch: " + name)
        checked.append(name)
    zips = list(directory.glob("*-windows-x64-portable.zip"))
    if len(zips) != 1:
        raise ValueError("Expected exactly one portable ZIP")
    with zipfile.ZipFile(zips[0]) as archive:
        names = archive.namelist()
        expected = sorted(p.relative_to(payload).as_posix() for p in payload.rglob("*") if p.is_file())
        if sorted(names) != expected or names.count("ferrum.exe") != 1:
            raise ValueError("ZIP payload allowlist mismatch")
        for name in names:
            path = Path(name)
            if path.is_absolute() or ".." in path.parts or "\\" in name:
                raise ValueError("Unsafe ZIP path")
            if name not in ("ferrum.exe", "README.txt", "LICENSE.txt", "INSTALLATION.txt", "THIRD_PARTY_NOTICES.txt") and not name.startswith("licenses/"):
                raise ValueError("Unexpected portable file: " + name)
            if archive.read(name) != (payload / name).read_bytes():
                raise ValueError("ZIP bytes differ from verified staging")
    pe = inspect(payload / "ferrum.exe")
    if pe["machine"] != 0x8664 or pe["subsystem"] != 2 or pe["private_build_path_hits"] or pe["vc_runtime_imports"] or pe["icon_sizes"] != [16,24,32,48,64,128,256]:
        raise ValueError("Packaged PE policy failed")
    source = list(directory.glob("*-source.zip"))
    if len(source) != 1:
        raise ValueError("Expected one clean source ZIP")
    with zipfile.ZipFile(source[0]) as archive:
        for name in archive.namelist():
            if name.startswith((".git/", "target/", "reports/", "dist/")) or name.endswith((".exe", ".dll", ".pdb", ".pfx", ".key")):
                raise ValueError("Private/development output in source ZIP")
        for name in ("Cargo.toml", "Cargo.lock", "src/main.rs", ".github/workflows/release.yml", "packaging/windows/ferrum.iss", "resources/ferrum.ico"):
            if name not in archive.namelist():
                raise ValueError("Missing publication source: " + name)
    result = {"checked_sha256": checked, "portable_files": len(names), "binary": pe,
              "installer_compiled": len(list(directory.glob("*-windows-x64-setup.exe"))) == 1,
              "installer_executed": False, "gui_tested": False,
              "note": "Installer archive extraction is recorded separately; actual install/uninstall remains manual."}
    (directory / "ARTIFACT_VERIFICATION.json").write_text(json.dumps(result, indent=2))
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    parser.add_argument("--payload", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps(verify(args.directory.resolve(), args.payload.resolve()), indent=2))
