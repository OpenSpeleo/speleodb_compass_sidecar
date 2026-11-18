#!/usr/bin/env python3
"""Verify the smoke runner reuses built binaries and serves original payloads."""

from __future__ import annotations

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError, URLError
from urllib.parse import unquote, urlsplit
from urllib.request import urlopen

import run_updater_smoke as smoke


FIXTURE = json.loads((Path(__file__).parent / "fixtures/latest.input.json").read_text())


class UpdaterSmokeTests(unittest.TestCase):
    def setUp(self) -> None:
        temporary = tempfile.TemporaryDirectory(prefix="speleodb-compass-sidecar-smoke-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.endpoint = None

    def release(self, platform: str) -> Path:
        root = self.root / platform / "release with spaces"
        for target in smoke.TARGETS_BY_PLATFORM[platform]:
            entry = FIXTURE["platforms"][target]
            name = unquote(Path(urlsplit(entry["url"]).path).name)
            if name.endswith(".app.tar.gz"):
                name = "SpeleoDB Compass Sidecar.app.tar.gz"
                directory = "macos"
            else:
                directory = {".msi": "msi", ".exe": "nsis"}[Path(name).suffix]
            payload = root / "bundle" / directory / name
            payload.parent.mkdir(parents=True, exist_ok=True)
            payload.write_bytes(f"original payload for {name}".encode())
            payload.with_name(name + ".sig").write_text(entry["signature"] + "\n")
        (root / ("speleodb-compass-sidecar.exe" if platform.startswith("windows") else "speleodb-compass-sidecar")).touch()
        if platform.startswith("macos-"):
            bundled = root / "bundle/macos/SpeleoDB Compass Sidecar.app/Contents/MacOS/speleodb-compass-sidecar"
            bundled.parent.mkdir(parents=True)
            bundled.touch()
        # These stand in for retained compilation outputs; nothing may remove them.
        (root.parent / "debug").mkdir()
        (root.parent / "debug/cached.rlib").write_bytes(b"keep compilation cache")
        return root

    @staticmethod
    def snapshot(root: Path) -> dict:
        return {path: (path.read_bytes(), path.stat().st_mtime_ns, path.stat().st_ino)
                for path in root.rglob("*") if path.is_file()}

    def invoke(self, platform: str, root: Path, command) -> int:
        with patch("sys.argv", ["run_updater_smoke.py", "--platform", platform,
                                "--release-root", str(root), "--version", FIXTURE["version"]]), \
                patch.object(smoke.subprocess, "run", side_effect=command) as run:
            result = smoke.main()
        self.assertEqual(run.call_count, 1, "smoke must launch only the existing executable")
        return result

    def inspect_child(self, platform: str, root: Path, args, *, check, env):
        if platform.startswith("macos-"):
            executable = root / "bundle/macos/SpeleoDB Compass Sidecar.app/Contents/MacOS/speleodb-compass-sidecar"
        else:
            executable = root / ("speleodb-compass-sidecar.exe" if platform.startswith("windows") else "speleodb-compass-sidecar")
        self.assertEqual(args, [str(executable), "--verify-updater-runtime"])
        self.assertTrue(check)
        for key, value in os.environ.items():
            if not key.startswith("SIDECAR_UPDATER_SMOKE_"):
                self.assertEqual(env[key], value, f"preserve inherited runtime environment: {key}")
        self.assertEqual(env["SIDECAR_UPDATER_SMOKE_TARGETS"].split(","), smoke.TARGETS_BY_PLATFORM[platform])
        self.endpoint = env["SIDECAR_UPDATER_SMOKE_ENDPOINT"]
        asset_map = Path(env["SIDECAR_UPDATER_SMOKE_ASSETS"])
        self.assertEqual({path.name for path in asset_map.parent.iterdir()}, {"latest.json", "assets.json"})
        originals = json.loads(asset_map.read_text())
        with urlopen(self.endpoint, timeout=5) as response:
            latest = json.load(response)
        self.assertEqual(latest["version"], FIXTURE["version"])
        self.assertEqual(set(latest["platforms"]), set(smoke.TARGETS_BY_PLATFORM[platform]))
        for target, entry in latest["platforms"].items():
            self.assertEqual(entry["signature"], FIXTURE["platforms"][target]["signature"])
            name = Path(urlsplit(entry["url"]).path).name
            source = Path(originals[name])
            self.assertTrue(source.is_relative_to(root))
            with urlopen(entry["url"], timeout=5) as response:
                self.assertEqual(response.read(), source.read_bytes())
        # Only explicit public files are served, never the private path map or directories.
        for suffix in ["/", "/assets.json", "/../assets.json", "/%2e%2e/assets.json", "/missing"]:
            with self.assertRaises(HTTPError) as error:
                urlopen(self.endpoint.removesuffix("/latest.json") + suffix, timeout=5)
            self.assertEqual(error.exception.code, 404)
            error.exception.close()
        return subprocess.CompletedProcess(args, 0)

    def test_all_platforms_run_built_executable_without_compilation_or_payload_copies(self):
        for platform in smoke.TARGETS_BY_PLATFORM:
            with self.subTest(platform=platform):
                root = self.release(platform)
                before = self.snapshot(self.root)
                self.assertEqual(self.invoke(platform, root, lambda *args, **kwargs:
                                            self.inspect_child(platform, root, *args, **kwargs)), 0)
                self.assertEqual(self.snapshot(self.root), before)
                with self.assertRaises(URLError):
                    urlopen(self.endpoint, timeout=5)

    def test_binary_failure_is_fatal_and_server_closes_without_changing_caches(self):
        platform = "windows-x86_64"
        root = self.release(platform)
        before = self.snapshot(self.root)

        def failure(*args, **kwargs):
            self.inspect_child(platform, root, *args, **kwargs)
            raise subprocess.CalledProcessError(23, args[0])

        with self.assertRaises(subprocess.CalledProcessError) as error:
            self.invoke(platform, root, failure)
        self.assertEqual(error.exception.returncode, 23)
        self.assertEqual(self.snapshot(self.root), before)
        with self.assertRaises(URLError):
            urlopen(self.endpoint, timeout=5)

    def test_missing_executable_fails_without_fallback_build(self):
        with patch.object(smoke.subprocess, "run") as run:
            with self.assertRaisesRegex(FileNotFoundError, "already-built release executable"):
                smoke.release_executable("windows-x86_64", self.root)
            run.assert_not_called()

    def test_macos_requires_unique_bundled_binary_even_when_raw_binary_exists(self):
        root = self.release("macos-arm64")
        executable = smoke.release_executable("macos-arm64", root)
        executable.unlink()
        with self.assertRaisesRegex(ValueError, "expected exactly one match"):
            smoke.release_executable("macos-arm64", root)
        executable.touch()
        duplicate = root / "bundle/macos/Another.app/Contents/MacOS/speleodb-compass-sidecar"
        duplicate.parent.mkdir(parents=True)
        duplicate.touch()
        with self.assertRaisesRegex(ValueError, "expected exactly one match"):
            smoke.release_executable("macos-arm64", root)

    def test_missing_signature_fails_without_launching_binary(self):
        root = self.release("windows-x86_64")
        next(root.rglob("*.sig")).unlink()
        with patch("sys.argv", ["run_updater_smoke.py", "--platform", "windows-x86_64",
                                "--release-root", str(root), "--version", FIXTURE["version"]]), \
                patch.object(smoke.subprocess, "run") as run:
            with self.assertRaisesRegex(ValueError, "missing updater signature"):
                smoke.main()
            run.assert_not_called()


if __name__ == "__main__":
    unittest.main()
