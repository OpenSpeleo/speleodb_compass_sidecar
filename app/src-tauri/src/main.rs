// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let _guard = option_env!("SENTRY_DSN_SPELEODB_COMPASS").map(|dsn| {
        sentry::init((
            dsn,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                // Capture user IPs and potentially sensitive headers when using HTTP server integrations
                // see https://docs.sentry.io/platforms/rust/data-management/data-collected for more info
                send_default_pii: true,
                ..Default::default()
            },
        ))
    });
    speleodb_compass_sidecar_lib::run()
}
