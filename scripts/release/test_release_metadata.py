#!/usr/bin/env python3
"""Exercise fragments and final aggregation against actual asset inventories."""

from __future__ import annotations

import argparse
import copy
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import release_metadata as metadata
from validate_latest_json import FULL_TARGETS

REPO = "OpenSpeleo/speleodb_compass_sidecar"


class ReleaseMetadataTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.assets = set()
        self.notes = "Release notes"
        self.inventory = self.root / "release.json"
        self.paths = {}
        for directory, filename, signature in [
            ("macos", "SpeleoDB Compass Sidecar.app.tar.gz", "mac-token"),
            ("msi", "SpeleoDB Compass Sidecar_1.2.3_x64_en-US.msi", "msi-token"),
            ("nsis", "SpeleoDB Compass Sidecar_1.2.3_x64-setup.exe", "nsis-token"),
        ]:
            payload = self.root / "bundle" / directory / filename
            payload.parent.mkdir(parents=True)
            payload.write_bytes(b"release bytes")
            payload.with_name(payload.name + ".sig").write_text(signature + "\n")
            # tauri-action may rename the uploaded macOS archive.
            published = filename.replace(".app.tar.gz", "_aarch64.app.tar.gz")
            self.assets.add(published)
            self.paths[directory] = payload
        self.release = {
            "draft": True, "tag_name": "v1.2.3", "body": self.notes,
            "assets": [{"name": name} for name in sorted(self.assets)],
        }
        self.inventory.write_text(json.dumps(self.release))
        self.args = argparse.Namespace(
            repo=REPO, tag="v1.2.3", version="1.2.3", release_root=self.root,
            release_json=self.inventory, output=self.root / "latest.json",
        )

    def fragment(self, platform):
        self.args.platform = platform
        return metadata.fragment(self.args, self.assets, self.notes)

    def inputs(self):
        paths = []
        for platform in metadata.PLATFORMS:
            path = self.root / f"latest-{platform}.json"
            metadata.write_json(path, self.fragment(platform))
            paths.append(path)
        self.args.input = paths
        return paths

    def test_fragments_use_exact_signatures_and_url_encoded_uploaded_names(self):
        data = self.fragment("macos-arm64")
        entry = data["platforms"]["darwin-aarch64"]
        self.assertEqual(entry, data["platforms"]["darwin-aarch64-app"])
        self.assertEqual(entry["signature"], "mac-token")
        self.assertEqual(entry["url"], f"https://github.com/{REPO}/releases/download/v1.2.3/"
                         "SpeleoDB%20Compass%20Sidecar_aarch64.app.tar.gz")
        self.assertEqual(data["notes"], self.notes)

    def test_aggregates_both_platforms_with_distinct_nsis_payload(self):
        self.inputs()
        data = metadata.aggregate(self.args, self.assets, self.notes)
        self.assertEqual(set(data["platforms"]), FULL_TARGETS)
        self.assertEqual(data["platforms"]["windows-x86_64"], data["platforms"]["windows-x86_64-msi"])
        self.assertNotEqual(data["platforms"]["windows-x86_64-nsis"]["signature"],
                            data["platforms"]["windows-x86_64"]["signature"])

    def test_missing_duplicate_or_invalid_platform_fragment_fails(self):
        paths = self.inputs()
        for inputs, message in [([], "both platform"), ([paths[0]], "both platform"),
                                ([paths[0], paths[0]], "duplicate platform")]:
            self.args.input = inputs
            with self.subTest(inputs=inputs), self.assertRaisesRegex(ValueError, message):
                metadata.aggregate(self.args, self.assets, self.notes)
        invalid = self.root / "invalid.json"
        invalid.write_text('{"platforms": {"windows-aarch64": {}}}')
        self.args.input = [invalid, paths[0]]
        with self.assertRaisesRegex(ValueError, "exactly one platform"):
            metadata.aggregate(self.args, self.assets, self.notes)

    def test_mismatched_versions_and_notes_fail(self):
        for key, value in [("version", "4.5.6"), ("notes", "different notes")]:
            paths = self.inputs()
            data = json.loads(paths[0].read_text())
            data[key] = value
            paths[0].write_text(json.dumps(data))
            with self.subTest(key=key), self.assertRaises(ValueError):
                metadata.aggregate(self.args, self.assets, self.notes)

    def test_inventory_rejects_published_wrong_tag_and_duplicate_assets(self):
        for key, value in [("draft", False), ("tag_name", "v4.5.6"),
                           ("body", 42), ("assets", [{"name": "duplicate"}] * 2),
                           ("assets", [{"name": ""}])]:
            data = copy.deepcopy(self.release)
            data[key] = value
            self.inventory.write_text(json.dumps(data))
            with self.subTest(key=key), self.assertRaises(ValueError):
                metadata.release_assets(self.inventory, "v1.2.3")

    def test_missing_or_ambiguous_uploaded_payload_fails(self):
        archive = next(name for name in self.assets if name.endswith(".app.tar.gz"))
        for assets in [self.assets - {archive}, self.assets | {"other.app.tar.gz"}]:
            with self.subTest(assets=assets), self.assertRaisesRegex(ValueError, "exactly one published"):
                self.args.platform = "macos-arm64"
                metadata.fragment(self.args, assets, self.notes)

    def test_missing_empty_and_multiline_signatures_fail(self):
        signature = self.paths["macos"].with_name(self.paths["macos"].name + ".sig")
        for text in [None, "", "one\ntwo\n"]:
            if text is None:
                signature.unlink()
            else:
                signature.write_text(text)
            with self.subTest(text=text), self.assertRaises(ValueError):
                self.fragment("macos-arm64")

    def test_cli_writes_complete_manifest_and_preserves_existing_output_on_failure(self):
        paths = self.inputs()
        argv = ["release_metadata.py", "aggregate", "--repo", REPO, "--tag", "v1.2.3",
                "--release-json", str(self.inventory), "--input", *(str(p) for p in paths),
                "--output", str(self.args.output)]
        with patch("sys.argv", argv):
            self.assertEqual(metadata.main(), 0)
        before = self.args.output.read_bytes()
        self.release["draft"] = False
        self.inventory.write_text(json.dumps(self.release))
        with patch("sys.argv", argv):
            self.assertEqual(metadata.main(), 1)
        self.assertEqual(self.args.output.read_bytes(), before)


if __name__ == "__main__":
    unittest.main()
