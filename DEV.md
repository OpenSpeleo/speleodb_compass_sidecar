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

Template created!

Your system is missing dependencies (or they do not exist in $PATH):
╭───────────────┬───────────────────────────────────────────────────────────────────╮
│ Rust          │ Visit https://www.rust-lang.org/learn/get-started#installing-rust │
├───────────────┼───────────────────────────────────────────────────────────────────┤
│ Tauri CLI     │ Run `cargo install tauri-cli --version '^2.0.0' --locked`         │
├───────────────┼───────────────────────────────────────────────────────────────────┤
│ Trunk         │ Run `cargo install trunk --locked`                                │
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
