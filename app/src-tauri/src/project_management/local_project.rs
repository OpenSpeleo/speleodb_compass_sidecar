use crate::{
    SPELEODB_COMPASS_VERSION,
    paths::{compass_project_index_path, compass_project_working_path},
    project_management::{SPELEODB_COMPASS_PROJECT_FILE, SpeleoDbProjectRevision},
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
/// The additional path is required to locate the project files, as this can represent a working copy or an index.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LocalProject {
    project_path: PathBuf,
    speleodb: SpeleoDb,
    map: ProjectMap,
}

impl LocalProject {
    // Get the SpeleoDb project revision associated with this project, if it exists.
    pub fn revision(&self) -> Option<SpeleoDbProjectRevision> {
        SpeleoDbProjectRevision::revision_for_project(self.speleodb.id)
    }

    pub fn import_compass_project(mak_path: &Path, id: Uuid) -> Result<Self, Error> {
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
            project_path: project_path.clone(),
            speleodb: SpeleoDb {
                id,
                version: SPELEODB_COMPASS_VERSION,
            },
            map: ProjectMap::import(
                mak_path.file_name().unwrap().to_string_lossy().to_string(),
                project_files.clone(),
            ),
        };
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let serialized_project =
            toml::to_string_pretty(&new_project).map_err(|_| Error::Serialization)?;
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
        Ok(new_project)
    }

    /// Create an empty Compass project with no files.
    pub fn empty_working_copy(id: Uuid) -> Self {
        info!("Creating empty Compass project for id: {id}");
        //
        let project_path = compass_project_working_path(id);
        let new_project = Self {
            project_path,
            speleodb: SpeleoDb {
                id,
                version: SPELEODB_COMPASS_VERSION,
            },
            map: ProjectMap::new(),
        };
        new_project
    }

    pub fn load_working_project(id: Uuid) -> Result<Self, Error> {
        let mut project_path = compass_project_working_path(id);
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let project_data = std::fs::read_to_string(&project_path)
            .map_err(|_| Error::ProjectNotFound(project_path.clone()))?;
        let project: LocalProject =
            toml::from_str(&project_data).map_err(|e| Error::Deserialization(e.to_string()))?;
        Ok(project)
    }

    pub fn load_index_project(id: Uuid) -> Result<Self, Error> {
        let mut project_path = compass_project_index_path(id);
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let project_data = std::fs::read_to_string(&project_path)
            .map_err(|_| Error::ProjectNotFound(project_path.clone()))?;
        let project: LocalProject =
            toml::from_str(&project_data).map_err(|e| Error::Deserialization(e.to_string()))?;
        Ok(project)
    }

    pub fn is_empty(&self) -> bool {
        self.map.mak_file.is_none()
    }

    /// Pack the working copy of a Compass project into a zip file and return the path to the zip.
    pub fn pack_zip(&self) -> Result<PathBuf, String> {
        let project_id = self.speleodb.id;
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
        let project_toml = toml::to_string_pretty(&self).map_err(|e| e.to_string())?;
        zip_writer
            .write_all(project_toml.as_bytes())
            .map_err(|e| e.to_string())?;
        let project_dir = compass_project_working_path(project_id);

        if let Some(mak_file_path) = self.map.mak_file.as_ref() {
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

        for dat_path in self.map.dat_files.iter() {
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
            &PathBuf::from_str("assets/test_data/Fulfords.mak").unwrap(),
            id,
        )
        .unwrap();
        let serialized_project =
            toml::to_string_pretty(&project).expect("Failed to serialize imported project");
        println!("Imported project: {:?}", serialized_project);
    }
}
