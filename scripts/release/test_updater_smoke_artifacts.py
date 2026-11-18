#!/usr/bin/env python3
"""Verify metadata served by the local updater smoke test."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from urllib.parse import unquote, urlparse

from updater_smoke_artifacts import UpdaterArtifact, build_latest_json, discover_artifacts

TAURI_SIGNATURE_TOKEN = (
    "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXkK"
    "UlVRQWZmSXhKRVVLT1loNnkyTE52bXNWVjRSVk92dElBTDViNnp5cmcwRG9z"
)


class BuildLatestJsonTests(unittest.TestCase):
    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.tmp_path = Path(self._tmp.name)

        self.artifact_path = self.tmp_path / "SpeleoDB Compass Sidecar.app.tar.gz"
        self.artifact_path.write_bytes(b"binary-payload")

        self.signature_path = self.tmp_path / "SpeleoDB Compass Sidecar.app.tar.gz.sig"
        self.signature_path.write_text(f"{TAURI_SIGNATURE_TOKEN}\n", encoding="utf-8")

    def _build(self) -> dict:
        artifact = UpdaterArtifact(
            platform_key="darwin-aarch64",
            artifact_path=self.artifact_path,
            signature_path=self.signature_path,
            asset_name="SpeleoDB%20Compass%20Sidecar_aarch64.app.tar.gz",
        )
        return build_latest_json(
            version="0.5.0",
            notes="Test release",
            url_base="https://github.com/OpenSpeleo/speleodb_compass_sidecar/releases/download/v0.5.0",
            artifacts=[artifact],
        )

    def test_signature_is_copied_from_sig_file_without_encoding(self) -> None:
        latest = self._build()
        signature_field = latest["platforms"]["darwin-aarch64"]["signature"]

        self.assertEqual(
            signature_field,
            TAURI_SIGNATURE_TOKEN,
            "signature field must use the single-line .sig token verbatim",
        )

    def test_url_joins_url_base_with_asset_name(self) -> None:
        latest = self._build()

        self.assertEqual(
            latest["platforms"]["darwin-aarch64"]["url"],
            "https://github.com/OpenSpeleo/speleodb_compass_sidecar/releases/download/v0.5.0/SpeleoDB%20Compass%20Sidecar_aarch64.app.tar.gz",
        )

    def test_rejects_multiline_signature_file(self) -> None:
        self.signature_path.write_text("line-one\nline-two\n", encoding="utf-8")

        with self.assertRaisesRegex(ValueError, "single-line"):
            self._build()


class DiscoverArtifactsTests(unittest.TestCase):
    """Smoke URLs must match the original on-disk payload filenames."""

    def setUp(self) -> None:
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)
        self.release_root = Path(self._tmp.name)
        self.bundle_root = self.release_root / "bundle"

    def _stage(self, relative_path: str, signature: bool = True) -> Path:
        path = self.bundle_root / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b"binary-payload")
        if signature:
            path.with_name(f"{path.name}.sig").write_text(
                f"{TAURI_SIGNATURE_TOKEN}\n",
                encoding="utf-8",
            )
        return path

    @staticmethod
    def _basename_from_url(url: str) -> str:
        return unquote(Path(urlparse(url).path).name)

    def _assert_url_basename_matches_disk(self, platform: str) -> None:
        artifacts = discover_artifacts(platform, self.release_root)
        self.assertTrue(artifacts, f"no artifacts discovered for {platform}")

        url_base = "https://github.com/OpenSpeleo/speleodb_compass_sidecar/releases/download/v0.5.0"
        latest = build_latest_json(
            version="0.5.0",
            notes="",
            url_base=url_base,
            artifacts=artifacts,
        )

        for key, platform_entry in latest["platforms"].items():
            url_name = self._basename_from_url(platform_entry["url"])
            artifact = next(a for a in artifacts if a.platform_key == key)
            self.assertEqual(
                url_name,
                artifact.artifact_path.name,
                f"{key}: latest.json URL must match the on-disk artifact basename",
            )

    def test_macos_arm64_url_matches_arch_suffixed_bundle_file(self) -> None:
        self._stage("macos/SpeleoDB Compass Sidecar_aarch64.app.tar.gz")
        self._assert_url_basename_matches_disk("macos-arm64")

    def test_macos_arm64_url_matches_default_bundle_file(self) -> None:
        self._stage("macos/SpeleoDB Compass Sidecar.app.tar.gz")
        self._assert_url_basename_matches_disk("macos-arm64")

    def test_windows_msi_and_nsis_urls_match_basenames(self) -> None:
        self._stage("msi/SpeleoDB Compass Sidecar_0.5.0_x64_en-US.msi")
        self._stage("nsis/SpeleoDB Compass Sidecar_0.5.0_x64-setup.exe")
        self._assert_url_basename_matches_disk("windows-x86_64")


if __name__ == "__main__":
    unittest.main()
