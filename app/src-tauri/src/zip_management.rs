use bytes::Bytes;
use common::{
    CompassProject, SPELEODB_COMPASS_PROJECT_FILE, compass_project_index_path,
    compass_project_working_path,
};
use std::{
    fs::File,
    io::prelude::*,
    path::{Path, PathBuf},
};
use tauri::ipc::private::tracing::info;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

// Unpack a project zip directly into the index and return the resulting `CompassProject`.
pub fn unpack_project_zip(project_id: Uuid, zip_bytes: Bytes) -> Result<CompassProject, String> {
    // Create temp zip file
    let temp_dir = std::env::temp_dir();
    let zip_filename = format!("project_{}.zip", project_id);
    let zip_path = temp_dir.join(&zip_filename);
    info!("Creating zip file in temp folder: {zip_path:?}");
    std::fs::write(&zip_path, &zip_bytes)
        .map_err(|e| format!("Failed to write temp zip file: {}", e))?;
    // Unzip the project
    let file = std::fs::File::open(&zip_path)
        .map_err(|e| format!("Failed to open temp zip file: {}", e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {}", e))?;

    let index_path = compass_project_index_path(project_id);

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry {}: {}", i, e))?;

        let file_path = match file.enclosed_name() {
            Some(path) => index_path.join(path),
            None => continue,
        };
        if file.is_dir() {
            // Ignore, we automatically create directories for files as needed below
        } else {
            // Create parent directories if they don't exist
            if let Some(p) = file_path.parent() {
                std::fs::create_dir_all(p).map_err(|e| {
                    format!("Failed to create parent directory {}: {}", p.display(), e)
                })?;
            }
            let mut out_file = File::create(&file_path)
                .map_err(|e| format!("Failed to create file {}: {}", file_path.display(), e))?;

            std::io::copy(&mut file, &mut out_file)
                .map_err(|e| format!("Failed to write file {}: {}", file_path.display(), e))?;
        }
    }
    // Clean up temp ZIP file
    cleanup_temp_zip(&zip_path);

    log::info!("Successfully unzipped project to: {}", index_path.display());
    let project = CompassProject::load_index_project(project_id)
        .map_err(|e| format!("Failed to load project: {}", e))?;
    Ok(project)
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
