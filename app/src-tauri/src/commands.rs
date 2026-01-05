use crate::{
    paths::compass_project_working_path, project_management::LocalProject, state::AppState,
    user_prefs::UserPrefs,
};
use common::{Error, api_types::ProjectSaveResult};
use log::{error, info};
use std::process::Command;
use tauri::{AppHandle, Manager, State, Url};
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

#[tauri::command]
pub async fn ensure_initialized(app_handle: AppHandle) {
    info!("Ensuring app is initialized");
    let app_state = app_handle.state::<AppState>();
    app_state.init_app_state(&app_handle).await;
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
        .update_user_prefs(prefs, &app_handle)
        .map_err(|e| e.to_string())?;
    app_state.authenticated(&app_handle).await;
    Ok(())
}

#[tauri::command]
pub fn open_project(project_id: Uuid) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let project_dir = compass_project_working_path(project_id);
    #[cfg(not(target_os = "windows"))]
    let project_dir = compass_project_working_path(project_id);
    if !project_dir.exists() {
        return Err("Project folder does not exist".to_string());
    }

    // Just open the folder in system file explorer
    #[cfg(target_os = "macos")]
    Command::new("open")
        .arg(&project_dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    Command::new("xdg-open")
        .arg(&project_dir)
        .spawn()
        .map_err(|e| e.to_string())?;

    // On Windows, actually try to open the project with Compass if possible
    #[cfg(target_os = "windows")]
    {
        let compass_project = LocalProject::mak_file_path(project_id).map_err(|e| e.to_string())?;
        info!("{compass_project:?}");

        info!("Attempting to open project with Compass Software");
        Command::new("explorer")
            .arg(project_dir)
            .spawn()
            .map_err(|e| e.to_string())?
            .wait()
            .map_err(|e| e.to_string())?;
        info!("Compass Closed Successfully");
    }
    Ok(())
}

#[tauri::command]
pub async fn save_project(
    app_handle: AppHandle,
    project_id: Uuid,
    commit_message: String,
) -> Result<ProjectSaveResult, String> {
    /*
    log::info!("Zipping project folder for project: {}", project_id);
    let zipped_project_path = pack_project_working_copy(project_id)?;
    info!("Project zipped successfully, uploading project ZIP to SpeleoDB");
    let app_state = app_handle.state::<AppState>();
    let result = api::project::upload_project_zip(
        &app_state.api_info(),
        project_id,
        commit_message,
        &zipped_project_path,
    )
    .await;
    // Clean up temp zip file regardless of success or failure
    cleanup_temp_zip(&zipped_project_path);

    let cloned_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let app_state = cloned_handle.state::<AppState>();
        match update_project_info(&app_state, project_id).await {
            Ok(project_info) => {
                info!(
                    "Updated project revision info after save: {:?}",
                    project_info
                );
                if let Some(latest_commit) = project_info.latest_commit.as_ref() {
                    let latest_rev = SpeleoDbProjectRevision::from(latest_commit);
                    if let Err(e) = latest_rev.save_revision_for_project(project_id) {
                        error!(
                            "Failed to save latest revision for project {}: {}",
                            project_id, e
                        );
                    }
                }
            }
            Err(e) => error!("Failed to update project revision info after save: {}", e),
        }
    });
    Ok(result.map_err(|e| format!("Failed to upload project ZIP: {}", e))?)
    */
    Err("Saving projects is not yet implemented".to_string())
}

#[tauri::command]
pub async fn import_compass_project(app: tauri::AppHandle, project_id: Uuid) -> Result<(), Error> {
    tauri::async_runtime::spawn(async move {
        let Some(FilePath::Path(file_path)) = app
            .dialog()
            .file()
            .add_filter("MAK", &["mak"])
            .blocking_pick_file()
        else {
            return Err(Error::NoProjectSelected);
        };
        info!("Selected MAK file: {}", file_path.display());
        info!("Importing into Compass project: {:?}", project_id);
        //let project = LocalProject::import_compass_project(&file_path, project_id)?;
        error!("Importing Compass projects is not yet implemented");
        //info!("Successfully imported Compass project: {project:?}");
        Ok(())
    })
    .await
    .unwrap()
}

#[tauri::command]
pub async fn set_active_project(app_handle: AppHandle, project_id: Uuid) -> Result<(), Error> {
    info!("Setting active project: {project_id}");
    let app_state = app_handle.state::<AppState>();
    app_state
        .set_active_project(Some(project_id), &app_handle)
        .await
}

#[tauri::command]
pub async fn clear_active_project(app_handle: AppHandle) -> Result<(), Error> {
    info!("Clearing active project");
    let app_state = app_handle.state::<AppState>();
    app_state.set_active_project(None, &app_handle).await
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
    app_state.set_active_project(Some(id), &app_handle).await?;
    Ok(())
}
