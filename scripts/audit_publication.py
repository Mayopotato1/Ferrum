"""Audit publication bytes without printing matched private data. No network or writes."""
import argparse
import hashlib
import json
import os
import re
import subprocess
from pathlib import Path
from export_source import collect

TOKEN_PATTERNS = [
    rb"gh[pousr]_[A-Za-z0-9]{30,}", rb"github_pat_[A-Za-z0-9_]{30,}",
    rb"AKIA[0-9A-Z]{16}", rb"AIza[A-Za-z0-9_-]{30,}",
    rb"sk-(?:proj-)?[A-Za-z0-9_-]{30,}", rb"-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----",
]
FORBIDDEN_PARTS = {".git", "target", "dist", "reports", "publication-source", "__pycache__"}
FORBIDDEN_SUFFIXES = {".exe", ".dll", ".pdb", ".zip", ".log", ".pfx", ".p12", ".key", ".pem", ".sqlite", ".db"}
FORBIDDEN_NAMES = {"config.json", "credentials.json", "local-discovery.json", "action-audit.json"}

def audit(entries, private_values=()):
    findings, inventory = [], []
    for relative, data in entries:
        path = Path(relative); name = path.as_posix()
        categories = []
        if set(path.parts) & FORBIDDEN_PARTS or path.suffix.lower() in FORBIDDEN_SUFFIXES or path.name in FORBIDDEN_NAMES or path.name.startswith((".env", "ferrum-index-", "ferrum-diagnostics-")):
            categories.append("private/generated file")
        folded = data.lower()
        if any(v and v.encode().lower() in folded for v in private_values):
            categories.append("local identity/path")
        if any(re.search(pattern, data) for pattern in TOKEN_PATTERNS):
            categories.append("possible credential")
        findings.extend({"file": name, "category": c} for c in categories)
        inventory.append({"file": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    return {"files": len(inventory), "findings": findings, "inventory": inventory}

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("root", type=Path)
    parser.add_argument("--tracked", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(); root = args.root.resolve()
    if args.tracked:
        names = subprocess.check_output(["git", "-C", str(root), "ls-files", "-z"]).decode().split("\0")
        entries = [(Path(n), (root / n).read_bytes()) for n in names if n]
    else:
        entries = collect(root)
    home = os.environ.get("USERPROFILE", "")
    email = subprocess.run(["git", "config", "user.email"], cwd=root, capture_output=True, text=True).stdout.strip()
    private = [home, home.replace("\\", "/")]
    if email and not email.endswith("@users.noreply.github.com"):
        private.append(email)
    result = audit(entries, private)
    if args.output: args.output.write_text(json.dumps(result, indent=2), encoding="utf-8")
    print(json.dumps({"files": result["files"], "findings": result["findings"]}, indent=2))
    raise SystemExit(bool(result["findings"]))
