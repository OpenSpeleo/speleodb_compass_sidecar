#!/usr/bin/env python3
"""Resolve the application version from the root Cargo workspace."""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path
from typing import Any


def app_version_from_metadata(metadata: dict[str, Any]) -> str:
    """Return the Sidecar workspace package's resolved version."""
    workspace_members = set(metadata.get("workspace_members", []))
    packages = [
        package
        for package in metadata.get("packages", [])
        if package.get("id") in workspace_members
        and package.get("name") == "speleodb-compass-sidecar"
    ]
    if len(packages) != 1:
        raise ValueError(
            "Cargo workspace must contain exactly one speleodb-compass-sidecar package"
        )

    version = packages[0].get("version")
    if not isinstance(version, str) or not version:
        raise ValueError("Sidecar Cargo workspace package must have a non-empty version")
    return version


def read_app_version(repo_root: Path = Path(".")) -> str:
    """Resolve Cargo inheritance and require the Tauri package version to match."""
    cargo = os.environ.get("CARGO", "cargo")
    result = subprocess.run(
        [cargo, "metadata", "--locked", "--no-deps", "--format-version", "1"],
        cwd=repo_root,
        check=True,
        capture_output=True,
        text=True,
    )
    version = app_version_from_metadata(json.loads(result.stdout))
    config = json.loads((repo_root / "app/src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    if config.get("version") != version:
        raise ValueError("Tauri version must match the Sidecar Cargo workspace package version")
    return version


if __name__ == "__main__":
    print(read_app_version())
