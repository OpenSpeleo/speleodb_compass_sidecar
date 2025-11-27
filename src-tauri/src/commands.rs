use crate::{
    api,
    state::{ApiInfo, ProjectInfoManager},
    zip_management::{cleanup_temp_zip, pack_project_working_copy, unpack_project_zip},
    ACTIVE_PROJECT_ID,
};
use log::{error, info};
use speleodb_compass_common::{
    api_types::{ProjectInfo, ProjectRevisionInfo, ProjectSaveResult},
    compass_project_index_path, compass_project_working_path, ensure_compass_project_dirs_exist,
    CompassProject, Error, SpeleoDbProjectRevision, UserPrefs,
};
use std::process::Command;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

#[tauri::command]
pub fn save_user_prefs(api_info: State<'_, ApiInfo>, prefs: UserPrefs) -> Result<(), String> {
    info!("Saving user preferences: {prefs:?}");
    UserPrefs::save(&prefs).map_err(|e| e.to_string())?;
    api_info.set(&prefs);
    Ok(())
}

#[tauri::command]
pub fn load_user_prefs(api_info: State<'_, ApiInfo>) -> Result<Option<UserPrefs>, String> {
    info!("Loading user preferences");
    let user_prefs = UserPrefs::load().map_err(|e| e.to_string())?;
    if let Some(user_prefs) = &user_prefs {
        info!("Loaded user prefs:{user_prefs:?}");
        api_info.set(user_prefs);
    }
    Ok(user_prefs)
}

#[tauri::command]
pub fn forget_user_prefs(api_info: State<'_, ApiInfo>) -> Result<(), String> {
    let result = UserPrefs::forget().map_err(|e| e.to_string());
    api_info.reset();
    result
}

#[tauri::command]
pub async fn fetch_projects(api_info: State<'_, ApiInfo>) -> Result<Vec<ProjectInfo>, String> {
    api::fetch_projects(&api_info).await
}

#[tauri::command]
pub async fn auth_request(
    api_info: State<'_, ApiInfo>,
    email: Option<String>,
    password: Option<String>,
    oauth: Option<String>,
    instance: String,
) -> Result<String, String> {
    info!("Starting auth request");
    let updated_token = if let Some(oauth_token) = oauth {
        api::authorize_with_token(&instance, &oauth_token).await?
    } else {
        let email = email.ok_or("Email is required for email/password authentication")?;
        let password = password.ok_or("Password is required for email/password authentication")?;
        api::authorize_with_email(&instance, &email, &password).await?
    };
    info!("Auth request successful, updating user preferences");
    let new_prefs = UserPrefs {
        instance,
        email: None,
        password: None,
        oauth_token: Some(updated_token.clone()),
    };
    api_info.set(&new_prefs);
    UserPrefs::save(&new_prefs).map_err(|e| e.to_string())?;
    Ok(updated_token)
}

#[tauri::command]
pub async fn acquire_project_mutex(
    api_info: State<'_, ApiInfo>,
    project_id: Uuid,
) -> Result<(), String> {
    api::acquire_project_mutex(&api_info, project_id).await
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
    api_info: &ApiInfo,
    project_info: &ProjectInfoManager,
    project_id: Uuid,
) -> Result<ProjectRevisionInfo, String> {
    match api::get_project_revisions(&api_info, project_id).await {
        Ok(revisions) => {
            project_info.update_project(&revisions);
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
    api_info: State<'_, ApiInfo>,
    project_info: State<'_, ProjectInfoManager>,
    project_id: Uuid,
) -> Result<bool, String> {
    // Get the index revision for the project, if none, we're not up to date
    let Some(index_revision) = SpeleoDbProjectRevision::revision_for_project(project_id) else {
        info!("No index revision found for project {}", project_id);
        return Ok(false);
    };
    match api::get_project_revisions(&api_info, project_id).await {
        Ok(revisions) => {
            project_info.update_project(&revisions);
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

#[tauri::command]
pub async fn update_index(
    api_info: State<'_, ApiInfo>,
    project_info: State<'_, ProjectInfoManager>,
    project_id: Uuid,
) -> Result<CompassProject, String> {
    let version_info = match project_info.get_project(project_id) {
        Some(info) => info,
        None => update_project_revision_info(&api_info, &project_info, project_id).await?,
    };
    ensure_compass_project_dirs_exist(project_id).map_err(|e| e.to_string())?;
    log::info!("Downloading project ZIP from");
    match api::download_project_zip(&api_info, project_id).await {
        Ok(bytes) => {
            log::info!("Downloaded ZIP ({} bytes)", bytes.len());
            let project = unpack_project_zip(project_id, bytes)?;

            if let Some(latest_commit) = version_info.latest_commit() {
                SpeleoDbProjectRevision::from(latest_commit)
                    .save_revision_for_project(project_id)
                    .map_err(|e| e.to_string())?;
            };

            Ok(project)
        }
        Err(Error::EmptyProjectDirectory(project_id)) => {
            info!("Empty project on SpeleoDB");
            CompassProject::empty_project(project_id).map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("Failed to download project ZIP: {}", e)),
    }
}

#[tauri::command]
pub fn unzip_project(zip_path: String, project_id: Uuid) -> serde_json::Value {
    use std::fs::File;
    use zip::ZipArchive;

    log::info!("Unzipping project {} from {}", project_id, zip_path);

    // Ensure compass directory exists
    if let Err(e) = speleodb_compass_common::ensure_compass_dir_exists() {
        return serde_json::json!({"ok": false, "error": format!("Failed to create compass directory: {}", e)});
    }

    // Create project-specific directory
    let project_path = match ensure_compass_project_dirs_exist(project_id) {
        Ok(p) => p,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to create project directory: {}", e)})
        }
    };

    // Open the ZIP file
    let file = match File::open(&zip_path) {
        Ok(f) => f,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to open ZIP file: {}", e)})
        }
    };

    let mut archive = match ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to read ZIP archive: {}", e)})
        }
    };

    // Extract all files
    for i in 0..archive.len() {
        let mut file = match archive.by_index(i) {
            Ok(f) => f,
            Err(e) => {
                return serde_json::json!({"ok": false, "error": format!("Failed to read ZIP entry {}: {}", i, e)})
            }
        };

        let outpath = match file.enclosed_name() {
            Some(path) => project_path.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            // Directory
            if let Err(e) = std::fs::create_dir_all(&outpath) {
                return serde_json::json!({"ok": false, "error": format!("Failed to create directory {}: {}", outpath.display(), e)});
            }
        } else {
            // File
            if let Some(p) = outpath.parent() {
                if let Err(e) = std::fs::create_dir_all(p) {
                    return serde_json::json!({"ok": false, "error": format!("Failed to create parent directory {}: {}", p.display(), e)});
                }
            }

            let mut outfile = match File::create(&outpath) {
                Ok(f) => f,
                Err(e) => {
                    return serde_json::json!({"ok": false, "error": format!("Failed to create file {}: {}", outpath.display(), e)})
                }
            };

            if let Err(e) = std::io::copy(&mut file, &mut outfile) {
                return serde_json::json!({"ok": false, "error": format!("Failed to write file {}: {}", outpath.display(), e)});
            }
        }
    }

    // Clean up temp ZIP file
    let _ = std::fs::remove_file(&zip_path);

    log::info!(
        "Successfully unzipped project to: {}",
        project_path.display()
    );

    serde_json::json!({
        "ok": true,
        "path": project_path.to_string_lossy().to_string(),
        "message": "Project extracted successfully"
    })
}

#[tauri::command]
pub fn open_project(project_id: Uuid) -> Result<(), String> {
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
    let api_info = app_handle.state::<ApiInfo>();
    let result =
        api::upload_project_zip(&api_info, project_id, commit_message, &zipped_project_path).await;
    // Clean up temp zip file regardless of success or failure
    cleanup_temp_zip(&zipped_project_path);

    let cloned_handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        let api_info = cloned_handle.state::<ApiInfo>();
        let project_info = cloned_handle.state::<ProjectInfoManager>();
        match update_project_revision_info(&api_info, &project_info, project_id).await {
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
    api_info: State<'_, ApiInfo>,
    project_id: Uuid,
) -> Result<(), String> {
    api::release_project_mutex(&api_info, &project_id).await?;
    // Always return success (fire and forget)
    Ok(())
}

#[tauri::command]
pub async fn create_project(
    name: String,
    description: String,
    country: String,
    latitude: Option<String>,
    longitude: Option<String>,
) -> serde_json::Value {
    use reqwest::Client;
    use std::time::Duration;

    // Load user prefs
    let prefs = match UserPrefs::load() {
        Ok(p) => p,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to load user preferences: {}", e)})
        }
    };

    let prefs = match prefs {
        Some(p) => p,
        _ => {
            return serde_json::json!({"ok": false, "error": "No instance URL in user preferences"});
        }
    };

    let oauth = match prefs.oauth_token {
        Some(t) => t,
        _ => {
            return serde_json::json!({"ok": false, "error": "No OAuth token in user preferences"});
        }
    };

    let base = prefs.instance.trim_end_matches('/');
    let url = format!("{}{}", base, "/api/v1/projects/");

    let client = match Client::builder().timeout(Duration::from_secs(30)).build() {
        Ok(c) => c,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to build HTTP client: {}", e)})
        }
    };

    let mut body = serde_json::Map::new();
    body.insert("name".to_string(), serde_json::json!(name));
    body.insert("description".to_string(), serde_json::json!(description));
    body.insert("country".to_string(), serde_json::json!(country));
    if let Some(lat) = latitude {
        if !lat.is_empty() {
            body.insert("latitude".to_string(), serde_json::json!(lat));
        }
    }
    if let Some(lon) = longitude {
        if !lon.is_empty() {
            body.insert("longitude".to_string(), serde_json::json!(lon));
        }
    }

    log::info!("Creating project: {}", name);

    let resp = match client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Network request failed: {}", e)})
        }
    };

    let status = resp.status();

    if status.is_success() {
        let json: serde_json::Value = match resp.json().await {
            Ok(j) => j,
            Err(e) => {
                return serde_json::json!({"ok": false, "error": format!("Failed to parse response: {}", e)})
            }
        };

        // Extract the project data from the API response
        let project_data = match json.get("data") {
            Some(data) => data.clone(),
            None => {
                return serde_json::json!({"ok": false, "error": "No data field in API response"});
            }
        };

        // Return the project data wrapped in our standard format
        serde_json::json!({
            "ok": true,
            "data": project_data
        })
    } else {
        // Try to get error message from body
        let error_msg = if let Ok(err_json) = resp.json::<serde_json::Value>().await {
            err_json.to_string()
        } else {
            format!("Status {}", status.as_u16())
        };

        serde_json::json!({
            "ok": false,
            "error": format!("Create failed: {}", error_msg),
            "status": status.as_u16()
        })
    }
}
