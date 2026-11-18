#!/usr/bin/env python3
"""Fixture tests for Tauri updater metadata validation."""

from __future__ import annotations

import copy
import unittest
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urlparse

from validate_latest_json import load_json, validate_latest_json

FIXTURES = Path(__file__).parent / "fixtures"
TARGET_REPO = "OpenSpeleo/speleodb_compass_sidecar"


def asset_name_for_platform(data: dict, platform_key: str) -> str:
    url = data["platforms"][platform_key]["url"]
    return PurePosixPath(unquote(urlparse(url).path)).name


def upload_assets_for_metadata(data: dict) -> set[str]:
    assets = {"latest.json"}
    for platform_key in data["platforms"]:
        asset_name = asset_name_for_platform(data, platform_key)
        assets.add(asset_name)

    return assets


class ValidateLatestJsonTests(unittest.TestCase):
    def setUp(self) -> None:
        self.valid = load_json(FIXTURES / "latest.input.json")
        self.tag = f"v{self.valid['version']}"
        self.assets = upload_assets_for_metadata(self.valid)

    def validate(self, data: dict) -> list[str]:
        return validate_latest_json(
            data,
            repo=TARGET_REPO,
            tag=self.tag,
            assets=self.assets,
            mode="full",
        )

    def test_accepts_valid_tauri2_metadata(self) -> None:
        self.assertEqual(self.validate(copy.deepcopy(self.valid)), [])

    def test_rejects_release_tag_that_differs_from_metadata_version(self) -> None:
        errors = validate_latest_json(
            copy.deepcopy(self.valid),
            repo=TARGET_REPO,
            tag="v9.9.9",
            assets=self.assets,
            mode="full",
        )

        self.assertIn(
            f"release tag v9.9.9 must match metadata version v{self.valid['version']}",
            errors,
        )

    def test_fixture_covers_tauri_installer_targets(self) -> None:
        self.assertEqual(
            set(self.valid["platforms"]),
            {
                "darwin-aarch64",
                "darwin-aarch64-app",
                "windows-x86_64",
                "windows-x86_64-msi",
                "windows-x86_64-nsis",
            },
        )

    def test_rejects_extra_updater_target(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-aarch64"] = copy.deepcopy(data["platforms"]["windows-x86_64"])

        self.assertIn(
            "unexpected updater target for mode full: windows-aarch64",
            self.validate(data),
        )

    def test_rejects_missing_windows_nsis_exe_metadata(self) -> None:
        data = copy.deepcopy(self.valid)
        del data["platforms"]["windows-x86_64-nsis"]

        self.assertIn(
            "missing required updater target: windows-x86_64-nsis",
            self.validate(data),
        )

    def test_rejects_missing_signature(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64"]["signature"] = ""

        self.assertIn(
            "windows-x86_64: signature must be a non-empty string",
            self.validate(data),
        )

    def test_accepts_opaque_single_line_signature(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64-nsis"]["signature"] = "opaque-signature-token"

        self.assertEqual(self.validate(data), [])

    def test_rejects_multiline_signature(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64-nsis"]["signature"] = "line-one\nline-two"

        self.assertIn(
            "windows-x86_64-nsis: signature must be a single-line string; found an embedded newline",
            self.validate(data),
        )

    def test_rejects_alias_group_url_mismatch(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["darwin-aarch64-app"]["url"] = data["platforms"]["darwin-aarch64"]["url"].replace(".app.tar.gz", "-different.app.tar.gz")

        self.assertIn(
            "alias group 'macOS arm64 app archive' (darwin-aarch64, darwin-aarch64-app) "
            "must share one URL; got 2 distinct URLs",
            self.validate(data),
        )

    def test_rejects_alias_group_signature_mismatch(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64-msi"]["signature"] = "different-signature-token"

        self.assertIn(
            "alias group 'Windows MSI' (windows-x86_64, windows-x86_64-msi) "
            "must share one signature; got 2 distinct signatures",
            self.validate(data),
        )

    def test_rejects_nsis_signature_matching_msi_alias_group(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64-nsis"]["signature"] = data["platforms"]["windows-x86_64"][
            "signature"
        ]

        self.assertIn(
            "windows-x86_64-nsis: signature must differ from the MSI alias group "
            "(windows-x86_64, windows-x86_64-msi); NSIS signs a distinct artifact",
            self.validate(data),
        )

    def test_rejects_urls_from_another_repository(self) -> None:
        data = copy.deepcopy(self.valid)
        for entry in data["platforms"].values():
            entry["url"] = entry["url"].replace(TARGET_REPO, "owner/another-repo")
        self.assertTrue(any("url must start with" in error for error in self.validate(data)))

    def test_rejects_v1_compatible_wrappers(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64"]["url"] += ".zip"

        errors = self.validate(data)

        self.assertTrue(any("legacy updater wrapper is not allowed" in error for error in errors), errors)

    def test_rejects_wrong_windows_installer_suffix(self) -> None:
        data = copy.deepcopy(self.valid)
        data["platforms"]["windows-x86_64-nsis"]["url"] = data["platforms"]["windows-x86_64"]["url"]
        self.assertTrue(any("expected URL asset to end with .exe" in error
                            for error in self.validate(data)))

    def test_rejects_legacy_asset_in_upload_set(self) -> None:
        assets = set(self.assets or set())
        assets.add("Sidecar_0.5.0_x64.msi.zip")

        errors = validate_latest_json(
            copy.deepcopy(self.valid),
            repo=TARGET_REPO,
            tag=self.tag,
            assets=assets,
            mode="full",
        )

        self.assertIn(
            "legacy updater wrapper asset is not allowed: Sidecar_0.5.0_x64.msi.zip",
            errors,
        )

    def test_rejects_missing_referenced_asset(self) -> None:
        assets = set(self.assets or set())
        missing_asset = asset_name_for_platform(self.valid, "windows-x86_64-nsis")
        assets.remove(missing_asset)

        errors = validate_latest_json(
            copy.deepcopy(self.valid),
            repo=TARGET_REPO,
            tag=self.tag,
            assets=assets,
            mode="full",
        )

        self.assertIn(
            f"windows-x86_64-nsis: referenced asset is not in upload set: {missing_asset}",
            errors,
        )


if __name__ == "__main__":
    unittest.main()
