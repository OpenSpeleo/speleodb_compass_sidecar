# SpeleoDB Compass Sidecar

A Tauri desktop companion for synchronizing SpeleoDB projects and editing them
with Compass. The frontend uses React and TypeScript; Rust owns authentication,
HTTP requests, local files, project locks, Compass processes, menus and updates.

Install the Bun version in `app/.bun-version`, Node 24 and the native Rust/Tauri
prerequisites described in [Development](DEV.md), then run:

```sh
make setup
make dev
```

Use `make lint`, `make test`, and `make build-tauri` to validate and package the
app. [Testing](TESTING.md) distinguishes deterministic frontend/native coverage
from tests requiring real SpeleoDB credentials.

Initial Compass import supports choosing MAK survey sections and automatically
including dependencies. Original source files remain unchanged; see
[Compass import](docs/compass-import.md).

See [Frontend architecture](docs/react-frontend.md),
[Development server](docs/tauri-dev.md), and
[Release workflow](docs/release-workflow.md) for engineering details.
