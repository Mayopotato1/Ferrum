"""Headless package policy/fixture tests; never start a native window or installer."""
import hashlib
import json
import re
import struct
import sys
import tempfile
import unittest
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
from export_source import collect
from make_zip import make_zip
from audit_publication import audit


class PackagingTests(unittest.TestCase):
    def test_source_export_normalizes_text_but_preserves_artwork(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "README.md").write_bytes(b"line\r\n")
            artwork = b"PNG fixture\r\n\0"
            (root / "Ferrum.png").write_bytes(artwork)
            files = {p.as_posix(): data for p, data in collect(root)}
            self.assertEqual(files["README.md"], b"line\n")
            self.assertEqual(files["Ferrum.png"], artwork)

    def test_pinned_source_and_binary_guards(self):
        pinned = json.loads((ROOT / "packaging/update-source.json").read_text())
        self.assertEqual(pinned, {"configured": True, "owner": "Mayopotato1", "repository": "Ferrum", "channel": "beta"})
        for name in ["Build-Release.ps1", "release.ps1"]:
            self.assertIn("Assert-UpdateSource.ps1", (ROOT / "scripts" / name).read_text())
        for path in (ROOT / ".github/workflows").glob("*.yml"):
            workflow = json.loads(path.read_text())
            self.assertTrue(all("FERRUM_UPDATE_OWNER" not in job.get("env", {}) for job in workflow["jobs"].values()))
        self.assertIn("pinned update source", (ROOT / "build.rs").read_text())

    def test_publication_audit_rejects_sensitive_bytes_without_echoing_them(self):
        credential = b"github_pat_" + b"x" * 40
        report = audit([(Path("src/file.rs"), credential), (Path("reports/private.json"), b"{}"), (Path("README.md"), b"private-fixture-home")], ["private-fixture-home"])
        self.assertEqual(len(report["findings"]), 3)
        self.assertNotIn(credential.decode(), json.dumps(report))
        self.assertEqual(audit([(Path("src/main.rs"), b"fn main() {}")])["findings"], [])

    def test_selected_mit_license_and_remaining_publication_gates(self):
        license_text = (ROOT / "LICENSE").read_text()
        self.assertIn("MIT License", license_text)
        self.assertIn("Permission is hereby granted, free of charge", license_text)
        self.assertIn('license = "MIT"', (ROOT / "Cargo.toml").read_text())
        self.assertFalse((ROOT / "LICENSE-PENDING.txt").exists())
        policy = json.loads((ROOT / "packaging/release-policy.json").read_text())
        self.assertEqual(policy["source_license"], "MIT")
        guard = (ROOT / "scripts/Assert-ReleaseReady.ps1").read_text()
        for field in ["artwork_distribution_confirmed", "publication_approved", "manual_installation_validated", "phase_1_4_gui_validated"]:
            self.assertIn("-not $policy." + field, guard)

    def test_original_artwork_and_derived_icon_sizes(self):
        receipt = json.loads((ROOT / "resources/icon-source.json").read_text())
        self.assertEqual(hashlib.sha256((ROOT / "Ferrum.png").read_bytes()).hexdigest(), receipt["source_sha256"])
        data = (ROOT / "resources/ferrum.ico").read_bytes()
        reserved, kind, count = struct.unpack_from("<HHH", data)
        self.assertEqual((reserved, kind, count), (0, 1, 7))
        sizes = set()
        for index in range(count):
            width, height, _, _, _, _, size, offset = struct.unpack_from("<BBBBHHII", data, 6 + 16 * index)
            self.assertEqual(width, height)
            sizes.add(width or 256)
            self.assertLessEqual(offset + size, len(data))
            self.assertEqual(data[offset:offset + 8], bytes([137,80,78,71,13,10,26,10]))
        self.assertEqual(sizes, {16,24,32,48,64,128,256})
        self.assertEqual((ROOT / "resources/ferrum-icon-32.rgba").stat().st_size, 32*32*4)

    def test_installer_has_per_user_non_destructive_policy(self):
        script = (ROOT / "packaging/windows/ferrum.iss").read_text()
        active = "\n".join(line for line in script.splitlines() if not line.lstrip().startswith((";", "//")))
        for directive in ["PrivilegesRequired=lowest", "DefaultDirName={localappdata}\\Programs\\Ferrum", "CloseApplications=no", "RestartApplications=no", "AlwaysRestart=no", "Uninstallable=yes"]:
            self.assertIn(directive, active)
        self.assertNotIn("[UninstallDelete]", active)
        self.assertNotIn("[Registry]", active)
        self.assertNotIn("restartreplace", active)
        self.assertIn("if CompareText(Value, Expected) = 0 then", active)
        self.assertIn("RegQueryStringValue(HKCU64", active)
        self.assertIn("CurUninstallStep = usUninstall", active)
        self.assertIn("CheckForMutexes(ProductMutex)", active)
        self.assertIn("Flags: postinstall nowait skipifsilent unchecked", active)
        self.assertIn("AppId={{35C83E18-76F3-4F51-A9C6-2E8C071833B5}", active)

    def test_workflows_pin_actions_and_separate_publish_permissions(self):
        pins = json.loads((ROOT / "packaging/actions.lock.json").read_text())
        for path in (ROOT / ".github/workflows").glob("*.yml"):
            workflow = json.loads(path.read_text())
            self.assertEqual(workflow["permissions"], {"contents": "read"})
            for name, job in workflow["jobs"].items():
                for step in job["steps"]:
                    if "uses" in step:
                        action, sha = step["uses"].split("@")
                        self.assertRegex(sha, r"^[a-f0-9]{40}$")
                        self.assertEqual(sha, pins[action]["sha"])
                if job.get("permissions", {}).get("contents") == "write":
                    self.assertEqual(name, "draft-release")
                    self.assertEqual(job["environment"], "public-beta")
                    self.assertIn("github.event_name == 'push'", job["if"])
                    self.assertIn("FERRUM_ENABLE_DRAFT_RELEASES == 'true'", job["if"])
                    command = job["steps"][-1]["run"]
                    self.assertIn("--draft --prerelease", command)
                    self.assertIn("Assert-ReleaseReady.ps1", command)
                    self.assertIn("Refusing to replace an already-public release", command)
        ci = json.loads((ROOT / ".github/workflows/ci.yml").read_text())
        self.assertIn("pull_request", ci["on"])
        self.assertTrue(all(j.get("permissions", {}).get("contents") != "write" for j in ci["jobs"].values()))

    def test_source_export_excludes_private_history_and_binary_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for name in ["Cargo.toml", "src/main.rs", "target/private.txt", ".git/config", "reports/private.txt", "scripts/tool.exe"]:
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text("fixture")
            self.assertEqual({p.as_posix() for p, _ in collect(root)}, {"Cargo.toml", "src/main.rs"})
            (root / "src/config.json").write_text("private")
            with self.assertRaisesRegex(ValueError, "Private"):
                collect(root)

    def test_source_export_rejects_developer_home(self):
        import os
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "README.md").write_text("private-host/profile/secret")
            with patch.dict(os.environ, {"USERPROFILE": "private-host/profile"}):
                with self.assertRaisesRegex(ValueError, "Developer path"):
                    collect(root)

    def test_source_export_drops_pending_license_after_owner_selection(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / "LICENSE").write_text("owner selected fixture")
            (root / "LICENSE-PENDING.txt").write_text("pending fixture")
            self.assertEqual([str(p) for p, _ in collect(root)], ["LICENSE"])

    def test_zip_entries_are_stable_and_relative(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); source = root / "payload"; source.mkdir()
            (source / "ferrum.exe").write_bytes(b"fixture-not-executable")
            (source / "LICENSE.txt").write_text("fixture")
            a, b = root / "a.zip", root / "b.zip"
            make_zip(source, a); make_zip(source, b)
            self.assertEqual(a.read_bytes(), b.read_bytes())
            with zipfile.ZipFile(a) as archive:
                self.assertEqual(archive.namelist(), ["LICENSE.txt", "ferrum.exe"])
                self.assertTrue(all(i.date_time == (2026,1,1,0,0,0) for i in archive.infolist()))

    def test_real_publication_tree_contains_release_inputs_without_user_paths(self):
        paths = {p.as_posix() for p, _ in collect(ROOT)}
        for required in ["Cargo.lock", "Ferrum.png", "MANUAL_PLAYTEST.md", "packaging/windows/ferrum.iss", ".github/workflows/release.yml", "resources/ferrum.ico", "scripts/release.ps1"]:
            self.assertIn(required, paths)
        self.assertFalse(any(p.startswith(("target/", "reports/", ".git/", "dist/")) for p in paths))


if __name__ == "__main__":
    unittest.main()
