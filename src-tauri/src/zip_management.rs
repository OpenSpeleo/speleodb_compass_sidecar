use bytes::Bytes;
use speleodb_compass_common::CompassProject;
use uuid::Uuid;

pub fn unpack_project_zip(project_id: Uuid, _zip_bytes: Bytes) -> Result<CompassProject, String> {
    CompassProject::empty_project(project_id)
        .map_err(|e| format!("Failed to create empty project: {}", e))
}
/*
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
*/
