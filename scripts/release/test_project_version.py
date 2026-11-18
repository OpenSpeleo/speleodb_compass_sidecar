#!/usr/bin/env python3
"""Tests for Sidecar's resolved Cargo/Tauri release version."""

from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from project_version import app_version_from_metadata, read_app_version


def metadata() -> dict:
    return {
        "workspace_members": ["api-id", "sidecar-id", "common-id", "xtask-id"],
        "workspace_default_members": ["api-id", "sidecar-id", "common-id"],
        "packages": [
            {"id": "api-id", "name": "api", "version": "1.2.3"},
            {"id": "sidecar-id", "name": "speleodb-compass-sidecar", "version": "1.2.3"},
            {"id": "common-id", "name": "common", "version": "1.2.3"},
            {"id": "xtask-id", "name": "xtask", "version": "1.2.3"},
        ],
    }


class AppVersionTests(unittest.TestCase):
    def test_selects_sidecar_among_multiple_default_members(self):
        self.assertEqual(app_version_from_metadata(metadata()), "1.2.3")

    def test_rejects_missing_or_ambiguous_sidecar(self):
        data = metadata()
        data["packages"] = [p for p in data["packages"] if p["name"] != "speleodb-compass-sidecar"]
        duplicate = metadata()
        duplicate["packages"].append(copy.deepcopy(duplicate["packages"][1]))
        nonmember = metadata()
        nonmember["workspace_members"].remove("sidecar-id")
        for value in [data, duplicate, nonmember, {}]:
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "exactly one"):
                app_version_from_metadata(value)

    def test_rejects_empty_or_nonstring_version(self):
        for version in [None, "", 123]:
            data = metadata()
            data["packages"][1]["version"] = version
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, "non-empty"):
                app_version_from_metadata(data)

    @patch("project_version.subprocess.run")
    def test_resolves_cargo_and_requires_matching_tauri_version(self, run):
        run.return_value = subprocess.CompletedProcess([], 0, json.dumps(metadata()), "")
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config = root / "app/src-tauri/tauri.conf.json"
            config.parent.mkdir(parents=True)
            config.write_text('{"version":"1.2.3"}')
            with patch.dict("project_version.os.environ", {}, clear=True):
                self.assertEqual(read_app_version(root), "1.2.3")
            run.assert_called_once_with(
                ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
                cwd=root, check=True, capture_output=True, text=True,
            )
            config.write_text('{"version":"9.9.9"}')
            with self.assertRaisesRegex(ValueError, "Tauri version must match"):
                read_app_version(root)

    @patch("project_version.subprocess.run", side_effect=subprocess.CalledProcessError(1, "cargo"))
    def test_cargo_failure_is_fatal(self, run):
        with self.assertRaises(subprocess.CalledProcessError):
            read_app_version()


if __name__ == "__main__":
    unittest.main()
