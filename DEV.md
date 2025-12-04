$ sh <(curl https://create.tauri.app/sh)
  % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
                                 Dload  Upload   Total   Spent    Left  Speed
100 16121  100 16121    0     0    99k      0 --:--:-- --:--:-- --:--:--   99k

info: downloading create-tauri-app
✔ Project name · SpeleoDB Compass Sidecar
✔ Package name · speleodb-compass-sidecar
✔ Identifier · com.speleodb-compass-sidecar.app
✔ Choose which language to use for your frontend · Rust - (cargo)
✔ Choose your UI template · Yew - (https://yew.rs/)

# Windows 

You will need the windows toolchain & linker
```sh
rustup toolchain install stable-x86_64-pc-windows-gnu
rustup default stable-x86_64-pc-windows-gnu
```

Install: https://www.msys2.org/

Important: Go into the MSYS2 terminal and install the following packages to get libtool.exe/dlltool.exe and others

```sh
pacman -S --needed base-devel mingw-w64-ucrt-x86_64-toolchain \
    mingw-w64-ucrt-x86_64-nasm
```

Add `C:\msys64\ucrt64\bin` to your PATH. (Don't forget to restart your terminal or otherwise update the current environment to pick up the change.)

# Tooling

Template created!

Your system is missing dependencies (or they do not exist in $PATH):
╭───────────────┬───────────────────────────────────────────────────────────────────╮
│ Rust          │ Visit https://www.rust-lang.org/learn/get-started#installing-rust │
├───────────────┼───────────────────────────────────────────────────────────────────┤
│ Tauri CLI     │ Run `cargo install tauri-cli --version "^2.0.0" --locked`         │
├───────────────┼───────────────────────────────────────────────────────────────────┤
│ Trunk         │ Run `cargo install trunk --locked`                                │
├───────────────┼───────────────────────────────────────────────────────────────────┤
│ WASM-Pack     │ Run `cargo install wasm-pack`                                     │
├───────────────┼───────────────────────────────────────────────────────────────────┤
│ wasm32 target │ Run `rustup target add wasm32-unknown-unknown`                    │
╰───────────────┴───────────────────────────────────────────────────────────────────╯

Make sure you have installed the prerequisites for your OS: https://tauri.app/start/prerequisites/, then run:
  cd "SpeleoDB Compass Sidecar"
  cargo tauri android init
  cargo tauri ios init

For Desktop development, run:
  cargo tauri dev

For Android development, run:
  cargo tauri android dev

For iOS development, run:
  cargo tauri ios dev
