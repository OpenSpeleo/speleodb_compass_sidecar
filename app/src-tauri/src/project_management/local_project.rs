//! Module for managing local Compass projects stored on disk.
//! This includes loading, saving, importing, and packing projects.
//! A local project consists of an index copy and a working copy.
//! The index copy represents the last known state of the project as stored in SpeleoDB,
//! while the working copy represents the current state of the project on disk.

use crate::{
    SPELEODB_COMPASS_VERSION,
    paths::{compass_project_index_path, compass_project_working_path},
    project_management::SPELEODB_COMPASS_PROJECT_FILE,
};
use common::Error;
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::{
    io::prelude::*,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpeleoDb {
    pub id: Uuid,
    pub version: semver::Version,
}

impl Default for SpeleoDb {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            version: SPELEODB_COMPASS_VERSION,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectMap {
    pub mak_file: Option<String>,
    pub dat_files: Vec<String>,
    pub plt_files: Vec<String>,
}

impl ProjectMap {
    pub fn import(mak_file: String, dat_files: Vec<String>) -> Self {
        let plt_files = vec![];
        Self {
            mak_file: Some(mak_file),
            dat_files,
            plt_files,
        }
    }

    pub fn tracked_file_paths(&self) -> Vec<&str> {
        let mut paths = Vec::new();
        if let Some(mak) = &self.mak_file {
            paths.push(mak.as_str());
        }
        paths.extend(self.dat_files.iter().map(String::as_str));
        paths
    }
}

/// Represents a local Compass project stored on disk.
/// Note that this struct does not contain the actual survey data files,
/// but rather metadata about the project and references to the data files.
/// This struct is only ever serialized/deserialized to/from the SPELEODB_COMPASS_PROJECT_FILE file, and
/// not created or accessed anywhere outside of this file.
/// Instead, associated functions provide access to the work with the data on disk
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalProject {
    speleodb: SpeleoDb,
    #[serde(rename = "project")]
    project_map: ProjectMap,
}

impl LocalProject {
    fn is_compass_artifact(path: &Path) -> bool {
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.eq_ignore_ascii_case(SPELEODB_COMPASS_PROJECT_FILE))
        {
            return true;
        }

        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "mak" | "dat" | "plt"))
    }

    fn clear_compass_artifacts_from_dir(path: &Path) -> Result<(), Error> {
        if !path.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(path).map_err(|e| Error::FileRead(e.to_string()))?;
        for entry in entries {
            let entry = entry.map_err(|e| Error::FileRead(e.to_string()))?;
            let entry_path = entry.path();
            let file_type = entry
                .file_type()
                .map_err(|e| Error::FileRead(e.to_string()))?;
            if file_type.is_dir() {
                Self::clear_compass_artifacts_from_dir(&entry_path)?;
                continue;
            }
            if Self::is_compass_artifact(&entry_path) {
                std::fs::remove_file(&entry_path).map_err(|e| Error::FileWrite(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Remove Compass project artifacts from the working copy before re-importing.
    pub fn clear_working_copy_compass_artifacts(id: Uuid) -> Result<(), Error> {
        let working_copy_path = compass_project_working_path(id);
        Self::clear_compass_artifacts_from_dir(&working_copy_path)
    }

    fn copy_import_file(source_path: &Path, target_path: &Path) -> Result<(), Error> {
        std::fs::copy(source_path, target_path).map_err(|e| {
            let is_permission_error = e.kind() == std::io::ErrorKind::PermissionDenied;
            let details = format!(
                "{e} (kind: {:?}, raw_os_error: {:?})",
                e.kind(),
                e.raw_os_error()
            );
            error!(
                "Failed to copy {} -> {}: {}",
                source_path.display(),
                target_path.display(),
                details
            );
            Error::ProjectImport {
                src_path: source_path.to_path_buf(),
                dst_path: target_path.to_path_buf(),
                details,
                is_permission_error,
            }
        })?;
        Ok(())
    }

    pub fn working_copy_is_dirty(id: Uuid) -> Result<bool, Error> {
        let index_copy = LocalProject::load_index_project(id).ok();
        let working_copy = LocalProject::load_working_project(id).ok();

        match (index_copy, working_copy) {
            (Some(index), Some(working)) => {
                if index != working {
                    return Ok(true);
                }
                let index_root = compass_project_index_path(id);
                let working_root = compass_project_working_path(id);
                for relative_path in index.project_map.tracked_file_paths() {
                    let index_bytes = std::fs::read(index_root.join(relative_path));
                    let working_bytes = std::fs::read(working_root.join(relative_path));
                    match (index_bytes, working_bytes) {
                        (Ok(ib), Ok(wb)) if ib != wb => return Ok(true),
                        (Ok(_), Ok(_)) => {}
                        (Ok(_), Err(e)) | (Err(e), Ok(_))
                            if e.kind() == std::io::ErrorKind::NotFound =>
                        {
                            return Ok(true);
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            return Err(Error::FileRead(format!("{relative_path}: {e}")));
                        }
                    }
                }
                Ok(false)
            }
            (Some(_), None) => Ok(false),
            (None, Some(_)) => Ok(true),
            (None, None) => Ok(false),
        }
    }

    /// Import a Compass project from a .mak file into the local working copy.
    pub fn import_compass_project(id: Uuid, mak_path: &Path) -> Result<(), Error> {
        info!("Attempting to import {mak_path:?} to project {id}");
        // Verify that the .mak file exists
        let mak_path = std::path::PathBuf::from(mak_path);
        if !mak_path.exists() {
            return Err(Error::ProjectNotFound(mak_path));
        }
        // Make sure it's not at a weird path
        let mak_dir = mak_path
            .parent()
            .ok_or_else(|| Error::ProjectNotFound(mak_path.clone()))?;
        // Load and parse the compass project file
        let compass_project = compass_data::Project::read(&mak_path).map_err(|e| {
            error!("Error loading compass .mak file: {e}");
            Error::Deserialization(e.to_string())
        })?;
        info!("Project parsed successfully");
        // Verify that all referenced survey files exist
        let mut not_found = None;
        let mut project_file_paths = vec![];
        let mut project_files = vec![];
        compass_project.survey_files.iter().for_each(|f| {
            info!("Verifying referenced survey file: {f:?}");
            let mut file_path = mak_dir.to_path_buf();
            file_path.push(&f.file_path);
            if !file_path.exists() {
                error!("Referenced survey file not found: {}", file_path.display());
                not_found = Some(file_path);
            } else {
                project_file_paths.push(file_path);
                project_files.push(f.file_path.to_string_lossy().to_string());
            }
        });
        if let Some(path) = not_found {
            error!("Failed to find referenced project file: {path:?}");
            return Err(Error::ProjectFileNotFound(path));
        }

        // Everything looks good, create the new CompassProject
        let mut project_path = compass_project_working_path(id);

        std::fs::create_dir_all(&project_path).map_err(|e| {
            error!(
                "Failed to create working copy directory during import: {} (error: {})",
                project_path.display(),
                e
            );
            Error::CreateDirectory(project_path.clone())
        })?;
        let new_project = Self {
            speleodb: SpeleoDb {
                id,
                version: SPELEODB_COMPASS_VERSION,
            },
            project_map: ProjectMap::import(
                mak_path.file_name().unwrap().to_string_lossy().to_string(),
                project_files.clone(),
            ),
        };
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let serialized_project = toml::to_string_pretty(&new_project)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        std::fs::write(&project_path, &serialized_project).map_err(|e| {
            error!(
                "Failed to write Compass metadata file during import: {} (error: {})",
                project_path.display(),
                e
            );
            Error::ProjectWrite(project_path.clone())
        })?;
        // Copy the .mak file and all referenced survey files into the new project directory
        let mut mak_target_path = compass_project_working_path(id);
        mak_target_path.push(mak_path.file_name().unwrap());
        Self::copy_import_file(&mak_path, &mak_target_path)?;
        info!(
            "Copying {} referenced survey files for project {}",
            project_file_paths.len(),
            id
        );
        for (file_path, relative_path) in project_file_paths.iter().zip(project_files.iter()) {
            let mut target_path = compass_project_working_path(id);
            target_path.push(relative_path);
            if let Some(parent_dir) = target_path.parent() {
                std::fs::create_dir_all(parent_dir).map_err(|e| {
                    error!(
                        "Failed to create target directory for imported file: {} (error: {})",
                        parent_dir.display(),
                        e
                    );
                    Error::CreateDirectory(parent_dir.to_path_buf())
                })?;
            }
            Self::copy_import_file(file_path, &target_path)?;
        }
        Ok(())
    }
    #[cfg(target_os = "windows")]
    pub fn mak_file_path(id: Uuid) -> Result<PathBuf, Error> {
        let local_project = LocalProject::load_working_project(id)?;
        let mak_file_name = local_project
            .project_map
            .mak_file
            .ok_or(Error::NoProjectData(id))?;
        let mut mak_path = compass_project_working_path(id);
        mak_path.push(&mak_file_name);
        Ok(mak_path)
    }

    fn load_working_project(id: Uuid) -> Result<Self, Error> {
        let mut project_path = compass_project_working_path(id);
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let project_data = std::fs::read_to_string(&project_path)
            .map_err(|_| Error::ProjectNotFound(project_path.clone()))?;
        let project: LocalProject =
            toml::from_str(&project_data).map_err(|e| Error::Deserialization(e.to_string()))?;
        Ok(project)
    }

    fn load_index_project(id: Uuid) -> Result<Self, Error> {
        let mut project_path = compass_project_index_path(id);
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let project_data = std::fs::read_to_string(&project_path)
            .map_err(|_| Error::ProjectNotFound(project_path.clone()))?;
        let project: LocalProject =
            toml::from_str(&project_data).map_err(|e| Error::Deserialization(e.to_string()))?;
        Ok(project)
    }

    pub fn working_copy_exists(id: Uuid) -> bool {
        match LocalProject::load_working_project(id) {
            Ok(working_copy) => working_copy.project_map.mak_file.is_some(),
            Err(Error::ProjectNotFound(_)) => false,
            Err(e) => panic!("Error checking working copy existence: {}", e),
        }
    }

    pub fn index_exists(id: Uuid) -> bool {
        match LocalProject::load_index_project(id) {
            Ok(index) => index.project_map.mak_file.is_some(),
            Err(Error::ProjectNotFound(_)) => false,
            Err(e) => panic!("Error checking index existence: {}", e),
        }
    }

    /// Pack the working copy of a Compass project into a zip file and return the path to the zip.
    pub fn pack_zip(id: Uuid) -> Result<PathBuf, Error> {
        let working_copy = LocalProject::load_working_project(id)?;
        // Create temp zip file
        let temp_dir = std::env::temp_dir();
        let zip_filename = format!("project_{}.zip", id);
        let zip_path = temp_dir.join(&zip_filename);
        info!("Creating zip file in temp folder: {zip_path:?}");
        let zip_file =
            std::fs::File::create(&zip_path).map_err(|_| Error::ProjectWrite(zip_path.clone()))?;
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        let mut zip_writer = zip::ZipWriter::new(zip_file);
        zip_writer
            .start_file(SPELEODB_COMPASS_PROJECT_FILE, options)
            .map_err(|e| Error::ZipFile(e.to_string()))?;
        let project_toml = toml::to_string_pretty(&working_copy)
            .map_err(|e| Error::Serialization(e.to_string()))?;
        zip_writer
            .write_all(project_toml.as_bytes())
            .map_err(|e| Error::ZipFile(e.to_string()))?;
        let project_dir = compass_project_working_path(id);

        for relative_path in working_copy.project_map.tracked_file_paths() {
            let full_path = project_dir.join(relative_path);
            zip_writer
                .start_file(relative_path, options)
                .map_err(|e| Error::ZipFile(e.to_string()))?;
            let contents = std::fs::read(&full_path).map_err(|e| Error::FileRead(e.to_string()))?;
            zip_writer
                .write_all(&contents)
                .map_err(|e| Error::ZipFile(e.to_string()))?;
        }

        zip_writer
            .finish()
            .map_err(|e| Error::ZipFile(e.to_string()))?;
        Ok(zip_path)
    }
}

#[cfg(test)]
mod test {
    #[derive(Clone, Debug, PartialEq)]
    enum ValidationIssue {
        MakParseError(String),
        UntrackedDatFile(String),
        OrphanedDatFile(String),
        MissingDatFile(String),
        MissingMakFile(String),
        NoProjectMetadata,
    }

    #[derive(Clone, Debug, PartialEq)]
    struct ValidationReport {
        issues: Vec<ValidationIssue>,
    }

    impl ValidationReport {
        fn is_valid(&self) -> bool {
            self.issues.is_empty()
        }
    }

    fn validate_working_copy(id: Uuid) -> ValidationReport {
        let mut issues = Vec::new();

        let local_project = match LocalProject::load_working_project(id) {
            Ok(p) => p,
            Err(_) => {
                issues.push(ValidationIssue::NoProjectMetadata);
                return ValidationReport { issues };
            }
        };

        let working_root = compass_project_working_path(id);

        let mak_file = match &local_project.project_map.mak_file {
            Some(m) => m.clone(),
            None => return ValidationReport { issues },
        };

        let mak_path = working_root.join(&mak_file);
        if !mak_path.exists() {
            issues.push(ValidationIssue::MissingMakFile(mak_file));
            return ValidationReport { issues };
        }

        let mak_dat_files: Vec<String> = match compass_data::Project::read(&mak_path) {
            Ok(project) => project
                .survey_files
                .iter()
                .map(|f| f.file_path.to_string_lossy().to_string())
                .collect(),
            Err(e) => {
                issues.push(ValidationIssue::MakParseError(e.to_string()));
                return ValidationReport { issues };
            }
        };

        let tracked_dats: std::collections::HashSet<&str> = local_project
            .project_map
            .dat_files
            .iter()
            .map(String::as_str)
            .collect();

        let mak_dats: std::collections::HashSet<&str> =
            mak_dat_files.iter().map(String::as_str).collect();

        for dat in &mak_dats {
            if !tracked_dats.contains(dat) {
                issues.push(ValidationIssue::UntrackedDatFile(dat.to_string()));
            }
        }

        for dat in &tracked_dats {
            if !mak_dats.contains(dat) {
                issues.push(ValidationIssue::OrphanedDatFile(dat.to_string()));
            }
        }

        for dat in tracked_dats.union(&mak_dats) {
            if !working_root.join(dat).exists() {
                issues.push(ValidationIssue::MissingDatFile(dat.to_string()));
            }
        }

        ValidationReport { issues }
    }
    use super::*;
    use crate::paths::{
        compass_project_index_path, compass_project_path, compass_project_working_path,
    };
    use serial_test::serial;
    use std::path::{Path, PathBuf};

    fn fixture_path(file_name: &str) -> PathBuf {
        PathBuf::from(format!(
            "{}/assets/test_data/{}",
            env!("CARGO_MANIFEST_DIR"),
            file_name
        ))
    }

    fn cleanup_project_dir(id: Uuid) {
        let _ = std::fs::remove_dir_all(compass_project_path(id));
    }

    fn setup_import_source(id: Uuid, with_dat_files: bool) -> PathBuf {
        let source_dir = std::env::temp_dir().join(format!("speleodb_import_source_{id}"));
        let _ = std::fs::remove_dir_all(&source_dir);
        std::fs::create_dir_all(&source_dir).expect("source dir should be created");

        std::fs::copy(
            fixture_path("Fulfords.mak"),
            source_dir.join("Fulfords.mak"),
        )
        .expect("test mak should be copied");

        if with_dat_files {
            std::fs::copy(fixture_path("Fulford.dat"), source_dir.join("FULFORD.DAT"))
                .expect("FULFORD.DAT should be copied");
            std::fs::copy(fixture_path("Fulsurf.dat"), source_dir.join("FULSURF.DAT"))
                .expect("FULSURF.DAT should be copied");
        }

        source_dir
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) {
        std::fs::create_dir_all(dst).expect("destination dir should exist");
        for entry in std::fs::read_dir(src).expect("source dir should be readable") {
            let entry = entry.expect("directory entry should be readable");
            let entry_type = entry.file_type().expect("file type should be available");
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if entry_type.is_dir() {
                copy_dir_recursive(&src_path, &dst_path);
            } else {
                std::fs::copy(src_path, dst_path).expect("file should copy");
            }
        }
    }

    fn setup_synced_index_and_working_copy(id: Uuid) -> PathBuf {
        cleanup_project_dir(id);
        let source_dir = setup_import_source(id, true);
        let mak_path = source_dir.join("Fulfords.mak");
        LocalProject::import_compass_project(id, &mak_path).expect("import should succeed");

        let working_copy = compass_project_working_path(id);
        let index_copy = compass_project_index_path(id);
        let _ = std::fs::remove_dir_all(&index_copy);
        copy_dir_recursive(&working_copy, &index_copy);
        source_dir
    }

    #[test]
    #[serial]
    fn test_project_import() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let source_dir = setup_import_source(id, true);
        let mak_path = source_dir.join("Fulfords.mak");

        let result = LocalProject::import_compass_project(id, &mak_path);

        assert!(result.is_ok(), "import should succeed for valid fixture");
        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_clear_working_copy_compass_artifacts_removes_only_compass_files() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let working_copy = compass_project_working_path(id);
        std::fs::create_dir_all(working_copy.join("nested"))
            .expect("nested path should be created");

        std::fs::write(working_copy.join("legacy.mak"), "mak").expect("legacy mak created");
        std::fs::write(working_copy.join("legacy.dat"), "dat").expect("legacy dat created");
        std::fs::write(working_copy.join("legacy.plt"), "plt").expect("legacy plt created");
        std::fs::write(working_copy.join(SPELEODB_COMPASS_PROJECT_FILE), "project")
            .expect("compass.toml created");
        std::fs::write(working_copy.join("notes.txt"), "notes").expect("notes created");
        std::fs::write(working_copy.join("nested/keep.md"), "keep").expect("keep created");
        std::fs::write(working_copy.join("nested/legacy.dat"), "nested dat")
            .expect("nested dat created");

        LocalProject::clear_working_copy_compass_artifacts(id).expect("cleanup should succeed");

        assert!(
            !working_copy.join("legacy.mak").exists(),
            "mak files should be removed"
        );
        assert!(
            !working_copy.join("legacy.dat").exists(),
            "dat files should be removed"
        );
        assert!(
            !working_copy.join("legacy.plt").exists(),
            "plt files should be removed"
        );
        assert!(
            !working_copy.join(SPELEODB_COMPASS_PROJECT_FILE).exists(),
            "compass.toml should be removed"
        );
        assert!(
            !working_copy.join("nested/legacy.dat").exists(),
            "nested dat files should be removed"
        );
        assert!(
            working_copy.join("notes.txt").exists(),
            "non-compass files should be kept"
        );
        assert!(
            working_copy.join("nested/keep.md").exists(),
            "non-compass nested files should be kept"
        );

        cleanup_project_dir(id);
    }

    #[test]
    #[serial]
    fn test_clear_working_copy_compass_artifacts_is_idempotent() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);

        LocalProject::clear_working_copy_compass_artifacts(id)
            .expect("cleanup should succeed for missing dir");

        let working_copy = compass_project_working_path(id);
        std::fs::create_dir_all(&working_copy).expect("working copy should be created");
        std::fs::write(working_copy.join("keep.txt"), "keep").expect("keep file should be created");

        LocalProject::clear_working_copy_compass_artifacts(id).expect("first cleanup succeeds");
        LocalProject::clear_working_copy_compass_artifacts(id).expect("second cleanup succeeds");

        assert!(
            working_copy.join("keep.txt").exists(),
            "cleanup should not remove unrelated files"
        );

        cleanup_project_dir(id);
    }

    #[test]
    #[serial]
    fn test_reimport_after_cleanup_writes_new_compass_toml() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);

        let working_copy = compass_project_working_path(id);
        std::fs::create_dir_all(&working_copy).expect("working copy should be created");
        std::fs::write(working_copy.join("old.mak"), "old").expect("old mak should be created");
        std::fs::write(working_copy.join("old.dat"), "old").expect("old dat should be created");
        std::fs::write(
            working_copy.join(SPELEODB_COMPASS_PROJECT_FILE),
            "stale-compass-toml",
        )
        .expect("stale compass.toml should be created");

        LocalProject::clear_working_copy_compass_artifacts(id).expect("cleanup should succeed");

        let source_dir = setup_import_source(id, true);
        let mak_path = source_dir.join("Fulfords.mak");
        LocalProject::import_compass_project(id, &mak_path).expect("reimport should succeed");

        let compass_toml_path = working_copy.join(SPELEODB_COMPASS_PROJECT_FILE);
        let compass_toml = std::fs::read_to_string(&compass_toml_path)
            .expect("new compass.toml should be written");

        assert!(
            compass_toml.contains("FULFORD.DAT"),
            "compass.toml should reference imported survey files"
        );
        assert!(
            !working_copy.join("old.mak").exists(),
            "cleanup should remove stale mak before import"
        );

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_copy_import_file_includes_io_error_details() {
        let temp_dir =
            std::env::temp_dir().join(format!("speleodb_copy_import_{:?}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir should be created");
        let source = temp_dir.join("missing.dat");
        let target = temp_dir.join("copied.dat");

        let err = LocalProject::copy_import_file(&source, &target)
            .expect_err("copy should fail for missing source file");

        match err {
            Error::ProjectImport {
                src_path,
                dst_path,
                details,
                is_permission_error,
            } => {
                assert_eq!(src_path, source);
                assert_eq!(dst_path, target);
                assert!(
                    details.contains("os error"),
                    "copy error details should include io/os context"
                );
                assert!(
                    !is_permission_error,
                    "NotFound errors must not set is_permission_error"
                );
            }
            other => panic!("expected ProjectImport error, got: {other:?}"),
        }

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    #[serial]
    fn test_working_copy_is_dirty_false_when_tracked_files_are_identical() {
        let id = Uuid::new_v4();
        let source_dir = setup_synced_index_and_working_copy(id);

        let is_dirty = LocalProject::working_copy_is_dirty(id)
            .expect("dirty check should succeed for synced copies");
        assert!(!is_dirty, "identical tracked files should be clean");

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_working_copy_is_dirty_true_when_tracked_file_bytes_differ() {
        let id = Uuid::new_v4();
        let source_dir = setup_synced_index_and_working_copy(id);
        let working_project =
            LocalProject::load_working_project(id).expect("working project metadata should load");
        let changed_file = working_project
            .project_map
            .dat_files
            .first()
            .expect("fixture should include tracked dat files");
        let changed_path = compass_project_working_path(id).join(changed_file);
        std::fs::write(&changed_path, b"modified-by-test")
            .expect("tracked file should be writable for test");

        let is_dirty = LocalProject::working_copy_is_dirty(id).expect("dirty check should succeed");
        assert!(
            is_dirty,
            "byte differences in tracked files must be detected"
        );

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_working_copy_is_dirty_true_when_tracked_file_is_missing() {
        let id = Uuid::new_v4();
        let source_dir = setup_synced_index_and_working_copy(id);
        let working_project =
            LocalProject::load_working_project(id).expect("working project metadata should load");
        let removed_file = working_project
            .project_map
            .dat_files
            .first()
            .expect("fixture should include tracked dat files");
        let removed_path = compass_project_working_path(id).join(removed_file);
        std::fs::remove_file(&removed_path).expect("tracked file should be removable");

        let is_dirty = LocalProject::working_copy_is_dirty(id).expect("dirty check should succeed");
        assert!(is_dirty, "missing tracked files must be treated as dirty");

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_import_compass_project_missing_mak_returns_project_not_found() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let missing_mak_path = std::env::temp_dir().join(format!("missing_import_{id}.mak"));

        let err = LocalProject::import_compass_project(id, &missing_mak_path)
            .expect_err("missing mak should return an error");

        assert!(
            matches!(err, Error::ProjectNotFound(path) if path == missing_mak_path),
            "expected ProjectNotFound for missing mak file"
        );
    }

    #[test]
    #[serial]
    fn test_import_compass_project_missing_referenced_file_returns_error() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let source_dir = setup_import_source(id, false);
        let mak_path = source_dir.join("Fulfords.mak");

        let err = LocalProject::import_compass_project(id, &mak_path)
            .expect_err("missing referenced file should error");

        assert!(
            matches!(err, Error::ProjectFileNotFound(_)),
            "expected ProjectFileNotFound when a referenced dat file is missing"
        );

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    fn test_copy_import_file_not_found_sets_permission_flag_false() {
        let source = std::env::temp_dir().join("nonexistent_speleodb_test_file.dat");
        let target = std::env::temp_dir().join("target_speleodb_test_file.dat");

        let err = LocalProject::copy_import_file(&source, &target)
            .expect_err("copying nonexistent file should fail");

        match err {
            Error::ProjectImport {
                src_path,
                dst_path,
                details,
                is_permission_error,
            } => {
                assert_eq!(src_path, source);
                assert_eq!(dst_path, target);
                assert!(
                    details.contains("kind:"),
                    "details should include error kind, got: {details}"
                );
                assert!(
                    !is_permission_error,
                    "NotFound errors must not set is_permission_error"
                );
            }
            other => panic!("expected ProjectImport error, got: {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_copy_import_file_permission_denied_sets_permission_flag_true() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = std::env::temp_dir().join(format!("speleodb_perm_test_{}", Uuid::new_v4()));
        std::fs::create_dir_all(&temp_dir).expect("temp dir");
        let source = temp_dir.join("locked.dat");
        let target = temp_dir.join("target.dat");
        std::fs::write(&source, b"data").expect("write source");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o000))
            .expect("remove read permissions");

        let err = LocalProject::copy_import_file(&source, &target)
            .expect_err("copying unreadable file should fail");

        // Restore permissions before any assertions so cleanup always works
        let _ = std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o644));
        let _ = std::fs::remove_dir_all(&temp_dir);

        match err {
            Error::ProjectImport {
                is_permission_error,
                ..
            } => {
                assert!(
                    is_permission_error,
                    "PermissionDenied IO errors must set is_permission_error"
                );
            }
            other => panic!("expected ProjectImport error, got: {other:?}"),
        }
    }

    #[test]
    #[serial]
    fn test_validate_working_copy_clean_project() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let source_dir = setup_import_source(id, true);
        let mak_path = source_dir.join("Fulfords.mak");
        LocalProject::import_compass_project(id, &mak_path).expect("import should succeed");

        let report = validate_working_copy(id);
        assert!(
            report.is_valid(),
            "freshly imported project should have no issues, got: {:?}",
            report.issues
        );

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_validate_working_copy_untracked_dat_file() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let source_dir = setup_import_source(id, true);
        let mak_path = source_dir.join("Fulfords.mak");
        LocalProject::import_compass_project(id, &mak_path).expect("import should succeed");

        // Remove a dat file from compass.toml tracking but keep it in the .mak
        let mut project = LocalProject::load_working_project(id).expect("load should succeed");
        let removed = project.project_map.dat_files.remove(0);
        let toml = toml::to_string_pretty(&project).expect("serialize");
        let toml_path = compass_project_working_path(id).join(SPELEODB_COMPASS_PROJECT_FILE);
        std::fs::write(&toml_path, &toml).expect("write compass.toml");

        let report = validate_working_copy(id);
        assert!(
            report
                .issues
                .iter()
                .any(|i| matches!(i, ValidationIssue::UntrackedDatFile(f) if f == &removed)),
            "should detect untracked dat file '{removed}', got: {:?}",
            report.issues
        );

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_validate_working_copy_missing_dat_file() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);
        let source_dir = setup_import_source(id, true);
        let mak_path = source_dir.join("Fulfords.mak");
        LocalProject::import_compass_project(id, &mak_path).expect("import should succeed");

        // Delete a tracked dat file from disk
        let project = LocalProject::load_working_project(id).expect("load should succeed");
        let deleted = project
            .project_map
            .dat_files
            .first()
            .expect("has dat files")
            .clone();
        let deleted_path = compass_project_working_path(id).join(&deleted);
        std::fs::remove_file(&deleted_path).expect("remove dat file");

        let report = validate_working_copy(id);
        assert!(
            report
                .issues
                .iter()
                .any(|i| matches!(i, ValidationIssue::MissingDatFile(f) if f == &deleted)),
            "should detect missing dat file '{deleted}', got: {:?}",
            report.issues
        );

        cleanup_project_dir(id);
        let _ = std::fs::remove_dir_all(source_dir);
    }

    #[test]
    #[serial]
    fn test_validate_working_copy_no_metadata() {
        let id = Uuid::new_v4();
        cleanup_project_dir(id);

        let report = validate_working_copy(id);
        assert!(
            report
                .issues
                .iter()
                .any(|i| matches!(i, ValidationIssue::NoProjectMetadata)),
            "should report NoProjectMetadata when compass.toml is missing, got: {:?}",
            report.issues
        );

        cleanup_project_dir(id);
    }
}
