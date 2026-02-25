use crate::{
    SPELEODB_COMPASS_VERSION, paths::compass_project_working_path,
    project_management::LocalProject, state::AppState, user_prefs::UserPrefs,
};
use common::{Error, api_types::ProjectSaveResult};
use log::info;
use serde::Serialize;
use std::{path::PathBuf, process::Command, sync::mpsc, time::Duration};
use tauri::{AppHandle, Manager, State, Url};
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

const FILE_PICKER_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Serialize)]
pub struct AboutInfo {
    version: String,
    repo: &'static str,
    authors: Vec<&'static str>,
    description: &'static str,
}

#[tauri::command]
pub fn about_info() -> AboutInfo {
    AboutInfo {
        version: SPELEODB_COMPASS_VERSION.to_string(),
        repo: "https://github.com/OpenSpeleo/speleodb_compass_sidecar",
        authors: vec!["Jonathan Dekhtiar", "Zachary Heylmun"],
        description: "Companion app to use SpeleoDB with Compass",
    }
}

#[tauri::command]
pub fn ensure_initialized(app_handle: AppHandle) {
    let app_state = app_handle.state::<AppState>();

    // Mark the WebView as ready BEFORE doing anything else.
    // This is the signal that the frontend JS runtime is alive and the
    // Tauri IPC bridge is functional — it's now safe to call emit_str().
    app_state.mark_webview_ready();
    app_state.reset_ui_state();

    // Spawn initialization on background task so command returns immediately
    tauri::async_runtime::spawn(async move {
        let app_state = app_handle.state::<AppState>();
        app_state.init_app_state(&app_handle).await;
    });
}

#[tauri::command]
pub fn sign_out(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    app_state.sign_out(&app_handle).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn auth_request(
    app_handle: AppHandle,
    email: Option<String>,
    password: Option<String>,
    oauth: Option<String>,
    instance: Url,
) -> Result<(), String> {
    info!("Starting auth request");
    let api_info = if let Some(oauth_token) = oauth {
        api::auth::authorize_with_token(instance, &oauth_token).await?
    } else {
        let email = email.ok_or("Email is required for email/password authentication")?;
        let password = password.ok_or("Password is required for email/password authentication")?;
        api::auth::authorize_with_email(instance, &email, &password).await?
    };
    info!("Auth request successful, updating user preferences");
    let prefs = UserPrefs::new(api_info);
    let app_state = app_handle.state::<AppState>();
    app_state
        .update_user_prefs(prefs)
        .map_err(|e| e.to_string())?;
    app_state.authenticated().await;
    Ok(())
}

#[tauri::command]
pub fn open_project(_app_state: State<'_, AppState>, project_id: Uuid) -> Result<(), Error> {
    let project_dir = compass_project_working_path(project_id);
    if !project_dir.exists() {
        return Err(Error::ProjectNotFound(project_dir));
    }

    // Just open the folder in system file explorer
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(&project_dir)
            .spawn()
            .map_err(|e| Error::OsCommand(e.to_string()))?;
        Ok(())
    }
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open")
            .arg(&project_dir)
            .spawn()
            .map_err(|e| Error::OsCommand(e.to_string()))?;
        Ok(())
    }

    // On Windows, actually try to open the project with Compass if possible
    #[cfg(target_os = "windows")]
    {
        const COMPASS_EXE: &str = r"C:\Fountainware\Compass\wcomp32\comp32.exe";

        // Check if Compass is installed
        let compass_path = std::path::Path::new(COMPASS_EXE);
        if !compass_path.exists() {
            // If compass isn't found, open the folder in explorer, but return an error so the UI can notify the user
            return Err(Error::CompassNotFound);
        }
        let project_path = LocalProject::mak_file_path(project_id)?;

        log::info!(
            "Opening {} with Compass: {}",
            project_path.display(),
            COMPASS_EXE
        );

        // Open the .MAK file with Compass
        let child_process = match Command::new(COMPASS_EXE).arg(&project_path).spawn() {
            Ok(child) => {
                log::info!("Compass launched with PID: {}", child.id());
                Ok(child)
            }
            Err(e) => {
                log::error!("Failed to open project with Compass: {}", e);
                Err(Error::CompassExecutable(e.to_string()))
            }
        }?;
        let pid = child_process.id();
        _app_state.set_compass_pid(Some(pid));
        Ok(())
    }
}

#[tauri::command]
pub async fn save_project(
    app_handle: AppHandle,
    commit_message: String,
) -> Result<ProjectSaveResult, Error> {
    info!("Project zipped successfully, uploading project ZIP to SpeleoDB");
    let app_state = app_handle.state::<AppState>();
    app_state.save_active_project(commit_message).await
}

async fn pick_compass_project_file_path(app_handle: &AppHandle) -> Result<PathBuf, Error> {
    let (tx, rx) = mpsc::channel::<Option<FilePath>>();
    app_handle
        .dialog()
        .file()
        .add_filter("MAK", &["mak"])
        .pick_file(move |file_path| {
            let _ = tx.send(file_path);
        });

    // Wait off the async runtime thread so we never block the UI event loop.
    let file_path =
        tauri::async_runtime::spawn_blocking(move || rx.recv_timeout(FILE_PICKER_TIMEOUT))
            .await
            .map_err(|e| Error::OsCommand(format!("File picker task failed: {e}")))?
            .map_err(|e| Error::OsCommand(format!("File picker timed out or failed: {e}")))?;

    let Some(file_path) = file_path else {
        return Err(Error::NoProjectSelected);
    };

    match file_path {
        FilePath::Path(path) => Ok(path),
        FilePath::Url(url) => url.to_file_path().map_err(|_| {
            Error::Deserialization("Failed to convert selected file URL to path".into())
        }),
    }
}

async fn import_project_from_path(
    app_handle: AppHandle,
    project_id: Uuid,
    mak_path: PathBuf,
    commit_message: String,
    clear_working_copy: bool,
) -> Result<(), Error> {
    info!("Selected MAK file: {}", mak_path.display());
    info!("Importing into Compass project: {:?}", project_id);

    if clear_working_copy {
        LocalProject::clear_working_copy_compass_artifacts(project_id)?;
    }

    LocalProject::import_compass_project(project_id, &mak_path)?;
    info!("Successfully imported Compass project from : {mak_path:?}");
    save_project(app_handle, commit_message).await?;
    Ok(())
}

#[tauri::command]
pub async fn import_compass_project(
    app_handle: AppHandle,
    project_id: Uuid,
) -> Result<bool, Error> {
    let file_path = match pick_compass_project_file_path(&app_handle).await {
        Ok(path) => path,
        Err(Error::NoProjectSelected) => return Ok(false),
        Err(err) => return Err(err),
    };
    import_project_from_path(
        app_handle,
        project_id,
        file_path,
        "Imported local project".to_string(),
        false,
    )
    .await?;
    Ok(true)
}

#[tauri::command]
pub async fn pick_compass_project_file(app_handle: AppHandle) -> Result<Option<String>, Error> {
    match pick_compass_project_file_path(&app_handle).await {
        Ok(path) => Ok(Some(path.to_string_lossy().to_string())),
        Err(Error::NoProjectSelected) => Ok(None),
        Err(err) => Err(err),
    }
}

#[tauri::command]
pub async fn reimport_compass_project(
    app_handle: AppHandle,
    project_id: Uuid,
    mak_path: String,
    commit_message: String,
) -> Result<(), Error> {
    import_project_from_path(
        app_handle,
        project_id,
        PathBuf::from(mak_path),
        commit_message,
        true,
    )
    .await
}

#[tauri::command]
pub async fn discard_changes(app_handle: AppHandle) -> Result<(), Error> {
    info!("Discarding local changes for active project");
    let app_state = app_handle.state::<AppState>();
    app_state.discard_active_project_changes().await
}

#[tauri::command]
pub async fn set_active_project(app_handle: AppHandle, project_id: Uuid) -> Result<(), Error> {
    info!("Setting active project: {project_id}");
    let app_state = app_handle.state::<AppState>();
    app_state.set_active_project(Some(project_id)).await
}

#[tauri::command]
pub async fn clear_active_project(app_handle: AppHandle) -> Result<(), Error> {
    info!("Clearing active project");
    let app_state = app_handle.state::<AppState>();
    app_state.set_active_project(None).await
}

#[tauri::command]
pub async fn release_project_mutex(
    app_state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<(), String> {
    api::project::release_project_mutex(&app_state.api_info(), project_id)
        .await
        .map_err(|e| e.to_string())?;
    // Always return success (fire and forget)
    Ok(())
}

#[tauri::command]
pub async fn create_project(
    app_handle: AppHandle,
    name: String,
    description: String,
    country: String,
    latitude: Option<String>,
    longitude: Option<String>,
) -> Result<(), Error> {
    let app_state = app_handle.state::<AppState>();
    let project_info = api::project::create_project(
        &app_state.api_info(),
        name,
        description,
        country,
        latitude,
        longitude,
    )
    .await?;
    let id = project_info.id;
    app_state.update_local_project(project_info).await?;
    app_state.set_active_project(Some(id)).await?;
    Ok(())
}
