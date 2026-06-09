mod commands;
mod paths;
mod project_management;
mod self_update;
mod state;
mod user_prefs;

use crate::{
    commands::{
        about_info, auth_request, check_for_updates_now, clear_active_project, create_project,
        discard_changes, dismiss_update_notification, ensure_initialized, import_compass_project,
        open_latest_release, open_project, pick_compass_project_file, reimport_compass_project,
        release_project_mutex, report_frontend_error, save_project, set_active_project, sign_out,
    },
    paths::{compass_home, ensure_app_dir_exists, init_file_logger},
    state::AppState,
};
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_dialog::{DialogExt, MessageDialogKind};

/// Whether the `SENTRY_VERIFY` env value requests a synthetic verification
/// event. Only the exact value "1" enables it.
fn sentry_verify_requested(value: Option<&str>) -> bool {
    value == Some("1")
}

/// Emit a synthetic event to confirm the Sentry pipeline end-to-end. Logs at
/// error! (forwarded to Sentry as an event) when a client is configured, or a
/// warning explaining that no DSN was compiled in.
fn emit_sentry_verification_event() {
    if sentry::Hub::current().client().is_some() {
        log::error!("SENTRY_VERIFY: synthetic verification event from startup");
        log::info!("SENTRY_VERIFY: verification event dispatched; check Sentry");
    } else {
        log::warn!("SENTRY_VERIFY set, but Sentry is not configured (no DSN compiled in)");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ensure the hidden application directory exists in the user's home directory.
    if let Err(e) = ensure_app_dir_exists() {
        eprintln!(
            "Failed to create application directory '{:?}': {:#}",
            compass_home(),
            e
        );
    }

    // Initialize logging
    let _ = init_file_logger("debug");
    log::info!(
        "Starting SpeleoDB Compass Sidecar v{}",
        env!("CARGO_PKG_VERSION")
    );

    if let Ok(path) = std::env::current_dir() {
        log::info!("Current working directory: {}", path.display());
    }

    // Log where we are logging to
    if compass_home().exists() {
        log::info!("Application starting. Logging to: {:?}", compass_home());
    }

    if sentry_verify_requested(std::env::var("SENTRY_VERIFY").ok().as_deref()) {
        emit_sentry_verification_event();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            about_info,
            auth_request,
            clear_active_project,
            check_for_updates_now,
            create_project,
            discard_changes,
            dismiss_update_notification,
            ensure_initialized,
            sign_out,
            import_compass_project,
            open_latest_release,
            pick_compass_project_file,
            reimport_compass_project,
            report_frontend_error,
            open_project,
            release_project_mutex,
            set_active_project,
            save_project,
        ])
        .manage(AppState::new())
        .setup(|app| {
            app.on_menu_event(move |app_handle, event| match event.id().0.as_str() {
                "sign_out" => {
                    log::info!("Sign out menu item clicked");
                    let app_state = app_handle.state::<AppState>();
                    app_state.sign_out(app_handle).ok();
                }
                "about" => {
                    log::info!("About menu item clicked");
                    if let Some(window) = app_handle.get_webview_window("about") {
                        window.set_focus().ok();
                    } else {
                        WebviewWindowBuilder::new(
                            app_handle,
                            "about",
                            WebviewUrl::App("about.html".into()),
                        )
                        .title("About SpeleoDB Compass Sidecar")
                        .inner_size(450.0, 550.0)
                        .resizable(false)
                        .build()
                        .ok();
                    }
                }
                "check_for_updates_now" => {
                    log::info!("Check for updates menu item clicked");
                    AppState::start_manual_update_check(app_handle);
                }
                _ => {}
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let app_state = window.state::<AppState>();
                if app_state.compass_is_open() {
                    log::info!("Window close prevented: Compass is still open");
                    api.prevent_close();
                    window
                        .dialog()
                        .message(
                            "Please close Compass before exiting to prevent losing unsaved work.",
                        )
                        .title("Compass is Open")
                        .kind(MessageDialogKind::Warning)
                        .blocking_show();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::Ready = event {
                let movable = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let app_state = movable.state::<AppState>();
                    app_state.init_app_state(&movable).await;
                });
            }
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let app_state = app_handle.state::<AppState>();
                if let Some(project_id) = app_state.get_active_project_id() {
                    log::info!(
                        "App exit requested, releasing mutex for project: {}",
                        project_id
                    );
                    tauri::async_runtime::block_on(async {
                        let app_state = app_handle.state::<AppState>();
                        api::project::release_project_mutex(&app_state.api_info(), project_id)
                            .await
                            .ok();
                    });
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::sentry_verify_requested;

    #[test]
    fn sentry_verify_requested_only_for_exactly_one() {
        assert!(sentry_verify_requested(Some("1")));
        assert!(!sentry_verify_requested(Some("0")));
        assert!(!sentry_verify_requested(Some("true")));
        assert!(!sentry_verify_requested(Some("")));
        assert!(!sentry_verify_requested(None));
    }
}
