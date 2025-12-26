mod commands;
mod paths;
mod project;
mod state;

use crate::{
    commands::{
        acquire_project_mutex, auth_request, clear_active_project, create_project,
        ensure_initialized, fetch_projects, import_compass_project, open_project,
        release_project_mutex, save_project, set_active_project, sign_out,
    },
    state::AppState,
};
use common::compass_home;
use semver::Version;
use tauri::Manager;

const SPELEODB_COMPASS_VERSION: Version = Version::new(0, 0, 1);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ensure the hidden application directory exists in the user's home directory.
    if let Err(e) = common::ensure_app_dir_exists() {
        eprintln!(
            "Failed to create application directory '{:?}': {:#}",
            compass_home(),
            e
        );
    }
    // This should be called as early in the execution of the app as possible
    #[cfg(debug_assertions)] // only enable instrumentation in development builds
    let devtools = tauri_plugin_devtools::init();

    // Initialize logging
    let _ = common::init_file_logger("info");

    if let Ok(path) = std::env::current_dir() {
        log::info!("Current working directory: {}", path.display());
    }

    // Log where we are logging to
    if compass_home().exists() {
        log::info!("Application starting. Logging to: {:?}", compass_home());
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            acquire_project_mutex,
            auth_request,
            clear_active_project,
            create_project,
            ensure_initialized,
            fetch_projects,
            sign_out,
            import_compass_project,
            open_project,
            release_project_mutex,
            set_active_project,
            save_project,
        ])
        .manage(AppState::new());
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }
    builder
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
                if let Some(project_id) = app_state.get_active_project() {
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
