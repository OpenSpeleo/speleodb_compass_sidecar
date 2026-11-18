#!/usr/bin/env python3
"""Validate Tauri 2 updater metadata before publishing it."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any, Optional
from urllib.parse import unquote, urlparse

LEGACY_UPDATER_SUFFIXES = (
    ".msi.zip",
    ".nsis.zip",
)

FULL_TARGETS = {
    "darwin-aarch64",
    "darwin-aarch64-app",
    "windows-x86_64",
    "windows-x86_64-msi",
    "windows-x86_64-nsis",
}

MODE_TARGETS = {
    "full": FULL_TARGETS,
    "macos-arm64": {"darwin-aarch64", "darwin-aarch64-app"},
    "windows-x86_64": {
        "windows-x86_64",
        "windows-x86_64-msi",
        "windows-x86_64-nsis",
    },
}

EXPECTED_SUFFIXES = {
    "darwin-aarch64": ".app.tar.gz",
    "darwin-aarch64-app": ".app.tar.gz",
    "windows-x86_64": ".msi",
    "windows-x86_64-msi": ".msi",
    "windows-x86_64-nsis": ".exe",
}

ALIAS_GROUPS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("macOS arm64 app archive", ("darwin-aarch64", "darwin-aarch64-app")),
    ("Windows MSI", ("windows-x86_64", "windows-x86_64-msi")),
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Tauri updater latest.json metadata.",
    )
    parser.add_argument("--input", required=True, type=Path, help="Path to latest.json")
    parser.add_argument("--repo", required=True, help="Expected GitHub repo, for example OpenSpeleo/speleodb_compass_sidecar")
    parser.add_argument("--tag", required=True, help="Expected release tag")
    parser.add_argument(
        "--assets",
        type=Path,
        help="Optional newline-delimited release asset names that every updater URL must match",
    )
    parser.add_argument(
        "--mode",
        choices=sorted(MODE_TARGETS),
        default="full",
        help="Required platform target set to validate",
    )
    return parser.parse_args()


def load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise ValueError(f"{path} is not valid JSON: {error}") from error

    if not isinstance(data, dict):
        raise ValueError(f"{path} must contain a JSON object")

    return data


def load_assets(path: Optional[Path]) -> Optional[set[str]]:
    if path is None:
        return None

    return {
        line.strip()
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip()
    }


def validate_asset_set(assets: Optional[set[str]]) -> list[str]:
    if assets is None:
        return []

    errors: list[str] = []
    for asset_name in sorted(assets):
        for suffix in LEGACY_UPDATER_SUFFIXES:
            if asset_name.endswith(suffix) or asset_name.endswith(f"{suffix}.sig"):
                errors.append(f"legacy updater wrapper asset is not allowed: {asset_name}")

    return errors


def validate_signature(platform_key: str, signature: Any) -> list[str]:
    """Validate manifest signature shape without decoding the cryptographic token."""
    if not isinstance(signature, str) or not signature.strip():
        return [f"{platform_key}: signature must be a non-empty string"]
    if "\n" in signature or "\r" in signature:
        return [f"{platform_key}: signature must be a single-line string; found an embedded newline"]
    return []


def asset_name_from_url(
    platform_key: str,
    url: Any,
    expected_prefix: str,
) -> tuple[Optional[str], list[str]]:
    errors: list[str] = []
    if not isinstance(url, str) or not url:
        return None, [f"{platform_key}: url must be a non-empty string"]

    if not url.startswith(expected_prefix):
        errors.append(f"{platform_key}: url must start with {expected_prefix}")

    parsed = urlparse(url)
    asset_name = unquote(Path(parsed.path).name)
    if not asset_name:
        errors.append(f"{platform_key}: url does not contain an asset name")

    for suffix in LEGACY_UPDATER_SUFFIXES:
        if asset_name.endswith(suffix):
            errors.append(f"{platform_key}: legacy updater wrapper is not allowed: {asset_name}")

    expected_suffix = EXPECTED_SUFFIXES.get(platform_key)
    if expected_suffix is not None and not asset_name.endswith(expected_suffix):
        errors.append(
            f"{platform_key}: expected URL asset to end with {expected_suffix}, got {asset_name}"
        )

    return asset_name, errors


def validate_latest_json(
    data: dict[str, Any],
    *,
    repo: str,
    tag: str,
    assets: Optional[set[str]],
    mode: str,
) -> list[str]:
    errors: list[str] = []

    version = data.get("version")
    if not isinstance(version, str) or not version:
        errors.append("latest.json version must be a non-empty string")
    elif tag != f"v{version}":
        errors.append(f"release tag {tag} must match metadata version v{version}")

    platforms = data.get("platforms")
    if not isinstance(platforms, dict) or not platforms:
        return ["latest.json must contain a non-empty platforms object"]

    required_targets = MODE_TARGETS[mode].copy()
    missing_targets = sorted(required_targets - set(platforms))
    for target in missing_targets:
        errors.append(f"missing required updater target: {target}")

    extra_targets = sorted(set(platforms) - required_targets)
    for target in extra_targets:
        errors.append(f"unexpected updater target for mode {mode}: {target}")

    errors.extend(validate_asset_set(assets))

    expected_prefix = f"https://github.com/{repo}/releases/download/{tag}/"

    for platform_key, platform in sorted(platforms.items()):
        if not isinstance(platform, dict):
            errors.append(f"{platform_key}: platform entry must be a JSON object")
            continue

        errors.extend(validate_signature(platform_key, platform.get("signature")))
        asset_name, url_errors = asset_name_from_url(platform_key, platform.get("url"), expected_prefix)
        errors.extend(url_errors)

        if asset_name and assets is not None:
            if asset_name not in assets:
                errors.append(f"{platform_key}: referenced asset is not in upload set: {asset_name}")

    errors.extend(validate_alias_groups(platforms))

    return errors


def validate_alias_groups(platforms: dict[str, Any]) -> list[str]:
    errors: list[str] = []

    def is_dict_entry(key: str) -> bool:
        return isinstance(platforms.get(key), dict)

    for label, group_keys in ALIAS_GROUPS:
        present = [key for key in group_keys if is_dict_entry(key)]
        if len(present) != len(group_keys):
            continue

        urls = {platforms[key].get("url") for key in present}
        signatures = {platforms[key].get("signature") for key in present}

        if len(urls) > 1:
            errors.append(
                f"alias group '{label}' ({', '.join(group_keys)}) must share one URL; "
                f"got {len(urls)} distinct URLs"
            )
        if len(signatures) > 1:
            errors.append(
                f"alias group '{label}' ({', '.join(group_keys)}) must share one signature; "
                f"got {len(signatures)} distinct signatures"
            )

    msi_group = ("windows-x86_64", "windows-x86_64-msi")
    nsis_key = "windows-x86_64-nsis"
    if is_dict_entry(nsis_key) and all(is_dict_entry(key) for key in msi_group):
        nsis_signature = platforms[nsis_key].get("signature")
        msi_signatures = {platforms[key].get("signature") for key in msi_group}
        if nsis_signature is not None and nsis_signature in msi_signatures:
            errors.append(
                f"{nsis_key}: signature must differ from the MSI alias group "
                f"({', '.join(msi_group)}); NSIS signs a distinct artifact"
            )

    return errors


def main() -> int:
    args = parse_args()
    data = load_json(args.input)
    assets = load_assets(args.assets)
    errors = validate_latest_json(
        data,
        repo=args.repo,
        tag=args.tag,
        assets=assets,
        mode=args.mode,
    )

    if errors:
        for error in errors:
            print(f"error: {error}", file=sys.stderr)
        return 1

    print(f"Validated updater metadata: {args.input}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
