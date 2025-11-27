use bytes::Bytes;
use speleodb_compass_common::{
    compass_project_working_path, CompassProject, SPELEODB_COMPASS_PROJECT_FILE,
};
use std::{
    io::prelude::*,
    path::{Path, PathBuf},
};
use tauri::ipc::private::tracing::info;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

// Unpack a project zip directly into the compass project index
pub fn unpack_project_zip(project_id: Uuid, zip_bytes: Bytes) -> Result<CompassProject, String> {
    // Create temp zip file
    let temp_dir = std::env::temp_dir();
    let zip_filename = format!("project_{}.zip", project_id);
    let zip_path = temp_dir.join(&zip_filename);
    info!("Creating zip file in temp folder: {zip_path:?}");
    std::fs::write(&zip_path, &zip_bytes)
        .map_err(|e| format!("Failed to write temp zip file: {}", e))?;
    // Ensure compass project directory exists
    speleodb_compass_common::ensure_compass_project_dirs_exist(project_id)
        .map_err(|e| format!("Failed to create compass directory: {}", e))?;
    CompassProject::empty_project(project_id)
        .map_err(|e| format!("Failed to create empty project: {}", e))
}

/// Pack the working copy of a Compass project into a zip file and return the path to the zip.
pub fn pack_project_working_copy(project_id: Uuid) -> Result<PathBuf, String> {
    let project = CompassProject::load_working_project(project_id)
        .map_err(|e| format!("Failed to load project: {}", e))?;
    // Create temp zip file
    let temp_dir = std::env::temp_dir();
    let zip_filename = format!("project_{}.zip", project_id);
    let zip_path = temp_dir.join(&zip_filename);
    info!("Creating zip file in temp folder: {zip_path:?}");
    let zip_file = std::fs::File::create(&zip_path)
        .map_err(|e| format!("Failed to create temp zip file: {}", e))?;
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut zip_writer = zip::ZipWriter::new(zip_file);
    zip_writer
        .start_file(SPELEODB_COMPASS_PROJECT_FILE, options)
        .map_err(|e| e.to_string())?;
    let project_toml = toml::to_string_pretty(&project).map_err(|e| e.to_string())?;
    zip_writer
        .write_all(project_toml.as_bytes())
        .map_err(|e| e.to_string())?;
    let project_dir = compass_project_working_path(project.speleodb.id);

    if let Some(mak_file_path) = project.project.mak_file.as_ref() {
        let mak_full_path = project_dir.join(&mak_file_path);
        zip_writer
            .start_file(&mak_file_path, options)
            .map_err(|e| e.to_string())?;
        let mak_contents =
            std::fs::read(&mak_full_path).map_err(|e| format!("Failed to read MAK file: {}", e))?;
        zip_writer
            .write_all(&mak_contents)
            .map_err(|e| e.to_string())?;
    }

    for dat_path in project.project.dat_files.iter() {
        let dat_full_path = project_dir.join(dat_path);
        zip_writer
            .start_file(&dat_path, options)
            .map_err(|e| e.to_string())?;
        let dat_contents =
            std::fs::read(&dat_full_path).map_err(|e| format!("Failed to read DAT file: {}", e))?;
        zip_writer
            .write_all(&dat_contents)
            .map_err(|e| e.to_string())?;
    }
    zip_writer.finish().map_err(|e| e.to_string())?;
    Ok(zip_path)
}

pub fn cleanup_temp_zip(zip_path: &Path) {
    if zip_path.exists() {
        if let Err(e) = std::fs::remove_file(&zip_path) {
            log::warn!(
                "Failed to delete temp zip file {}: {}",
                zip_path.display(),
                e
            );
        } else {
            log::info!("Deleted temp zip file: {}", zip_path.display());
        }
    }
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
