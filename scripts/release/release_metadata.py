#!/usr/bin/env python3
"""Build independent updater fragments and combine them after parallel uploads."""

from __future__ import annotations

import argparse
from dataclasses import replace
import json
import os
from pathlib import Path
import sys
import tempfile
from typing import Any
from urllib.parse import quote

from updater_smoke_artifacts import build_latest_json, discover_artifacts
from validate_latest_json import EXPECTED_SUFFIXES, MODE_TARGETS, load_json, validate_latest_json

PLATFORMS = ("macos-arm64", "windows-x86_64")


def release_assets(path: Path, tag: str) -> tuple[set[str], str]:
    release = load_json(path)
    if release.get("draft") is not True:
        raise ValueError("updater metadata requires an existing draft release")
    if release.get("tag_name") != tag:
        raise ValueError("release inventory tag does not match the requested tag")
    notes = release.get("body")
    if notes is None:
        notes = ""
    if not isinstance(notes, str):
        raise ValueError("release body must be a string or null")
    inventory = release.get("assets")
    if not isinstance(inventory, list):
        raise ValueError("release inventory must contain an assets array")
    names = []
    for asset in inventory:
        if not isinstance(asset, dict) or not isinstance(asset.get("name"), str) or not asset["name"]:
            raise ValueError("every release asset must have a nonempty name")
        names.append(asset["name"])
    if len(names) != len(set(names)):
        raise ValueError("release inventory contains duplicate asset names")
    return set(names), notes


def validate(data: dict[str, Any], args: argparse.Namespace, assets: set[str], mode: str, notes: str) -> None:
    errors = validate_latest_json(data, repo=args.repo, tag=args.tag, assets=assets, mode=mode)
    if data.get("notes") != notes:
        errors.append("updater notes must match the draft release body")
    if errors:
        raise ValueError("\n".join(errors))


def fragment(args: argparse.Namespace, assets: set[str], notes: str) -> dict[str, Any]:
    artifacts = []
    for artifact in discover_artifacts(args.platform, args.release_root):
        suffix = EXPECTED_SUFFIXES[artifact.platform_key]
        matches = sorted(name for name in assets if name.endswith(suffix))
        if len(matches) != 1:
            raise ValueError(f"expected exactly one published {suffix} payload, found {len(matches)}")
        name = matches[0]
        # tauri-action can rename the macOS archive; the release inventory owns
        # the published filename while local discovery owns its signed bytes.
        artifacts.append(replace(artifact, asset_name=quote(name, safe="")))
    data = build_latest_json(
        version=args.version,
        notes=notes,
        url_base=f"https://github.com/{quote(args.repo, safe='/')}/releases/download/{quote(args.tag, safe='')}",
        artifacts=artifacts,
    )
    validate(data, args, assets, args.platform, notes)
    return data


def aggregate(args: argparse.Namespace, assets: set[str], notes: str) -> dict[str, Any]:
    combined: dict[str, Any] = {}
    seen = set()
    for path in args.input:
        data = load_json(path)
        targets = data.get("platforms")
        if not isinstance(targets, dict):
            raise ValueError(f"{path}: platforms must be an object")
        modes = [mode for mode in PLATFORMS if set(targets) == MODE_TARGETS[mode]]
        if len(modes) != 1:
            raise ValueError(f"{path}: fragment must contain exactly one platform's updater targets")
        mode = modes[0]
        if mode in seen:
            raise ValueError(f"duplicate platform fragment: {mode}")
        validate(data, args, assets, mode, notes)
        seen.add(mode)
        if not combined:
            combined = {key: value for key, value in data.items() if key != "platforms"}
            combined["platforms"] = {}
        if data["version"] != combined["version"] or data["notes"] != combined["notes"]:
            raise ValueError("all updater fragments must have the same version and notes")
        combined["platforms"].update(targets)
    if seen != set(PLATFORMS):
        raise ValueError("both platform fragments are required")
    validate(combined, args, assets, "full", notes)
    return combined


def write_json(path: Path, data: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=path.parent, delete=False) as output:
            temporary = Path(output.name)
            json.dump(data, output, indent=2)
            output.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for command in ("fragment", "aggregate"):
        subparser = commands.add_parser(command)
        subparser.add_argument("--repo", required=True)
        subparser.add_argument("--tag", required=True)
        subparser.add_argument("--release-json", required=True, type=Path)
        subparser.add_argument("--output", required=True, type=Path)
        if command == "fragment":
            subparser.add_argument("--platform", required=True, choices=PLATFORMS)
            subparser.add_argument("--version", required=True)
            subparser.add_argument("--release-root", type=Path, default=Path("target/release"))
        else:
            subparser.add_argument("--input", required=True, nargs=2, type=Path)
    args = parser.parse_args()
    try:
        assets, notes = release_assets(args.release_json, args.tag)
        data = fragment(args, assets, notes) if args.command == "fragment" else aggregate(args, assets, notes)
        write_json(args.output, data)
    except (OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
