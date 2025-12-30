mod local_project;
mod revision;

pub use revision::SpeleoDbProjectRevision;

use crate::{
    paths::{
        compass_project_index_path, compass_project_path, compass_project_working_path,
        ensure_compass_project_dirs_exist,
    },
    project_management::local_project::LocalProject,
};
use bytes::Bytes;
use common::{
    ApiInfo, Error,
    api_types::{CommitInfo, ProjectInfo},
    ui_state::{LocalProjectStatus, ProjectStatus},
};
use log::{error, info};
use std::{
    fs::{File, copy, create_dir_all, read_dir},
    path::Path,
};
use uuid::Uuid;
use zip::ZipArchive;

pub const SPELEODB_COMPASS_PROJECT_FILE: &str = "compass.toml";
const SPELEODB_PROJECT_REVISION_FILE: &str = ".revision.txt";

// Information about the status of a Compass project.
#[derive(Clone, Debug)]
pub struct ProjectManager {
    project_info: ProjectInfo,
}

impl ProjectManager {
    /// Initialize a `ProjectManager` from `ProjectInfo`.
    pub fn initialize_from_info(project_info: ProjectInfo) -> Self {
        Self { project_info }
    }

    /// Get the project ID.
    pub fn id(&self) -> Uuid {
        self.project_info.id
    }

    /// Get the latest commit info, if available.
    pub fn latest_remote_commit(&self) -> Option<&CommitInfo> {
        self.project_info.latest_commit.as_ref()
    }

    /// Get the latest remote revision as a `SpeleoDbProjectRevision`, if available.
    pub fn latest_remote_revision(&self) -> Option<SpeleoDbProjectRevision> {
        self.latest_remote_commit()
            .map(|commit| SpeleoDbProjectRevision::from(commit))
    }

    pub fn local_revision(&self) -> Option<SpeleoDbProjectRevision> {
        SpeleoDbProjectRevision::revision_for_local_project(self.id())
    }

    pub fn project_status(&self) -> ProjectStatus {
        let local_status = self.local_project_status();
        ProjectStatus::new(local_status, self.project_info.clone())
    }

    pub async fn update_project_with_server_info(
        &mut self,
        api_info: &ApiInfo,
        server_info: ProjectInfo,
    ) -> Result<ProjectStatus, Error> {
        self.project_info = server_info;
        // Check local project status, and update if clean and out of date
        let project_status = self.local_project_status();
        match project_status {
            LocalProjectStatus::Dirty | LocalProjectStatus::DirtyAndOutOfDate => {
                log::warn!(
                    "Local working copy for project {} has unsaved changes, skipping update",
                    self.project_info.name
                );
                return Ok(ProjectStatus::new(
                    project_status,
                    self.project_info.clone(),
                ));
            }
            LocalProjectStatus::OutOfDate => {
                // Spawn async task to update local copies
                let project_id = self.id();
                let api_info = api_info.clone();
                let manager = self.clone();
                tauri::async_runtime::spawn(async move {
                    match manager.update_local_copies(&api_info).await {
                        Ok(_) => {
                            log::info!("Successfully updated index for project {}", project_id);
                        }
                        Err(e) => {
                            log::error!("Failed to update index for project {}: {}", project_id, e);
                        }
                    }
                });
            }
            _ => {}
        }

        info!(
            "Project: {} - status: {:?} ",
            self.project_info.name,
            self.local_project_status()
        );
        Ok(self.project_status())
    }

    pub async fn make_local(&self, api_info: &ApiInfo) -> Result<(), Error> {
        let project_status = self.local_project_status();
        if let LocalProjectStatus::RemoteOnly = project_status {
            info!(
                "Making local copy of remote project: {}",
                self.project_info.name
            );
            self.update_local_copies(api_info).await?;
            Ok(())
        } else {
            // Project is already local, nothing to do
            Ok(())
        }
    }

    /// Local project status determins the state of the local working copy and index.
    /// Assumes that the latest available server info has already been set into the manager's
    /// `project_info` field.
    fn local_project_status(&self) -> LocalProjectStatus {
        let project_dir = compass_project_path(self.id());
        if self.latest_remote_commit().is_none() && !project_dir.exists() {
            return LocalProjectStatus::EmptyLocal;
        } else if !project_dir.exists() {
            return LocalProjectStatus::RemoteOnly;
        } else if LocalProject::working_copy_exists(self.id()) {
            if LocalProject::index_exists(self.id()) {
                let index_revision = self.local_revision();
                let latest_server_revision = self.latest_remote_revision();
                // If we have a revision on the server, we have to compare against it
                if let Some(latest_server_revision) = &latest_server_revision {
                    // Compare local index revision to latest server revision
                    if let Some(index_revision) = index_revision {
                        // If the index revision exists, compare it to the latest server revision
                        // and check if working copy is dirty
                        if index_revision.revision == latest_server_revision.revision {
                            // Revisions match, now check if working copy is dirty
                            if LocalProject::working_copy_is_dirty(self.id()).unwrap() {
                                return LocalProjectStatus::Dirty;
                            } else {
                                return LocalProjectStatus::UpToDate;
                            }
                        } else {
                            // Revisions do not match, we're out of date
                            if LocalProject::working_copy_is_dirty(self.id()).unwrap() {
                                return LocalProjectStatus::DirtyAndOutOfDate;
                            } else {
                                return LocalProjectStatus::OutOfDate;
                            }
                        }
                    }
                    // If there is no local revision, but a non-empty working copy we're out of date *AND* dirty
                    else {
                        return LocalProjectStatus::DirtyAndOutOfDate;
                    }
                }
                // If we don't have a remote revision, but the local working copy isn't empty, we have local changes
                else {
                    return LocalProjectStatus::Dirty;
                }
            }
            // If there is no index, then the working copy must be
            else {
                return LocalProjectStatus::Dirty;
            }
        }
        // If there is no working copy, but the project directory exists, it must be empty
        // If there's a remote version we're  out of date, otherwise it's just a newly created empty local
        if self.latest_remote_commit().is_some() {
            LocalProjectStatus::OutOfDate
        } else {
            LocalProjectStatus::EmptyLocal
        }
    }

    /// Update the local copy of a Compass project by downloading the latest ZIP from SpeleoDB
    /// and unpacking it into both the index directory and working copy.
    /// Returns the updated Ok(Some(`LocalProject`)) if the project has a revision on the server if successful.
    /// Returns Ok(None) if there is no project data on the server.
    async fn update_local_copies(&self, api_info: &ApiInfo) -> Result<(), Error> {
        ensure_compass_project_dirs_exist(self.id())?;
        log::info!("Downloading project ZIP from");
        match api::project::download_project_zip(api_info, self.id()).await {
            Ok(bytes) => {
                log::info!("Downloaded ZIP ({} bytes)", bytes.len());
                unpack_project_zip(self.id(), bytes)?;
                if let Some(latest_commit) = self.latest_remote_commit() {
                    SpeleoDbProjectRevision::from(latest_commit)
                        .save_revision_for_project(self.id())?;
                };
                // Copy index to working copy
                let src = compass_project_index_path(self.id());
                let dst = compass_project_working_path(self.id());
                copy_dir_all(&src, &dst).map_err(|e| {
                    error!(
                        "Failed to copy index to working copy ({} -> {}): {}",
                        src.display(),
                        dst.display(),
                        e
                    );
                    Error::FileWrite(e.to_string())
                })?;
                SpeleoDbProjectRevision::from(self.project_info.latest_commit.as_ref().unwrap())
                    .save_revision_for_project(self.id())?;
                Ok(())
            }
            Err(Error::NoProjectData(_)) => {
                log::info!("No project data found on server for project {}", self.id());
                ensure_compass_project_dirs_exist(self.id())?;
                Ok(())
            }
            Err(e) => {
                log::error!("Failed to download project ZIP: {}", e);
                Err(e)
            }
        }
    }
}

// Unpack a project zip directly into the index and return the resulting `CompassProject`.
fn unpack_project_zip(project_id: Uuid, zip_bytes: Bytes) -> Result<(), Error> {
    // Create temp zip file
    let temp_dir = std::env::temp_dir();
    let zip_filename = format!("project_{}.zip", project_id);
    let zip_path = temp_dir.join(&zip_filename);
    info!("Creating zip file in temp folder: {zip_path:?}");
    std::fs::write(&zip_path, &zip_bytes).map_err(|e| Error::FileWrite(e.to_string()))?;
    // Unzip the project
    let file = std::fs::File::open(&zip_path).map_err(|e| Error::FileRead(e.to_string()))?;
    let mut archive = ZipArchive::new(file).map_err(|e| Error::ZipFile(e.to_string()))?;

    let index_path = compass_project_index_path(project_id);

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| Error::ZipFile(e.to_string()))?;

        let file_path = match file.enclosed_name() {
            Some(path) => index_path.join(path),
            None => continue,
        };
        if file.is_dir() {
            // Ignore, we automatically create directories for files as needed below
        } else {
            // Create parent directories if they don't exist
            if let Some(p) = file_path.parent() {
                std::fs::create_dir_all(p).map_err(|e| Error::CreateDirectory(p.to_path_buf()))?;
            }
            let mut out_file =
                File::create(&file_path).map_err(|e| Error::FileWrite(e.to_string()))?;

            std::io::copy(&mut file, &mut out_file).map_err(|e| Error::FileWrite(e.to_string()))?;
        }
    }
    // Clean up temp ZIP file
    cleanup_temp_zip(&zip_path);

    log::info!("Successfully unzipped project to: {}", index_path.display());
    Ok(())
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
