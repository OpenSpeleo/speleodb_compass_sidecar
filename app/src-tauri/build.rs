fn main() {
    // Rebuild when the Sentry DSN env var changes so option_env!() picks it up.
    println!("cargo:rerun-if-env-changed=SENTRY_DSN_SPELEODB_COMPASS");
    tauri_build::build()
}
