"""Collect locked Windows-reachable dependency license texts; no font/source assets."""
import argparse
import hashlib
import json
import re
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def generate(destination):
    destination.mkdir(parents=True, exist_ok=True)
    metadata = json.loads(subprocess.check_output([
        "cargo", "metadata", "--locked", "--filter-platform", "x86_64-pc-windows-msvc", "--format-version", "1"
    ], cwd=str(ROOT)))
    root_id = metadata["resolve"]["root"]
    # Metadata can retain inactive optional graph edges. Cargo tree is the
    # authoritative activated Windows normal/build dependency selection.
    tree = subprocess.check_output(["cargo", "tree", "--locked", "--target", "x86_64-pc-windows-msvc", "--edges", "normal,build", "--prefix", "none", "--format", "{p}"], cwd=str(ROOT), text=True)
    used = set(re.findall(r"^([A-Za-z0-9_-]+) v([^\s]+)", tree, re.M))
    overrides = json.loads((ROOT / "packaging/license-overrides.json").read_text())
    inventory, missing = [], []
    for package in sorted(metadata["packages"], key=lambda p: (p["name"], p["version"])):
        if (package["name"], package["version"]) not in used or package["id"] == root_id:
            continue
        folder = Path(package["manifest_path"]).parent
        candidates = set()
        for file in folder.iterdir():
            if file.is_file() and re.match(r"(?i)^(licen[cs]e|copying|copyright|notice|unlicense)([._-]|$)", file.name):
                candidates.add(file)
            if file.is_dir() and file.name.lower() in ("licenses", "licences"):
                candidates.update(p for p in file.rglob("*") if p.is_file() and p.suffix.lower() in (".txt", ".md", ".html", ""))
        if package.get("license_file"):
            candidates.add(folder / package["license_file"])
        bundle = destination / (package["name"] + "-" + package["version"])
        texts = []
        for file in sorted(candidates):
            if not file.is_file() or file.stat().st_size > 256_000:
                continue
            try:
                data = file.read_bytes()
                data.decode("utf-8-sig")
            except UnicodeError:
                continue
            try:
                relative = file.resolve().relative_to(folder.resolve())
            except ValueError:
                continue
            output = bundle / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)
            texts.append(output.relative_to(destination).as_posix())
        if not texts:
            for entry in overrides.get(package["name"] + "-" + package["version"], []):
                file = ROOT / entry["path"]
                data = file.read_bytes()
                if hashlib.sha256(data).hexdigest() != entry["sha256"]:
                    raise RuntimeError("Pinned license supplement changed: " + entry["path"])
                output = bundle / ("upstream-" + file.name)
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(data)
                texts.append(output.relative_to(destination).as_posix())
        if not texts:
            missing.append(package["name"] + " " + package["version"])
        inventory.append({"name": package["name"], "version": package["version"],
                          "declared_license": package.get("license"), "notice_files": texts,
                          "source": "https://crates.io/api/v1/crates/" + package["name"] + "/" + package["version"] + "/download"})
    # The selected Slint license comes from the pinned toolkit license already reviewed.
    for file in (ROOT / "packaging/licenses").iterdir():
        shutil.copyfile(file, destination / file.name)
    # Plaintext contains no package-manager filesystem paths.
    lines = ["Ferrum third-party notices", "", "Locked Windows-target dependency inventory (includes build-time components).",
             "License expressions are upstream declarations, not a claim that all components use MIT.",
             "Slint is used under its Royalty-free Desktop, Mobile, and Web Applications License 2.0;",
             "its AboutSlint attribution widget is included in Settings / About.",
             "Nucleo-matcher is unmodified MPL-2.0; its exact Source Code Form is available at the crate URL below.",
             "SQLite's bundled engine is public domain; the Rust wrapper retains its own listed license.",
             "Inno Setup's installer engine is Copyright Jordan Russell and Martijn Laan; see Inno-Setup-License.txt.", ""]
    for package in inventory:
        lines += [package["name"] + " " + package["version"], "Declared license: " + str(package["declared_license"]),
                  "Source: " + package["source"], "Notices: " + ", ".join(package["notice_files"]), ""]
    (destination.parent / "THIRD_PARTY_NOTICES.txt").write_text("\n".join(lines), encoding="utf-8")
    (destination / "dependency-licenses.json").write_text(json.dumps({"packages": inventory, "missing_notice_text": missing}, indent=2), encoding="utf-8")
    return missing


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    missing = generate(args.destination.resolve())
    print("Missing notice files:", ", ".join(missing) if missing else "none")
    if missing:
        raise SystemExit(2)
