// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod updater_smoke;

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--verify-updater-runtime") {
        if let Err(error) = updater_smoke::verify() {
            eprintln!("Updater runtime verification failed: {error}");
            std::process::exit(1);
        }
        return;
    }
    let _guard = option_env!("SENTRY_DSN_SPELEODB_COMPASS").map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions::new()
                .maybe_release(sentry::release_name!())
                // Capture user IPs and potentially sensitive headers when using HTTP server integrations
                // see https://docs.sentry.io/platforms/rust/data-management/data-collected for more info
                .send_default_pii(true),
        ))
    });
    speleodb_compass_sidecar_lib::run()
}
