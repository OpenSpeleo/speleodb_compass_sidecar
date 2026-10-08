# Development

Install Rust stable and the Bun version pinned in `app/.bun-version` (currently
1.4.2). Bun owns frontend packages and `app/bun.lock`; do not create npm, Yarn
or pnpm lockfiles. The private frontend package has no release version.

`app/bunfig.toml` sets `[run] bun = true`, so Vite, TypeScript, ESLint,
Prettier, Vitest, Playwright and the Tauri CLI use Bun, including executable
children with Node shebangs. There is no separate Node installation step. Keep
using `bun run test`: the configured runner is Vitest, and `bun test` runs a
different test framework. Node-compatible imports and `@types/node` describe
APIs supported by Bun and do not require a Node process.

## Rust and native prerequisites

Install Rust through
[rustup](https://www.rust-lang.org/learn/get-started#installing-rust). The Tauri
backend still requires Rust and Cargo even though the frontend now uses React.
Install the stable toolchain and the components used by `make lint`:

```sh
rustup toolchain install stable
rustup default stable
rustup component add rustfmt clippy
```

Install the native
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your
platform. Linux needs WebKitGTK 4.1, GTK 3, AppIndicator and librsvg development
libraries. macOS needs Xcode command-line tools.

## Windows

You will need the Windows toolchain and linker. For the local GNU setup:

```sh
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
rustup component add rustfmt clippy
```

Install [MSYS2](https://www.msys2.org/).

Go into the MSYS2 terminal and install the following packages to get
`libtool.exe`, `dlltool.exe` and the other build tools:

```sh
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain \
    mingw-w64-ucrt-x86_64-nasm
```

Add `C:\msys64\ucrt64\bin` to your PATH. Restart your terminal or update the
current environment to pick up the change. Windows CI uses MSVC with the Windows
SDK; the commands above retain the local GNU development setup.

## Tooling

From the repository root:

```sh
make setup                    # frozen Bun install, browsers, Rust helper tools
make dev                      # native Tauri app and Vite hot reload
make lint                     # Rust and frontend checks
make test                     # native, unit and browser tests
make build-ui                 # production frontend
make build-tauri              # packaged native application
```

`make setup` installs Chromium and WebKit for Playwright. On Linux, install
their system dependencies with
`cd app && bunx --bun --no-install playwright install --with-deps chromium webkit`
when needed. Native package builds still require the platform SDKs; frontend
browser tests do not replace native smoke tests.

The equivalent native commands are `bun run tauri dev` and `bun run tauri build`
from `app/`. Vite listens on `localhost:1420` with `strictPort`; stop an
existing listener if that port is occupied. It never silently changes ports or
kills unrelated processes. See [development server](docs/tauri-dev.md).

`bun run build` defaults to the debug/staging UI profile.
`bun run build:release` and the release workflow select production. Tauri builds
infer the native profile from their hook environment: `bun run tauri build`
selects production and `bun run tauri build --debug` selects staging.
`SIDECAR_UI_PROFILE` explicitly overrides this inference; Vite's default
production build mode does not choose the SpeleoDB instance.

The frontend imports plain CSS and preserves the existing HTML, assets and
static About page. No Rust WASM target, Trunk or wasm-pack is required. Native
Rust dependencies and their Cargo lockfile remain separate from frontend
packages. `make update` updates Rust dependencies within their declared bounds
and reports available frontend upgrades; review and apply frontend upgrades with
Bun.

VS Code recommendations cover Tauri, rust-analyzer, ESLint and Prettier. Rust
and TypeScript use language-specific formatters. `make pre-commit` runs
repository hooks after `make setup`; hooks include frontend checks for frontend
changes. The Markdown formatter hook uses prek's Bun environment and the system
Bun binary; it does not provision npm or Node.
