use crate::{paths::compass_project_working_path, state::AppState, user_prefs::UserPrefs};
use common::{
    Error,
    api_types::{ProjectInfo, ProjectSaveResult},
};
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
pub async fn fetch_projects(app_handle: AppHandle) -> Result<(), String> {
    let app_state = app_handle.state::<AppState>();
    let projects = api::project::fetch_projects(&app_state.api_info())
        .await
        .map_err(|e| e.to_string())?;
    for project_info in &projects {
        app_state.update_project_info(project_info);
    }
    app_state.emit_app_state_change(&app_handle);
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
        api::auth::authorize_with_token(&instance, &oauth_token).await?
    } else {
        let email = email.ok_or("Email is required for email/password authentication")?;
        let password = password.ok_or("Password is required for email/password authentication")?;
        api::auth::authorize_with_email(&instance, &email, &password).await?
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
pub async fn acquire_project_mutex(
    app_state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<(), String> {
    api::project::acquire_project_mutex(&app_state.api_info(), project_id)
        .await
        .map_err(|e| e.to_string())
}

/*
#[tauri::command]
pub async fn project_working_copy_is_dirty(project_id: Uuid) -> Result<bool, String> {
    info!("Checking if working copy is dirty");
    // If there is no working copy, it's not dirty
    if !compass_project_working_path(project_id).exists() {
        return Ok(false);
    }
    // If there is no index, it's not dirty
    else if !compass_project_index_path(project_id).exists() {
        return Ok(false);
    }
    let index_project = match LocalProject::load_index_project(project_id) {
        Ok(p) => p.map,
        Err(_) => return Ok(false),
    };
    let working_project = match LocalProject::load_working_project(project_id) {
        Ok(p) => p.map,
        Err(_) => return Ok(false),
    };
    if index_project != working_project {
        info!("Working copy is dirty");
        Ok(true)
    } else {
        info!("Working copy is clean");
        Ok(false)
    }
}
*/

pub async fn update_project_info(
    app_state: &AppState,
    project_id: Uuid,
) -> Result<ProjectInfo, String> {
    match api::project::fetch_project_info(&app_state.api_info(), project_id).await {
        Ok(revisions) => {
            app_state.update_project_info(&revisions);
            Ok(revisions)
        }
        Err(e) => {
            error!("Failed to get revisions for project {}: {}", project_id, e);
            return Err(format!(
                "Failed to get revisions for project {}: {}",
                project_id, e
            ));
        }
    }
}

/*
#[tauri::command]
pub async fn project_revision_is_current(
    app_state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<bool, String> {
    // Get the index revision for the project, if none, we're not up to date
    let Some(index_revision) = SpeleoDbProjectRevision::revision_for_project(project_id) else {
        info!("No index revision found for project {}", project_id);
        return Ok(false);
    };
    match update_project_info(&app_state, project_id).await {
        Ok(project_info) => {
            app_state.update_project_info(&project_info);
            let latest_revision = match project_info.latest_commit.as_ref() {
                Some(latest) => {
                    info!(
                        "Latest revision for project {} is {}",
                        project_id, latest.id
                    );
                    SpeleoDbProjectRevision::from(latest)
                }
                None => {
                    info!("No revisions found for project {}", project_id);
                    return Ok(true);
                }
            };
            if latest_revision == index_revision {
                info!(
                    "Project {} index is up to date (revision {})",
                    project_id, index_revision.revision
                );
                Ok(true)
            } else {
                info!(
                    "Project {} index is out of date (index: {:?}, latest: {:?})",
                    project_id, index_revision, latest_revision
                );
                Ok(false)
            }
        }
        Err(e) => {
            error!("Failed to get revisions for project {}: {}", project_id, e);
            return Err(format!(
                "Failed to get revisions for project {}: {}",
                project_id, e
            ));
        }
    }
}
*/

#[tauri::command]
pub fn open_project(project_id: Uuid) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut project_dir = compass_project_working_path(project_id);
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
        let compass_project = LocalProject::load_working_project(project_id)
            .map_err(|e| e.to_string())?
            .project
            .mak_file;
        info!("{compass_project:?}");
        if let Some(project_path) = compass_project {
            project_dir.push(project_path);
            info!("Attempting to open project with Compass Software");
            Command::new("explorer")
                .arg(project_dir)
                .spawn()
                .map_err(|e| e.to_string())?
                .wait()
                .map_err(|e| e.to_string())?;
            info!("Compass Closed Successfully");
        } else {
            std::process::Command::new("explorer")
                .arg(&project_dir)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
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
pub fn set_active_project(app_handle: AppHandle, project_id: Uuid) -> Result<(), String> {
    info!("Setting active project: {project_id}");
    let app_state = app_handle.state::<AppState>();
    app_state.set_active_project(Some(project_id), &app_handle);
    Ok(())
}

#[tauri::command]
pub fn clear_active_project(app_handle: AppHandle) -> Result<(), String> {
    info!("Clearing active project");
    let app_state = app_handle.state::<AppState>();
    app_state.set_active_project(None, &app_handle);
    Ok(())
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
) -> Result<ProjectInfo, String> {
    let app_state = app_handle.state::<AppState>();
    let project_info = api::project::create_project(
        &app_state.api_info(),
        name,
        description,
        country,
        latitude,
        longitude,
    )
    .await
    .map_err(|e| e.to_string())?;
    app_state.update_project_info(&project_info);
    app_state.set_active_project(Some(project_info.id), &app_handle);
    Ok(project_info)
}
