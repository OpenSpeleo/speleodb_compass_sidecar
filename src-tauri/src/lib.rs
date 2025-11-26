mod api;
mod commands;
mod state;

use crate::{
    commands::{
        acquire_project_mutex, auth_request, clear_active_project, create_project,
        download_project_zip, fetch_projects, forget_user_prefs, import_compass_project,
        load_user_prefs, open_project_folder, release_project_mutex, save_user_prefs,
        set_active_project, unzip_project, upload_project_zip, zip_project_folder,
    },
    state::ApiInfo,
};
use speleodb_compass_common::compass_home;
use tauri::Manager;
use uuid::Uuid;

#[cfg(debug_assertions)]
const API_BASE_URL: &str = "https://stage.speleodb.org";
#[cfg(not(debug_assertions))]
const API_BASE_URL: &str = "https://www.speleodb.com";

// Global state for active project
lazy_static::lazy_static! {
    static ref ACTIVE_PROJECT_ID: std::sync::Arc<std::sync::Mutex<Option<Uuid>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ensure the hidden application directory exists in the user's home directory.
    if let Err(e) = speleodb_compass_common::ensure_app_dir_exists() {
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
    let _ = speleodb_compass_common::init_file_logger("info");

    if let Ok(path) = std::env::current_dir() {
        log::info!("Current working directory: {}", path.display());
    }

    // Log where we are logging to
    if compass_home().exists() {
        log::info!("Application starting. Logging to: {:?}", compass_home());
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            acquire_project_mutex,
            clear_active_project,
            download_project_zip,
            fetch_projects,
            forget_user_prefs,
            load_user_prefs,
            auth_request,
            open_project_folder,
            release_project_mutex,
            save_user_prefs,
            import_compass_project,
            set_active_project,
            unzip_project,
            upload_project_zip,
            zip_project_folder,
            create_project,
        ])
        .manage(ApiInfo::default());
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }
    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(project_id) = ACTIVE_PROJECT_ID.lock().unwrap().as_ref() {
                    log::info!(
                        "App exit requested, releasing mutex for project: {}",
                        project_id
                    );
                    let runtime = tokio::runtime::Runtime::new().unwrap();
                    runtime.block_on(async {
                        let api = app_handle.state::<ApiInfo>();
                        api::release_project_mutex(&api, project_id).await;
                    });
                }
            }
        });
}
