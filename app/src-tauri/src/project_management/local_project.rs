//! Module for managing local Compass projects stored on disk.
//! This includes loading, saving, importing, and packing projects.
//! A local project consists of an index copy and a working copy.
//! The index copy represents the last known state of the project as stored in SpeleoDB,
//! while the working copy represents the current state of the project on disk.

use crate::{
    SPELEODB_COMPASS_VERSION,
    paths::{compass_project_index_path, compass_project_working_path},
    project_management::{SPELEODB_COMPASS_PROJECT_FILE, SpeleoDbProjectRevision},
};
use common::Error;
use compass_data::{Loaded, Project};
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

    pub fn new() -> Self {
        Self::default()
    }
}

impl Default for ProjectMap {
    fn default() -> Self {
        let mak_file = None;
        let dat_files = vec![];
        let plt_files = vec![];
        Self {
            mak_file,
            dat_files,
            plt_files,
        }
    }
}

/// Represents a local Compass project stored on disk.
/// Note that this struct does not contain the actual survey data files,
/// but rather metadata about the project and references to the data files.
/// This struct is only ever serialized/deserialized to/from the SPELEODB_COMPASS_PROJECT_FILE file, and
/// not created or accessed anywhere outside of this file.
/// Instead, associated functions provide access to the work with the data on disk
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(crate) struct LocalProject {
    speleodb: SpeleoDb,
    #[serde(rename = "project")]
    project_map: ProjectMap,
}

impl LocalProject {
    pub fn working_copy_is_dirty(id: Uuid) -> Result<bool, Error> {
        let index_copy = LocalProject::load_index_project(id).ok();
        let working_copy = LocalProject::load_working_project(id).ok();
        if let Some(index_copy) = index_copy {
            if let Some(working_copy) = working_copy {
                // Both copies exist, compare them
                if index_copy == working_copy {
                    // No changes at the map level, now check the files
                    let index_project = LocalProject::load_index_compass_project(id)?;
                    let working_project = LocalProject::load_working_copy_compass_project(id)?;
                    if index_project == working_project {
                        // No changes detected
                        Ok(false)
                    } else {
                        info!("Detected changes between loaded compass projects for: {id}");
                        info!("Index project: {:#?}", index_project);
                        info!("Working project: {:#?}", working_project);
                        // Changes detected
                        Ok(true)
                    }
                } else {
                    // Changes detected
                    Ok(true)
                }
            } else {
                // No working copy, so not dirty
                error!("Index is populated, but local copy doesn't exist");
                // TODO: Decide if we should just clone the index to working copy here
                unreachable!(
                    "Index populated, but working copy doesn't exist when checking for changes"
                );
            }
        } else if let Some(_) = working_copy {
            // No index copy, but working copy exists, so dirty
            Ok(true)
        } else {
            // Neither copy exists, so not dirty
            Ok(false)
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

        std::fs::create_dir_all(&project_path)
            .map_err(|_| Error::CreateDirectory(project_path.clone()))?;
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
        std::fs::write(&project_path, &serialized_project)
            .map_err(|_| Error::ProjectWrite(project_path.clone()))?;
        // Copy the .mak file and all referenced survey files into the new project directory
        let mut mak_target_path = compass_project_working_path(id);
        mak_target_path.push(mak_path.file_name().unwrap());
        std::fs::copy(&mak_path, &mak_target_path)
            .map_err(|_| Error::ProjectImport(mak_path.clone(), mak_target_path.clone()))?;
        for (file_path, relative_path) in project_file_paths.iter().zip(project_files.iter()) {
            let mut target_path = compass_project_working_path(id);
            target_path.push(relative_path);
            std::fs::copy(file_path, &target_path)
                .map_err(|_| Error::ProjectImport(file_path.to_owned(), target_path.to_owned()))?;
        }
        Ok(())
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
            Ok(working_copy) => {
                if working_copy.project_map.mak_file.is_none() {
                    false
                } else {
                    true
                }
            }
            Err(Error::ProjectNotFound(_)) => false,
            Err(e) => panic!("Error checking working copy existence: {}", e),
        }
    }

    pub fn index_exists(id: Uuid) -> bool {
        match LocalProject::load_index_project(id) {
            Ok(index) => {
                if index.project_map.mak_file.is_none() {
                    false
                } else {
                    true
                }
            }
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
            std::fs::File::create(&zip_path).map_err(|e| Error::ProjectWrite(zip_path.clone()))?;
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

        if let Some(mak_file_path) = working_copy.project_map.mak_file.as_ref() {
            let mak_full_path = project_dir.join(&mak_file_path);
            zip_writer
                .start_file(&mak_file_path, options)
                .map_err(|e| Error::ZipFile(e.to_string()))?;
            let mak_contents =
                std::fs::read(&mak_full_path).map_err(|e| Error::FileRead(e.to_string()))?;
            zip_writer
                .write_all(&mak_contents)
                .map_err(|e| Error::ZipFile(e.to_string()))?;
        }

        for dat_path in working_copy.project_map.dat_files.iter() {
            let dat_full_path = project_dir.join(dat_path);
            zip_writer
                .start_file(&dat_path, options)
                .map_err(|e| Error::ZipFile(e.to_string()))?;
            let dat_contents =
                std::fs::read(&dat_full_path).map_err(|e| Error::FileRead(e.to_string()))?;
            zip_writer
                .write_all(&dat_contents)
                .map_err(|e| Error::ZipFile(e.to_string()))?;
        }

        zip_writer
            .finish()
            .map_err(|e| Error::ZipFile(e.to_string()))?;
        Ok(zip_path)
    }

    fn load_compass_project(path: &Path) -> Result<Project<Loaded>, Error> {
        let compass_project =
            Project::read(path).map_err(|e| Error::CompassProject(e.to_string()))?;
        let loaded_compass_project = compass_project
            .load_survey_files()
            .map_err(|e| Error::CompassProject(e.to_string()))?;
        Ok(loaded_compass_project)
    }

    fn load_index_compass_project(id: Uuid) -> Result<Project<Loaded>, Error> {
        let local_project = LocalProject::load_index_project(id)?;
        let mut project_path = compass_project_index_path(id);
        let mak_file_name = local_project
            .project_map
            .mak_file
            .ok_or_else(|| Error::NoProjectData(id))?;
        project_path.push(&mak_file_name);
        LocalProject::load_compass_project(&project_path)
    }

    fn load_working_copy_compass_project(id: Uuid) -> Result<Project<Loaded>, Error> {
        let local_project = LocalProject::load_working_project(id)?;
        let mut project_path = compass_project_working_path(id);
        let mak_file_name = local_project
            .project_map
            .mak_file
            .ok_or_else(|| Error::NoProjectData(id))?;
        project_path.push(&mak_file_name);
        LocalProject::load_compass_project(&project_path)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serial_test::serial;
    use std::{path::PathBuf, str::FromStr};

    #[test]
    #[serial]
    fn test_project_import() {
        let id = Uuid::new_v4();
        let project = LocalProject::import_compass_project(
            id,
            &PathBuf::from_str("assets/test_data/Fulfords.mak").unwrap(),
        )
        .unwrap();
        let serialized_project =
            toml::to_string_pretty(&project).expect("Failed to serialize imported project");
        println!("Imported project: {:?}", serialized_project);
    }
}
