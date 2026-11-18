#!/usr/bin/env python3
"""Exercise package checks with fixture payloads and fake Apple tools."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("verify-artifacts.sh").resolve()


class ArtifactVerificationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="sidecar-artifacts-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.release = self.root / "release with spaces"
        self.release.mkdir()

    def stage(self, relative, signature="signature-token\n"):
        payload = self.release / "bundle" / relative
        payload.parent.mkdir(parents=True, exist_ok=True)
        payload.write_bytes(b"payload")
        if signature is not None:
            payload.with_name(payload.name + ".sig").write_text(signature)
        return payload

    def run_check(self, mode="updater-signatures", platform="macos-latest", env=None):
        return subprocess.run(
            ["bash", SCRIPT.as_posix(), mode, "--platform", platform,
             "--release-root", self.release.as_posix()],
            capture_output=True, text=True, env=env,
        )

    def test_accepts_both_platforms_and_filenames_with_spaces(self):
        self.stage("macos/SpeleoDB Compass Sidecar.app.tar.gz")
        self.stage("msi/SpeleoDB Compass Sidecar.msi", "msi-token")
        self.stage("nsis/SpeleoDB Compass Sidecar-setup.exe", "nsis-token")
        for platform in ["macos-latest", "windows-latest"]:
            result = self.run_check(platform=platform)
            self.assertEqual(result.returncode, 0, result.stderr)

    def test_missing_ambiguous_and_unsigned_payloads_fail(self):
        self.assertNotEqual(self.run_check().returncode, 0)
        archive = self.stage("macos/App.app.tar.gz", signature=None)
        self.assertIn("missing updater signature", self.run_check().stderr)
        archive.with_name(archive.name + ".sig").write_text("token")
        self.stage("macos/Another.app.tar.gz")
        self.assertIn("expected exactly one", self.run_check().stderr)

    def test_empty_and_multiline_signatures_fail(self):
        archive = self.stage("macos/App.app.tar.gz")
        for signature in ["", "  \n", "one\ntwo\n", "one\rtwo"]:
            archive.with_name(archive.name + ".sig").write_text(signature)
            with self.subTest(signature=signature):
                self.assertNotEqual(self.run_check().returncode, 0)

    def test_legacy_windows_archives_are_rejected(self):
        self.stage("msi/App.msi", "msi-token")
        self.stage("nsis/App.exe", "nsis-token")
        for name in ["msi/App.msi.zip", "nsis/App.nsis.zip", "msi/App.msi.zip.sig"]:
            legacy = self.stage(name, signature=None)
            with self.subTest(name=name):
                self.assertNotEqual(self.run_check(platform="windows-latest").returncode, 0)
            legacy.unlink()

    def test_unsupported_mode_or_platform_fails(self):
        self.assertNotEqual(self.run_check(mode="invalid").returncode, 0)
        self.assertNotEqual(self.run_check(platform="linux-latest").returncode, 0)

    def fake_tools(self, failure=""):
        tools = self.root / "tools"
        tools.mkdir(exist_ok=True)
        log = self.root / "calls.jsonl"
        common = tools / "fake.py"
        common.write_text('''import json, os, sys
from pathlib import Path
command = Path(sys.argv[1]).name
args = sys.argv[2:]
with open(os.environ["VERIFY_LOG"], "a") as output:
    output.write(json.dumps([command, *args]) + "\\n")
if command == "hdiutil" and args[0] == "attach":
    mount = Path(args[args.index("-mountpoint") + 1])
    binary = mount / "SpeleoDB Compass Sidecar.app/Contents/MacOS/speleodb-compass-sidecar"
    binary.parent.mkdir(parents=True)
    binary.touch()
if command == os.environ.get("VERIFY_FAILURE"):
    sys.exit(17)
''')
        for name in ["hdiutil", "codesign", "spctl", "xcrun"]:
            tool = tools / name
            tool.write_text('#!/usr/bin/env bash\nexec "$VERIFY_PYTHON" "$VERIFY_FAKE" "$0" "$@"\n')
            tool.chmod(0o755)
        return dict(os.environ, PATH=str(tools) + os.pathsep + os.environ["PATH"],
                    RUNNER_TEMP=str(self.root), VERIFY_LOG=str(log),
                    VERIFY_FAKE=str(common), VERIFY_PYTHON=sys.executable,
                    VERIFY_FAILURE=failure), log

    @unittest.skipIf(os.name == "nt", "Apple-tool fixture uses POSIX executable scripts")
    def test_checks_shipped_app_and_dmg_then_detaches(self):
        self.stage("dmg/SpeleoDB Compass Sidecar.dmg", signature=None)
        env, log = self.fake_tools()
        result = self.run_check(mode="runtime", env=env)
        self.assertEqual(result.returncode, 0, result.stderr)
        calls = [json.loads(line) for line in log.read_text().splitlines()]
        self.assertEqual([call[0] for call in calls],
                         ["hdiutil", "codesign", "spctl", "xcrun", "spctl", "xcrun", "hdiutil"])
        self.assertEqual(calls[1][1:5], ["--verify", "--deep", "--strict", "--verbose=2"])
        self.assertTrue(calls[1][-1].endswith("SpeleoDB Compass Sidecar.app"))
        self.assertEqual(calls[2][1:4], ["--assess", "--type", "execute"])
        self.assertEqual(calls[3][1:3], ["stapler", "validate"])
        self.assertEqual(calls[4][1:6], ["--assess", "--type", "open", "--context", "context:primary-signature"])
        self.assertEqual(calls[5][1:3], ["stapler", "validate"])
        self.assertEqual(calls[-1][1], "detach")
        self.assertFalse(Path(calls[-1][2]).exists())

    @unittest.skipIf(os.name == "nt", "Apple-tool fixture uses POSIX executable scripts")
    def test_signature_failure_remains_fatal_and_always_detaches(self):
        self.stage("dmg/App.dmg", signature=None)
        env, log = self.fake_tools(failure="codesign")
        result = self.run_check(mode="runtime", env=env)
        self.assertEqual(result.returncode, 17, result.stderr)
        calls = [json.loads(line) for line in log.read_text().splitlines()]
        self.assertEqual(calls[-1][0:2], ["hdiutil", "detach"])
        self.assertFalse(Path(calls[-1][2]).exists())


if __name__ == "__main__":
    unittest.main()
