mod local_project;
mod revision;

pub use {local_project::LocalProject, revision::SpeleoDbProjectRevision};

use crate::paths::{
    compass_project_index_path, compass_project_path, compass_project_working_path,
    ensure_compass_project_dirs_exist,
};
use bytes::Bytes;
use common::{
    ApiInfo, Error,
    api_types::{CommitInfo, ProjectInfo},
    ui_state::{LocalProjectStatus, ProjectSaveResult, ProjectStatus},
};
use log::{debug, error, info, warn};
use std::{
    fs::{File, copy, create_dir_all, read_dir},
    path::Path,
};
use uuid::Uuid;
use zip::ZipArchive;

pub const SPELEODB_COMPASS_PROJECT_FILE: &str = "compass.toml";
const SPELEODB_PROJECT_REVISION_FILE: &str = ".revision.txt";
const AUTOMATED_PROJECT_CREATION_COMMIT_MESSAGE: &str = "[Automated] Project Creation";

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
        if !self.remote_project_has_compass_data() {
            return None;
        }
        self.latest_remote_commit()
            .map(SpeleoDbProjectRevision::from)
    }

    pub fn local_revision(&self) -> Option<SpeleoDbProjectRevision> {
        SpeleoDbProjectRevision::revision_for_local_project(self.id())
    }

    fn remote_project_has_compass_data(&self) -> bool {
        self.latest_remote_commit().is_some_and(|latest_commit| {
            !(latest_commit.message == AUTOMATED_PROJECT_CREATION_COMMIT_MESSAGE
                && latest_commit.tree.is_empty())
        })
    }

    fn working_copy_is_dirty_or_assume_dirty(&self, context: &str) -> bool {
        match LocalProject::working_copy_is_dirty(self.id()) {
            Ok(is_dirty) => is_dirty,
            Err(e) => {
                error!(
                    "Failed to compute working copy dirty state for project '{}' ({}) while {}. Treating project as dirty to avoid data loss. Error: {}",
                    self.project_info.name,
                    self.id(),
                    context,
                    e
                );
                true
            }
        }
    }

    pub fn project_status(&self) -> ProjectStatus {
        let local_status = self.local_project_status();
        ProjectStatus::new(local_status, self.project_info.clone())
    }

    pub async fn update_project(&mut self, api_info: &ApiInfo) -> Result<ProjectStatus, Error> {
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
                log::info!(
                    "Local working copy for project {} is out of date, updating local copy",
                    self.project_info.name
                );
                self.update_local_copies(api_info).await?;
            }
            _ => {}
        }

        debug!(
            "Project: {} - status: {:?} - locked: {}",
            self.project_info.name,
            self.local_project_status(),
            self.project_info.active_mutex.is_some()
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

    pub async fn save_local_changes(
        &mut self,
        api_info: &ApiInfo,
        commit_message: String,
    ) -> Result<ProjectSaveResult, Error> {
        log::info!(
            "Zipping project folder for project: {}",
            self.project_info.name
        );
        let zip_file = LocalProject::pack_zip(self.id())?;
        let save_result =
            api::project::upload_project_zip(api_info, self.id(), commit_message, &zip_file)
                .await?;
        self.update_project(api_info).await?;
        // Clean up temp zip file regardless of success or failure
        std::fs::remove_file(&zip_file).ok();
        Ok(save_result)
    }

    /// Local project status determins the state of the local working copy and index.
    /// Assumes that the latest available server info has already been set into the manager's
    /// `project_info` field.
    fn local_project_status(&self) -> LocalProjectStatus {
        let project_dir = compass_project_path(self.id());
        if !self.remote_project_has_compass_data() && !project_dir.exists() {
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
                            if self.working_copy_is_dirty_or_assume_dirty(
                                "comparing equal local and remote revisions",
                            ) {
                                return LocalProjectStatus::Dirty;
                            } else {
                                return LocalProjectStatus::UpToDate;
                            }
                        } else {
                            // Revisions do not match, we're out of date
                            if self.working_copy_is_dirty_or_assume_dirty(
                                "comparing different local and remote revisions",
                            ) {
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
        if self.remote_project_has_compass_data() {
            LocalProjectStatus::OutOfDate
        } else {
            LocalProjectStatus::EmptyLocal
        }
    }

    /// Update the local copy of a Compass project by downloading the latest ZIP from SpeleoDB
    /// and unpacking it into both the index directory and working copy.
    /// Returns the updated local project status Ok(LocalProjectStatus::UpToDate) if successful.
    /// Returns Ok(LocalProjectStatus::EmptyLocal) if there is no project data on the server.
    pub async fn update_local_copies(
        &self,
        api_info: &ApiInfo,
    ) -> Result<LocalProjectStatus, Error> {
        ensure_compass_project_dirs_exist(self.id())?;
        log::info!("Downloading project ZIP from");
        match api::project::download_project_zip(api_info, self.id()).await {
            Ok(bytes) => {
                log::info!("Downloaded ZIP ({} bytes)", bytes.len());
                unpack_project_zip(self.id(), bytes)?;
                // Copy index to working copy
                let src = compass_project_index_path(self.id());
                let dst = compass_project_working_path(self.id());
                sync_dir_all(&src, &dst).map_err(|e| {
                    error!(
                        "Failed to copy index to working copy ({} -> {}): {}",
                        src.display(),
                        dst.display(),
                        e
                    );
                    Error::FileWrite(e.to_string())
                })?;
                if let Some(latest_commit) = self.latest_remote_commit() {
                    SpeleoDbProjectRevision::from(latest_commit)
                        .save_revision_for_project(self.id())?;
                } else {
                    warn!(
                        "Downloaded project ZIP for project {} but latest commit metadata is missing; skipping local revision update",
                        self.id()
                    );
                }
                Ok(LocalProjectStatus::UpToDate)
            }
            Err(Error::NoProjectData(_)) => {
                log::error!(
                    "Attempt to update project with no project data found on server for project {}",
                    self.id()
                );
                ensure_compass_project_dirs_exist(self.id())?;
                Ok(LocalProjectStatus::EmptyLocal)
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
    reset_dir(&index_path).map_err(|e| Error::FileWrite(e.to_string()))?;

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
                std::fs::create_dir_all(p).map_err(|_| Error::CreateDirectory(p.to_path_buf()))?;
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
        if let Err(e) = std::fs::remove_file(zip_path) {
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

fn reset_dir(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        std::fs::remove_dir_all(path)?;
    }
    create_dir_all(path)
}

fn sync_dir_all<A: AsRef<Path>>(src: impl AsRef<Path>, dst: A) -> std::io::Result<()> {
    reset_dir(dst.as_ref())?;
    copy_dir_all(src, dst)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{compass_project_index_path, compass_project_path};
    use common::api_types::{CommitInfo, CommitTreeEntry, ProjectInfo, ProjectType};
    use std::io::Write;

    fn cleanup_project_dir(id: Uuid) {
        let _ = std::fs::remove_dir_all(compass_project_path(id));
    }

    fn test_project_info(id: Uuid, latest_commit: Option<CommitInfo>) -> ProjectInfo {
        ProjectInfo {
            id,
            name: "Test Project".to_string(),
            description: "Test Description".to_string(),
            is_active: true,
            permission: "ADMIN".to_string(),
            active_mutex: None,
            country: "US".to_string(),
            created_by: "tester@example.com".to_string(),
            creation_date: "2026-01-01T00:00:00Z".to_string(),
            modified_date: "2026-01-01T00:00:00Z".to_string(),
            latitude: None,
            longitude: None,
            fork_from: None,
            visibility: "PRIVATE".to_string(),
            exclude_geojson: false,
            latest_commit,
            project_type: ProjectType::Compass,
        }
    }

    fn test_commit(message: &str, tree_entries: usize) -> CommitInfo {
        CommitInfo {
            id: "abc123".to_string(),
            message: message.to_string(),
            author_name: "SpeleoDB".to_string(),
            dt_since: "now".to_string(),
            tree: vec![CommitTreeEntry {}; tree_entries],
        }
    }

    #[test]
    fn test_local_project_status_empty_for_automated_project_creation_with_empty_tree() {
        let project_id = Uuid::new_v4();
        cleanup_project_dir(project_id);

        let manager = ProjectManager::initialize_from_info(test_project_info(
            project_id,
            Some(test_commit(AUTOMATED_PROJECT_CREATION_COMMIT_MESSAGE, 0)),
        ));

        assert_eq!(
            manager.local_project_status(),
            LocalProjectStatus::EmptyLocal
        );
        cleanup_project_dir(project_id);
    }

    #[test]
    fn test_local_project_status_not_empty_for_automated_project_creation_with_files() {
        let project_id = Uuid::new_v4();
        cleanup_project_dir(project_id);

        let manager = ProjectManager::initialize_from_info(test_project_info(
            project_id,
            Some(test_commit(AUTOMATED_PROJECT_CREATION_COMMIT_MESSAGE, 1)),
        ));

        assert_eq!(
            manager.local_project_status(),
            LocalProjectStatus::RemoteOnly
        );
        cleanup_project_dir(project_id);
    }

    #[test]
    fn test_sync_dir_all_replaces_destination_contents() {
        let temp_root = std::env::temp_dir().join(format!("speleodb_sync_test_{}", Uuid::new_v4()));
        let src = temp_root.join("src");
        let dst = temp_root.join("dst");
        std::fs::create_dir_all(&src).expect("src dir should exist");
        std::fs::create_dir_all(&dst).expect("dst dir should exist");
        std::fs::write(src.join("fresh.txt"), "fresh").expect("fresh file should be created");
        std::fs::write(dst.join("stale.txt"), "stale").expect("stale file should be created");

        sync_dir_all(&src, &dst).expect("sync should succeed");

        assert!(
            dst.join("fresh.txt").exists(),
            "fresh file should be copied"
        );
        assert!(
            !dst.join("stale.txt").exists(),
            "stale destination file should be removed"
        );

        let _ = std::fs::remove_dir_all(temp_root);
    }

    #[test]
    fn test_dirty_check_error_assumes_dirty_instead_of_panicking() {
        let project_id = Uuid::new_v4();
        cleanup_project_dir(project_id);

        // Both index and working_copy get identical compass.toml referencing
        // a .mak file that does NOT exist on disk. This makes
        // working_copy_is_dirty() return Err (cannot load compass project).
        let compass_toml = format!(
            "[speleodb]\nid = \"{project_id}\"\nversion = \"1.0.0\"\n\n\
             [project]\nmak_file = \"ghost.mak\"\ndat_files = []\nplt_files = []\n"
        );
        let index_path = compass_project_index_path(project_id);
        let working_path = compass_project_working_path(project_id);
        std::fs::create_dir_all(&index_path).expect("index dir");
        std::fs::create_dir_all(&working_path).expect("working dir");
        std::fs::write(
            index_path.join(SPELEODB_COMPASS_PROJECT_FILE),
            &compass_toml,
        )
        .expect("index compass.toml");
        std::fs::write(
            working_path.join(SPELEODB_COMPASS_PROJECT_FILE),
            &compass_toml,
        )
        .expect("working compass.toml");

        // Revision matching the server commit so we reach the dirty check
        SpeleoDbProjectRevision {
            revision: "abc123".to_string(),
        }
        .save_revision_for_project(project_id)
        .expect("revision file");

        let manager = ProjectManager::initialize_from_info(test_project_info(
            project_id,
            Some(test_commit("Test commit", 1)),
        ));

        // Before the fix this would panic via .unwrap(); now it should
        // fall back to Dirty (safe default to avoid data loss).
        assert_eq!(
            manager.local_project_status(),
            LocalProjectStatus::Dirty,
            "should assume dirty when dirty check errors"
        );

        cleanup_project_dir(project_id);
    }

    #[test]
    fn test_unpack_project_zip_clears_stale_index_files() {
        let project_id = Uuid::new_v4();
        cleanup_project_dir(project_id);
        let index_path = compass_project_index_path(project_id);
        std::fs::create_dir_all(&index_path).expect("index dir should be created");
        std::fs::write(index_path.join("stale.dat"), "stale")
            .expect("stale file should be created");

        let mut cursor = std::io::Cursor::new(Vec::<u8>::new());
        {
            let mut zip_writer = zip::ZipWriter::new(&mut cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated);
            zip_writer
                .start_file(SPELEODB_COMPASS_PROJECT_FILE, options)
                .expect("zip file should start");
            zip_writer
                .write_all(b"project = 'data'")
                .expect("zip write should succeed");
            zip_writer.finish().expect("zip finish should succeed");
        }
        let zip_bytes = bytes::Bytes::from(cursor.into_inner());

        unpack_project_zip(project_id, zip_bytes).expect("unpack should succeed");

        assert!(
            !index_path.join("stale.dat").exists(),
            "stale file should be removed when unpacking"
        );
        assert!(
            index_path.join(SPELEODB_COMPASS_PROJECT_FILE).exists(),
            "new compass.toml should exist after unpack"
        );

        cleanup_project_dir(project_id);
    }
}
