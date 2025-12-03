use crate::{
    ACTIVE_PROJECT_ID,
    state::AppState,
    zip_management::{cleanup_temp_zip, pack_project_working_copy, unpack_project_zip},
};
use common::{
    CompassProject, Error, SpeleoDbProjectRevision, UserPrefs,
    api_types::{ProjectInfo, ProjectRevisionInfo, ProjectSaveResult},
    compass_project_index_path, compass_project_working_path, ensure_compass_project_dirs_exist,
};
use log::{error, info};
use std::{
    fs::{copy, create_dir_all, read_dir},
    path::Path,
    process::Command,
};
use tauri::{AppHandle, Manager, State, Url};
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

#[tauri::command]
pub fn forget_user_prefs(app_state: State<'_, AppState>) -> Result<(), String> {
    app_state.forget_user_prefs().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn fetch_projects(app_state: State<'_, AppState>) -> Result<Vec<ProjectInfo>, String> {
    api::project::fetch_projects(&app_state.api_info())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn auth_request(
    app_state: State<'_, AppState>,
    email: Option<String>,
    password: Option<String>,
    oauth: Option<String>,
    instance: Url,
) -> Result<(), String> {
    info!("Starting auth request");
    let updated_token = if let Some(oauth_token) = oauth {
        api::auth::authorize_with_token(&instance, &oauth_token).await?
    } else {
        let email = email.ok_or("Email is required for email/password authentication")?;
        let password = password.ok_or("Password is required for email/password authentication")?;
        api::auth::authorize_with_email(&instance, &email, &password).await?
    };
    info!("Auth request successful, updating user preferences");
    let prefs = UserPrefs::new(instance, Some(updated_token));
    app_state
        .update_user_prefs(prefs)
        .map_err(|e| e.to_string())?;
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
    let index_project = match CompassProject::load_index_project(project_id) {
        Ok(p) => p.project,
        Err(_) => return Ok(false),
    };
    let working_project = match CompassProject::load_working_project(project_id) {
        Ok(p) => p.project,
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

pub async fn update_project_revision_info(
    app_state: &AppState,
    project_id: Uuid,
) -> Result<ProjectRevisionInfo, String> {
    match api::project::get_project_revisions(&app_state.api_info(), project_id).await {
        Ok(revisions) => {
            app_state.update_project(&revisions);
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
    match api::project::get_project_revisions(&app_state.api_info(), project_id).await {
        Ok(revisions) => {
            app_state.update_project(&revisions);
            let latest_revision = match revisions.latest_commit() {
                Some(latest) => {
                    info!(
                        "Latest revision for project {} is {}",
                        project_id, latest.hexsha
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

fn copy_dir_all<A: AsRef<Path>>(src: impl AsRef<Path>, dst: A) -> std::io::Result<()> {
    create_dir_all(&dst)?;
    for entry in read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn update_index(
    app_state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<CompassProject, String> {
    let version_info = match app_state.get_project(project_id) {
        Some(info) => info,
        None => update_project_revision_info(&app_state, project_id).await?,
    };
    ensure_compass_project_dirs_exist(project_id).map_err(|e| e.to_string())?;
    log::info!("Downloading project ZIP from");
    match api::project::download_project_zip(&app_state.api_info(), project_id).await {
        Ok(bytes) => {
            log::info!("Downloaded ZIP ({} bytes)", bytes.len());
            let project = unpack_project_zip(project_id, bytes)?;
            if let Some(latest_commit) = version_info.latest_commit() {
                SpeleoDbProjectRevision::from(latest_commit)
                    .save_revision_for_project(project_id)
                    .map_err(|e| e.to_string())?;
            };
            // Copy index to working copy
            let src = compass_project_index_path(project_id);
            let dst = compass_project_working_path(project_id);
            copy_dir_all(&src, &dst).map_err(|e| {
                format!(
                    "Failed to copy index to working copy ({} -> {}): {}",
                    src.display(),
                    dst.display(),
                    e
                )
            })?;

            Ok(project)
        }
        Err(Error::NoProjectData(project_id)) => {
            info!("Empty project on SpeleoDB");
            CompassProject::empty_project(project_id).map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("Failed to download project ZIP: {}", e)),
    }
}

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
        let compass_project = CompassProject::load_working_project(project_id)
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
        match update_project_revision_info(&app_state, project_id).await {
            Ok(project_info) => {
                info!(
                    "Updated project revision info after save: {:?}",
                    project_info
                );
                if let Some(latest_commit) = project_info.latest_commit() {
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
}

#[tauri::command]
pub async fn import_compass_project(
    app: tauri::AppHandle,
    project_id: Uuid,
) -> Result<CompassProject, Error> {
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
        let project = CompassProject::import_compass_project(&file_path, project_id)?;
        info!("Successfully imported Compass project: {project:?}");
        Ok(project)
    })
    .await
    .unwrap()
}

#[tauri::command]
pub fn set_active_project(project_id: Uuid) -> Result<(), String> {
    info!("Setting active project: {project_id}");
    *ACTIVE_PROJECT_ID.lock().unwrap() = Some(project_id);
    Ok(())
}

#[tauri::command]
pub fn clear_active_project() -> Result<(), String> {
    *ACTIVE_PROJECT_ID.lock().unwrap() = None;
    Ok(())
}

#[tauri::command]
pub async fn release_project_mutex(
    app_state: State<'_, AppState>,
    project_id: Uuid,
) -> Result<(), String> {
    api::project::release_project_mutex(&app_state.api_info(), &project_id)
        .await
        .map_err(|e| e.to_string())?;
    // Always return success (fire and forget)
    Ok(())
}

#[tauri::command]
pub async fn create_project(
    app_state: State<'_, AppState>,
    name: String,
    description: String,
    country: String,
    latitude: Option<String>,
    longitude: Option<String>,
) -> Result<ProjectInfo, String> {
    api::project::create_project(
        &app_state.api_info(),
        name,
        description,
        country,
        latitude,
        longitude,
    )
    .await
    .map_err(|e| e.to_string())
}
