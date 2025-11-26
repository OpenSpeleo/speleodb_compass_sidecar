use crate::{api, state::ApiInfo, ACTIVE_PROJECT_ID};
use log::info;
use speleodb_compass_common::{
    api_types::ProjectInfo, CompassProject, Error, ProjectMetadata, UserPrefs,
};
use tauri::State;
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
pub async fn acquire_project_mutex(project_id: String) -> Result<serde_json::Value, String> {
    use reqwest::Client;
    use std::time::Duration;

    // Load user prefs to get instance URL and OAuth token
    let prefs = match UserPrefs::load() {
        Ok(p) => p,
        Err(e) => {
            return Ok(
                serde_json::json!({"ok": false, "error": format!("Failed to load user preferences: {}", e)}),
            )
        }
    };

    let prefs = match prefs {
        Some(p) => p,
        _ => {
            return Ok(
                serde_json::json!({"ok": false, "error": "No instance URL in user preferences"}),
            );
        }
    };

    let oauth = match prefs.oauth_token {
        Some(t) => t.to_string(),
        _ => {
            return Ok(
                serde_json::json!({"ok": false, "error": "No OAuth token in user preferences"}),
            );
        }
    };

    let base = prefs.instance.trim_end_matches('/');
    // NOTE: Using /acquire/ endpoint - adjust if actual API endpoint differs
    let url = format!("{}/api/v1/projects/{}/acquire/", base, project_id);

    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => {
            return Ok(
                serde_json::json!({"ok": false, "locked": false, "message": format!("Failed to build HTTP client: {}", e)}),
            )
        }
    };

    let resp = match client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Ok(
                serde_json::json!({"ok": false, "locked": false, "message": format!("Network request failed: {}", e)}),
            )
        }
    };

    let status = resp.status();

    if status.is_success() {
        // Successfully acquired the mutex
        Ok(
            serde_json::json!({"ok": true, "locked": true, "message": "Project mutex acquired successfully"}),
        )
    } else if status.as_u16() == 409 || status.as_u16() == 423 {
        // 409 Conflict or 423 Locked - mutex is already held by another user
        Ok(
            serde_json::json!({"ok": true, "locked": false, "message": "Project is already locked by another user"}),
        )
    } else {
        // Other error
        Ok(
            serde_json::json!({"ok": false, "locked": false, "message": format!("Mutex acquisition failed with status {}", status.as_u16()), "status": status.as_u16()}),
        )
    }
}

#[tauri::command]
pub async fn download_project_zip(project_id: String) -> serde_json::Value {
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
    let url = format!(
        "{}/api/v1/projects/{}/download/compass_zip/",
        base, project_id
    );

    let client = match Client::builder().timeout(Duration::from_secs(60)).build() {
        Ok(c) => c,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to build HTTP client: {}", e)})
        }
    };

    log::info!("Downloading project ZIP from: {}", url);

    let resp = match client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Network request failed: {}", e)})
        }
    };

    let status = resp.status();

    // Handle 422 - Project has no compass data yet (new/empty project)
    if status.as_u16() == 422 {
        // Create the project directory even though there's no data to download
        if let Err(e) = speleodb_compass_common::ensure_project_compass_dir_exists(&project_id) {
            return serde_json::json!({
                "ok": false,
                "error": format!("Failed to create project directory: {}", e)
            });
        }

        log::info!("Created directory for empty project: {}", project_id);

        return serde_json::json!({
            "ok": true,
            "empty_project": true,
            "message": "Project contains no Compass data. Use 'Import from Disk' to initialize the project."
        });
    }

    if !status.is_success() {
        return serde_json::json!({
            "ok": false,
            "error": format!("Download failed with status {}", status.as_u16()),
            "status": status.as_u16(),
            "url": url  // Include URL for debugging
        });
    }

    // Get the bytes
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to read response body: {}", e)})
        }
    };

    // Save to temp directory
    let temp_dir = std::env::temp_dir();
    let zip_filename = format!("project_{}.zip", project_id);
    let zip_path = temp_dir.join(&zip_filename);

    match std::fs::write(&zip_path, &bytes) {
        Ok(_) => {
            log::info!("Downloaded ZIP to: {}", zip_path.display());
            serde_json::json!({
                "ok": true,
                "path": zip_path.to_string_lossy().to_string(),
                "size": bytes.len()
            })
        }
        Err(e) => {
            serde_json::json!({"ok": false, "error": format!("Failed to write ZIP file: {}", e)})
        }
    }
}

#[tauri::command]
pub fn unzip_project(zip_path: String, project_id: String) -> serde_json::Value {
    use std::fs::File;
    use zip::ZipArchive;

    log::info!("Unzipping project {} from {}", project_id, zip_path);

    // Ensure compass directory exists
    if let Err(e) = speleodb_compass_common::ensure_compass_dir_exists() {
        return serde_json::json!({"ok": false, "error": format!("Failed to create compass directory: {}", e)});
    }

    // Create project-specific directory
    let project_path = match speleodb_compass_common::ensure_project_compass_dir_exists(&project_id)
    {
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
pub fn open_project_folder(project_id: String) -> serde_json::Value {
    let project_path = speleodb_compass_common::project_compass_path(&project_id);

    if !project_path.exists() {
        return serde_json::json!({"ok": false, "error": "Project folder does not exist"});
    }

    // Open folder in system file explorer
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open")
        .arg(&project_path)
        .spawn();

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("explorer")
        .arg(&project_path)
        .spawn();

    #[cfg(target_os = "linux")]
    let result = std::process::Command::new("xdg-open")
        .arg(&project_path)
        .spawn();

    match result {
        Ok(_) => {
            log::info!("Opened project folder: {}", project_path.display());
            serde_json::json!({"ok": true, "message": "Folder opened successfully"})
        }
        Err(e) => {
            serde_json::json!({"ok": false, "error": format!("Failed to open folder: {}", e)})
        }
    }
}

#[tauri::command]
pub fn zip_project_folder(project_id: String) -> serde_json::Value {
    use std::fs::{self, File};
    use std::io::Write;
    use zip::ZipWriter;

    log::info!("Zipping project folder for project: {}", project_id);

    // Get project folder path
    let project_path = speleodb_compass_common::project_compass_path(&project_id);

    if !project_path.exists() {
        return serde_json::json!({
            "ok": false,
            "error": format!("Project folder does not exist: {}", project_path.display())
        });
    }

    // Create temp zip file
    let temp_dir = std::env::temp_dir();
    let zip_filename = format!("project_{}.zip", project_id);
    let zip_path = temp_dir.join(&zip_filename);

    let zip_file = match File::create(&zip_path) {
        Ok(f) => f,
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "error": format!("Failed to create ZIP file: {}", e)
            });
        }
    };

    let mut zip = ZipWriter::new(zip_file);

    // Helper function to recursively add directory to ZIP
    fn add_dir_to_zip(
        zip: &mut ZipWriter<File>,
        path: &std::path::Path,
        prefix: &str,
    ) -> std::io::Result<()> {
        let entries = fs::read_dir(path)?;

        for entry in entries {
            let entry = entry?;
            let entry_path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let zip_path = if prefix.is_empty() {
                name_str.to_string()
            } else {
                format!("{}/{}", prefix, name_str)
            };

            if entry_path.is_dir() {
                // Add directory - use () for default options
                zip.add_directory::<_, ()>(&zip_path, Default::default())?;
                // Recurse into subdirectory
                add_dir_to_zip(zip, &entry_path, &zip_path)?;
            } else {
                // Add file - use () for default options
                zip.start_file::<_, ()>(&zip_path, Default::default())?;
                let contents = fs::read(&entry_path)?;
                zip.write_all(&contents)?;
            }
        }
        Ok(())
    }

    // Add all files to ZIP
    if let Err(e) = add_dir_to_zip(&mut zip, &project_path, "") {
        return serde_json::json!({
            "ok": false,
            "error": format!("Failed to add files to ZIP: {}", e)
        });
    }

    // Finalize ZIP
    if let Err(e) = zip.finish() {
        return serde_json::json!({
            "ok": false,
            "error": format!("Failed to finalize ZIP: {}", e)
        });
    }

    log::info!("Created ZIP file: {}", zip_path.display());

    serde_json::json!({
        "ok": true,
        "path": zip_path.to_string_lossy().to_string()
    })
}

#[tauri::command]
pub async fn import_compass_project(
    app: tauri::AppHandle,
    project_metadata: ProjectMetadata,
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
        info!("Importing into Compass project: {:?}", project_metadata);
        let project = CompassProject::import_compass_project(&file_path, project_metadata)?;
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
) -> Result<serde_json::Value, String> {
    api::release_project_mutex(&api_info, &project_id).await;
    // Always return success (fire and forget)
    Ok(serde_json::json!({"ok": true, "message": "Mutex release attempted"}))
}

#[tauri::command]
pub async fn upload_project_zip(
    project_id: String,
    commit_message: String,
    zip_path: String,
) -> serde_json::Value {
    use reqwest::Client;
    use std::time::Duration;

    log::info!("Uploading project ZIP for project: {}", project_id);

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
    let url = format!(
        "{}/api/v1/projects/{}/upload/compass_zip/",
        base, project_id
    );

    // Read ZIP file
    let zip_bytes = match std::fs::read(&zip_path) {
        Ok(b) => b,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to read ZIP file: {}", e)});
        }
    };

    let client = match Client::builder().timeout(Duration::from_secs(120)).build() {
        Ok(c) => c,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Failed to build HTTP client: {}", e)});
        }
    };

    // Create multipart form
    let part = reqwest::multipart::Part::bytes(zip_bytes)
        .file_name("project.zip")
        .mime_str("application/zip")
        .unwrap();

    let form = reqwest::multipart::Form::new()
        .text("message", commit_message)
        .part("artifact", part);

    log::info!("Uploading to: {}", url);

    let resp = match client
        .put(&url)
        .header("Authorization", format!("Token {}", oauth))
        .multipart(form)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({"ok": false, "error": format!("Network request failed: {}", e)});
        }
    };

    let status = resp.status();

    // Clean up temp ZIP file
    let _ = std::fs::remove_file(&zip_path);

    if status.is_success() {
        log::info!("Successfully uploaded project: {}", project_id);
        serde_json::json!({
            "ok": true,
            "message": "Project uploaded successfully",
            "status": status.as_u16()
        })
    } else if status == reqwest::StatusCode::NOT_MODIFIED {
        log::info!("Project upload returned 304 Not Modified: {}", project_id);
        serde_json::json!({
            "ok": true,
            "message": "No changes detected",
            "status": 304
        })
    } else {
        serde_json::json!({
            "ok": false,
            "error": format!("Upload failed with status {}", status.as_u16()),
            "status": status.as_u16()
        })
    }
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

        // Extract project ID to create local folder
        if let Some(project_id) = project_data.get("id").and_then(|v| v.as_str()) {
            // Ensure compass directory exists
            if let Err(e) = speleodb_compass_common::ensure_compass_dir_exists() {
                return serde_json::json!({"ok": false, "error": format!("Failed to create compass directory: {}", e)});
            }

            // Create project-specific directory
            if let Err(e) = speleodb_compass_common::ensure_project_compass_dir_exists(project_id) {
                return serde_json::json!({"ok": false, "error": format!("Failed to create project directory: {}", e)});
            }

            log::info!(
                "Created local project directory for project: {}",
                project_id
            );
        }

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
