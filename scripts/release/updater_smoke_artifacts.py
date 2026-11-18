#!/usr/bin/env python3
"""Discover local payloads and build metadata for the updater smoke server."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class UpdaterArtifact:
    platform_key: str
    artifact_path: Path
    signature_path: Path
    asset_name: str


def find_one(root: Path, pattern: str) -> Path:
    matches = sorted(path for path in root.glob(pattern) if path.is_file())
    if len(matches) != 1:
        rendered = ", ".join(str(path) for path in matches) or "<none>"
        raise ValueError(f"expected exactly one match for {root / pattern}, found: {rendered}")
    return matches[0]


def with_signature(path: Path) -> tuple[Path, Path]:
    signature_path = path.with_name(f"{path.name}.sig")
    if not signature_path.is_file():
        raise ValueError(f"missing updater signature for {path}: {signature_path}")
    return path, signature_path


def read_signature(path: Path) -> str:
    signature = path.read_text(encoding="utf-8").strip()
    if not signature:
        raise ValueError(f"updater signature must be a non-empty string: {path}")
    if "\n" in signature or "\r" in signature:
        raise ValueError(f"updater signature must be a single-line string: {path}")
    return signature


def discover_artifacts(platform: str, release_root: Path) -> list[UpdaterArtifact]:
    """Discover the signed updater payloads built for this runner."""
    bundle_root = release_root / "bundle"

    if platform == "macos-arm64":
        artifact_path, signature_path = with_signature(find_one(bundle_root / "macos", "*.app.tar.gz"))
        asset_name = artifact_path.name
        return [
            UpdaterArtifact("darwin-aarch64", artifact_path, signature_path, asset_name),
            UpdaterArtifact("darwin-aarch64-app", artifact_path, signature_path, asset_name),
        ]

    if platform == "windows-x86_64":
        msi_path, msi_sig = with_signature(find_one(bundle_root / "msi", "*.msi"))
        nsis_path, nsis_sig = with_signature(find_one(bundle_root / "nsis", "*.exe"))
        return [
            UpdaterArtifact("windows-x86_64", msi_path, msi_sig, msi_path.name),
            UpdaterArtifact("windows-x86_64-msi", msi_path, msi_sig, msi_path.name),
            UpdaterArtifact("windows-x86_64-nsis", nsis_path, nsis_sig, nsis_path.name),
        ]

    raise ValueError(f"unsupported release platform: {platform}")


def build_latest_json(
    *,
    version: str,
    notes: str,
    url_base: str,
    artifacts: list[UpdaterArtifact],
) -> dict[str, Any]:
    platforms: dict[str, dict[str, str]] = {}
    for artifact in artifacts:
        signature = read_signature(artifact.signature_path)
        platforms[artifact.platform_key] = {
            "signature": signature,
            "url": f"{url_base.rstrip('/')}/{artifact.asset_name}",
        }

    return {
        "version": version,
        "notes": notes,
        "pub_date": datetime.now(timezone.utc)
        .isoformat(timespec="milliseconds")
        .replace("+00:00", "Z"),
        "platforms": platforms,
    }
