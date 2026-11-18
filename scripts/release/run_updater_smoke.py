#!/usr/bin/env python3
"""Run a no-install smoke test against locally built Tauri updater artifacts."""

from __future__ import annotations

import argparse
from dataclasses import replace
import json
import os
import subprocess
import sys
import tempfile
import threading
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit

from updater_smoke_artifacts import build_latest_json, discover_artifacts, find_one


# Tauri's default macOS lookup tries `<os>-<arch>-app` before `<os>-<arch>`
# (see tauri-plugin-updater's `get_urls`), so production app-bundled installs
# hit the `-app` keys first. Smoke both keys so we catch any drift between the
# canonical and installer-aware entries in the local smoke manifest.
TARGETS_BY_PLATFORM = {
    "macos-arm64": ["darwin-aarch64", "darwin-aarch64-app"],
    "windows-x86_64": [
        "windows-x86_64",
        "windows-x86_64-msi",
        "windows-x86_64-nsis",
    ],
}


class QuietHandler(SimpleHTTPRequestHandler):
    """Serve only the manifest and original payloads, without copying installers."""

    def __init__(self, *args: object, files: dict[str, Path], **kwargs: object) -> None:
        self.files = files
        super().__init__(*args, **kwargs)

    def send_head(self):
        if unquote(urlsplit(self.path).path) not in self.files:
            self.send_error(404)
            return None
        return super().send_head()

    def translate_path(self, path: str) -> str:
        return str(self.files[unquote(urlsplit(path).path)])

    def log_message(self, format: str, *args: object) -> None:
        sys.stdout.write(f"[updater-smoke] {format % args}\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Serve built updater artifacts locally and verify Tauri can download them.",
    )
    parser.add_argument("--platform", required=True, choices=sorted(TARGETS_BY_PLATFORM))
    parser.add_argument(
        "--release-root",
        type=Path,
        default=Path("target/release"),
        help="Tauri release target directory",
    )
    parser.add_argument("--version", required=True, help="Validated release version for latest.json")
    return parser.parse_args()


def prepare_artifacts(
    stage_dir: Path, platform: str, release_root: Path, url_base: str, version: str,
) -> dict[str, Path]:
    artifacts = discover_artifacts(platform, release_root)
    asset_paths: dict[str, Path] = {}
    for artifact in artifacts:
        source = artifact.artifact_path.resolve()
        if artifact.asset_name in asset_paths and asset_paths[artifact.asset_name] != source:
            raise ValueError(f"duplicate updater asset name: {artifact.asset_name}")
        asset_paths[artifact.asset_name] = source

    latest_json = build_latest_json(
        version=version,
        notes="Local updater smoke test",
        url_base=url_base,
        artifacts=[replace(artifact, asset_name=quote(artifact.asset_name, safe=""))
                   for artifact in artifacts],
    )
    latest_path = stage_dir / "latest.json"
    latest_path.write_text(f"{json.dumps(latest_json, indent=2)}\n", encoding="utf-8")
    (stage_dir / "assets.json").write_text(
        json.dumps({quote(name, safe=""): str(path) for name, path in asset_paths.items()}), encoding="utf-8",
    )
    return {"/latest.json": latest_path, **{f"/{name}": path for name, path in asset_paths.items()}}


def release_executable(platform: str, release_root: Path) -> Path:
    if platform.startswith("macos-"):
        # Exercise the executable shipped in the app bundle.
        executable = find_one(release_root / "bundle/macos", "*.app/Contents/MacOS/speleodb-compass-sidecar")
    else:
        executable = release_root / ("speleodb-compass-sidecar.exe" if platform == "windows-x86_64" else "speleodb-compass-sidecar")
    if not executable.is_file():
        raise FileNotFoundError(f"updater smoke requires the already-built release executable: {executable}")
    return executable.resolve()


def main() -> int:
    args = parse_args()
    executable = release_executable(args.platform, args.release_root)

    with tempfile.TemporaryDirectory(prefix="speleodb-compass-sidecar-updater-smoke-") as tmp:
        stage_dir = Path(tmp)
        files: dict[str, Path] = {}
        handler = partial(QuietHandler, files=files)
        server = ThreadingHTTPServer(("127.0.0.1", 0), handler)
        try:
            host, port = server.server_address
            url_base = f"http://{host}:{port}"
            files.update(prepare_artifacts(stage_dir, args.platform, args.release_root, url_base, args.version))

            env = os.environ.copy()
            env["SIDECAR_UPDATER_SMOKE_ENDPOINT"] = f"{url_base}/latest.json"
            env["SIDECAR_UPDATER_SMOKE_TARGETS"] = ",".join(TARGETS_BY_PLATFORM[args.platform])
            env["SIDECAR_UPDATER_SMOKE_ASSETS"] = str(stage_dir / "assets.json")

            thread = threading.Thread(target=server.serve_forever, daemon=True)
            thread.start()
            try:
                # Python waits for GUI-subsystem executables on Windows too.
                subprocess.run([str(executable), "--verify-updater-runtime"], check=True, env=env)
            finally:
                server.shutdown()
                thread.join()
        finally:
            server.server_close()

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
