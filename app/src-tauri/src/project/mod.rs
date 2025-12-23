mod map;
mod revision;

pub use map::LocalProject;
pub use revision::SpeleoDbProjectRevision;

use bytes::Bytes;
use common::{
    ApiInfo, Error,
    api_types::{CommitInfo, ProjectInfo},
    compass_project_index_path, compass_project_path, compass_project_working_path,
    ensure_compass_project_dirs_exist,
    ui_state::{LocalProjectStatus, ProjectStatus},
};
use log::info;
use std::{
    fs::{File, copy, create_dir_all, read_dir},
    io::prelude::*,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

pub const SPELEODB_COMPASS_PROJECT_FILE: &str = "compass.toml";

const SPELEODB_PROJECT_REVISION_FILE: &str = ".revision.txt";

// Information about the status of a Compass project.
pub struct ProjectManager {
    project_state: LocalProjectStatus,
    project_info: ProjectInfo,
    index: Option<LocalProject>,
    index_revision: Option<SpeleoDbProjectRevision>,
    working_copy: Option<LocalProject>,
    working_copy_revision: Option<SpeleoDbProjectRevision>,
}

impl ProjectManager {
    pub fn initialize_from_info(project_info: ProjectInfo) -> Self {
        Self {
            project_state: LocalProjectStatus::Unknown,
            project_info,
            index: None,
            index_revision: None,
            working_copy: None,
            working_copy_revision: None,
        }
    }

    pub fn id(&self) -> Uuid {
        self.project_info.id
    }

    pub fn latest_commit(&self) -> Option<&CommitInfo> {
        self.project_info.latest_commit.as_ref()
    }

    pub fn update_project_info(&mut self, project_info: &ProjectInfo) -> Result<(), Error> {
        self.project_info = project_info.clone();
        Ok(())
    }

    pub fn get_ui_status(&self) -> ProjectStatus {
        ProjectStatus::new(self.project_state, self.project_info.clone())
    }

    fn update_project_status(&mut self) -> Result<(), Error> {
        let project_dir = compass_project_path(self.id());
        if !project_dir.exists() {
            self.project_state = LocalProjectStatus::RemoteOnly;
            return Ok(());
        }

        Ok(())
    }

    /// Update the local index of a Compass project by downloading the latest ZIP from SpeleoDB
    async fn update_index(&self, api_info: &ApiInfo) -> Result<LocalProject, String> {
        ensure_compass_project_dirs_exist(self.id()).map_err(|e| e.to_string())?;
        log::info!("Downloading project ZIP from");
        match api::project::download_project_zip(api_info, self.id()).await {
            Ok(bytes) => {
                log::info!("Downloaded ZIP ({} bytes)", bytes.len());
                let project = unpack_project_zip(self.id(), bytes)?;
                if let Some(latest_commit) = self.latest_commit() {
                    SpeleoDbProjectRevision::from(latest_commit)
                        .save_revision_for_project(self.id())
                        .map_err(|e| e.to_string())?;
                };
                // Copy index to working copy
                let src = compass_project_index_path(self.id());
                let dst = compass_project_working_path(self.id());
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
                Ok(LocalProject::empty_project(project_id))
            }
            Err(e) => Err(format!("Failed to download project ZIP: {}", e)),
        }
    }

    /// Pack the working copy of a Compass project into a zip file and return the path to the zip.
    pub fn pack_project_working_copy(project_id: Uuid) -> Result<PathBuf, String> {
        let project = LocalProject::load_working_project(project_id)
            .map_err(|e| format!("Failed to load project: {}", e))?;
        // Create temp zip file
        let temp_dir = std::env::temp_dir();
        let zip_filename = format!("project_{}.zip", project_id);
        let zip_path = temp_dir.join(&zip_filename);
        info!("Creating zip file in temp folder: {zip_path:?}");
        let zip_file = std::fs::File::create(&zip_path)
            .map_err(|e| format!("Failed to create temp zip file: {}", e))?;
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut zip_writer = zip::ZipWriter::new(zip_file);
        zip_writer
            .start_file(SPELEODB_COMPASS_PROJECT_FILE, options)
            .map_err(|e| e.to_string())?;
        let project_toml = toml::to_string_pretty(&project).map_err(|e| e.to_string())?;
        zip_writer
            .write_all(project_toml.as_bytes())
            .map_err(|e| e.to_string())?;
        let project_dir = compass_project_working_path(project.speleodb.id);

        if let Some(mak_file_path) = project.map.mak_file.as_ref() {
            let mak_full_path = project_dir.join(&mak_file_path);
            zip_writer
                .start_file(&mak_file_path, options)
                .map_err(|e| e.to_string())?;
            let mak_contents = std::fs::read(&mak_full_path)
                .map_err(|e| format!("Failed to read MAK file: {}", e))?;
            zip_writer
                .write_all(&mak_contents)
                .map_err(|e| e.to_string())?;
        }

        for dat_path in project.map.dat_files.iter() {
            let dat_full_path = project_dir.join(dat_path);
            zip_writer
                .start_file(&dat_path, options)
                .map_err(|e| e.to_string())?;
            let dat_contents = std::fs::read(&dat_full_path)
                .map_err(|e| format!("Failed to read DAT file: {}", e))?;
            zip_writer
                .write_all(&dat_contents)
                .map_err(|e| e.to_string())?;
        }
        zip_writer.finish().map_err(|e| e.to_string())?;
        Ok(zip_path)
    }
}

// Unpack a project zip directly into the index and return the resulting `CompassProject`.
fn unpack_project_zip(project_id: Uuid, zip_bytes: Bytes) -> Result<LocalProject, String> {
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
    let project = LocalProject::load_index_project(project_id)
        .map_err(|e| format!("Failed to load project: {}", e))?;
    Ok(project)
}

fn cleanup_temp_zip(zip_path: &Path) {
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
