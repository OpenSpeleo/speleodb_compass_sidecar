mod local_project;
mod revision;

pub use local_project::LocalProject;
pub use revision::SpeleoDbProjectRevision;

use crate::paths::{
    compass_project_index_path, compass_project_path, compass_project_working_path,
    ensure_compass_project_dirs_exist,
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
        let project_status = self.local_project_status();
        match project_status {
            LocalProjectStatus::Dirty | LocalProjectStatus::DirtyAndOutOfDate => {
                log::warn!(
                    "Local working copy for project {} has unsaved changes, skipping update",
                    self.id()
                );
                return Ok(ProjectStatus::new(
                    project_status,
                    self.project_info.clone(),
                ));
            }
            _ => { /* continue to update */ }
        }
        if let Some(latest_commit) = self.latest_remote_commit() {
            SpeleoDbProjectRevision::from(latest_commit).save_revision_for_project(self.id())?;
            let current_revision = SpeleoDbProjectRevision::revision_for_local_project(self.id());
            if let Some(working_copy) = self.working_copy() {
            } else {
                log::info!(
                    "No working copy found for project {}, updating index",
                    self.id()
                );
                let _updated_project = self.update_index(api_info).await?;
            }
        };

        Ok(self.project_status())
    }

    pub async fn update_project(&mut self, api_info: &ApiInfo) -> Result<ProjectStatus, Error> {
        let server_info = api::project::fetch_project_info(api_info, self.id()).await?;
        self.update_project_with_server_info(api_info, server_info)
            .await
    }

    fn working_copy(&self) -> Option<LocalProject> {
        match LocalProject::load_working_project(self.id()) {
            Ok(project) => Some(project),
            Err(Error::ProjectNotFound(_)) => None,
            Err(e) => {
                log::error!(
                    "Failed to load working copy for project {}: {}",
                    self.id(),
                    e
                );
                None
            }
        }
    }

    fn index(&self) -> Option<LocalProject> {
        match LocalProject::load_index_project(self.id()) {
            Ok(project) => Some(project),
            Err(Error::ProjectNotFound(_)) => None,
            Err(e) => {
                log::error!("Failed to load index for project {}: {}", self.id(), e);
                None
            }
        }
    }

    /// Local project status determins the state of the local working copy and index.
    /// Assumes that the latest available server info has already been set into the manager's
    /// `project_info` field.
    fn local_project_status(&self) -> LocalProjectStatus {
        let project_dir = compass_project_path(self.id());
        if !project_dir.exists() {
            return LocalProjectStatus::RemoteOnly;
        } else if let Some(working_copy) = self.working_copy() {
            // Check if working copy is empty
            if working_copy.is_empty() {
                return LocalProjectStatus::EmptyLocal;
            }
            // If the working copy isn't empty, then we check for changes
            else if let Some(index) = self.index() {
                let index_revision = SpeleoDbProjectRevision::revision_for_local_project(self.id());
                let latest_server_revision = self.latest_remote_revision();
                // If we have a revision on the server, we have to compare against it
                if let Some(latest_server_revision) = &latest_server_revision {
                    // Compare local index revision to latest server revision
                    if let Some(index_revision) = index_revision {
                        // If the index revision exists, compare it to the latest server revision
                        // and check if working copy is dirty
                        if index_revision.revision == latest_server_revision.revision {
                            // Revisions match, now check if working copy is dirty
                            if working_copy == index {
                                log::info!("Local working copy is up to date and clean");
                                return LocalProjectStatus::UpToDate;
                            } else {
                                log::info!("Local working copy has unsaved changes");
                                return LocalProjectStatus::Dirty;
                            }
                        } else {
                            // Revisions do not match, we're out of date
                            if working_copy == index {
                                log::info!("Local working copy is out of date but clean");
                                return LocalProjectStatus::OutOfDate;
                            } else {
                                log::info!(
                                    "Local working copy is out of date and has unsaved changes"
                                );
                                return LocalProjectStatus::DirtyAndOutOfDate;
                            }
                        }
                    }
                    // If there is no local revision, but a non-empty working copy we're out of date *AND* dirty
                    else {
                        log::info!(
                            "Latest server revision: {}, local revision: None",
                            latest_server_revision.revision
                        );
                        return LocalProjectStatus::DirtyAndOutOfDate;
                    }
                }
                // If we don't have a remote revision, but the local working copy isn't empty, we have local changes
                else {
                    log::info!("Latest server revision: None");
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

    /// Update the local index of a Compass project by downloading the latest ZIP from SpeleoDB
    /// and unpacking it into the index directory.
    /// Returns the updated Ok(Some(`LocalProject`)) if the project has a revision on the server if successful.
    /// Returns Ok(None) if there is no project data on the server.
    async fn update_index(&self, api_info: &ApiInfo) -> Result<Option<LocalProject>, Error> {
        ensure_compass_project_dirs_exist(self.id())?;
        log::info!("Downloading project ZIP from");
        match api::project::download_project_zip(api_info, self.id()).await {
            Ok(bytes) => {
                log::info!("Downloaded ZIP ({} bytes)", bytes.len());
                let project = unpack_project_zip(self.id(), bytes)?;
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
                Ok(Some(project))
            }
            Err(Error::NoProjectData(_)) => {
                log::info!("No project data found on server for project {}", self.id());
                ensure_compass_project_dirs_exist(self.id())?;
                let project = LocalProject::load_index_project(self.id())?;
                Ok(None)
            }
            Err(e) => {
                log::error!("Failed to download project ZIP: {}", e);
                Err(e)
            }
        }
    }
}

// Unpack a project zip directly into the index and return the resulting `CompassProject`.
fn unpack_project_zip(project_id: Uuid, zip_bytes: Bytes) -> Result<LocalProject, Error> {
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
    let project = LocalProject::load_index_project(project_id)?;
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
