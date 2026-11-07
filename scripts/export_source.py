"""Export an allowlisted publication tree, not private development Git history."""
import argparse
import os
import re
import tempfile
from pathlib import Path
from make_zip import make_zip

DIRECTORIES = ["src", "tests", "examples", "resources", "packaging", "scripts", "docs", ".github", ".cargo"]
FILES = [".gitattributes", "Cargo.toml", "Cargo.lock", "build.rs", ".gitignore", "README.md", "RELEASE_NOTES.md", "SECURITY.md", "MANUAL_PLAYTEST.md", "THIRD_PARTY_NOTICES.md", "LICENSE", "LICENSE-PENDING.txt", "Ferrum.png"]


def collect(root):
    paths = [root / name for name in FILES if (root / name).is_file()]
    for name in DIRECTORIES:
        paths += [path for path in (root / name).rglob("*") if path.is_file()]
    result = []
    for path in sorted(set(paths)):
        relative = path.relative_to(root)
        if path.name == "LICENSE-PENDING.txt" and (root / "LICENSE").is_file():
            continue
        if "__pycache__" in relative.parts or path.suffix.lower() in (".exe", ".dll", ".pdb", ".zip", ".log", ".pfx", ".key", ".p12", ".pyc"):
            continue
        if path.is_symlink() or root not in path.resolve().parents:
            raise ValueError("Source export refuses symlinks/outside files")
        if path.name in ("config.json", "ferrum-index-v1.json", "local-discovery.json", "action-audit.json") or path.name.startswith(".env"):
            raise ValueError("Private source-export file")
        data = path.read_bytes()
        home = os.environ.get("USERPROFILE", "")
        if home and any(value.encode().lower() in data.lower() for value in (home, home.replace("\\", "/"))):
            raise ValueError("Developer path found in publication source: " + relative.as_posix())
        # Match the tracked .gitattributes policy in source ZIPs and clean checkouts.
        # Raster/derived branding bytes are never rewritten.
        if path.suffix.lower() not in (".png", ".ico", ".rgba"):
            data.decode("utf-8-sig")
            data = data.replace(b"\r\n", b"\n")
        result.append((relative, data))
    return result


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    with tempfile.TemporaryDirectory(prefix="ferrum-source-export-") as temporary:
        destination = Path(temporary)
        for relative, data in collect(root):
            file = destination / relative
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_bytes(data)
        make_zip(destination, args.output.resolve())
    print("Created clean publication snapshot; no repository, Git history or public remote was created")
