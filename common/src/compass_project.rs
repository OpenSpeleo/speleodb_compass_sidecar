use std::path::Path;

use log::{error, info};
use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    Error, compass_project_index_path, compass_project_working_path,
    ensure_compass_project_dirs_exist, path_for_project,
};
const SPELEODB_COMPASS_PROJECT_FILE: &str = "compass.toml";
const SPELEODB_COMPASS_VERSION: Version = Version::new(0, 0, 1);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpeleoDb {
    id: Uuid,
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
pub struct Project {
    pub mak_file: Option<String>,
    pub dat_files: Vec<String>,
    pub plt_files: Vec<String>,
}

impl Project {
    pub fn import(mak_file: String, dat_files: Vec<String>) -> Self {
        let plt_files = vec![];
        Self {
            mak_file: Some(mak_file),
            dat_files,
            plt_files,
        }
    }

    pub fn new() -> Self {
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

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct CompassProject {
    pub speleodb: SpeleoDb,
    pub project: Project,
}

impl CompassProject {
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
            .map_err(|_| Error::CreateProjectDirectory(project_path.clone()))?;
        let new_project = Self {
            speleodb: SpeleoDb {
                id,
                version: SPELEODB_COMPASS_VERSION,
            },
            project: Project::import(
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

    pub fn empty_project(id: Uuid) -> Result<Self, Error> {
        info!("Creating empty Compass project for id: {id}");
        ensure_compass_project_dirs_exist(id)?;
        let new_project = Self {
            speleodb: SpeleoDb {
                id,
                version: SPELEODB_COMPASS_VERSION,
            },
            project: Project::new(),
        };
        let mut index_path = compass_project_index_path(id);
        index_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let serialized_project =
            toml::to_string_pretty(&new_project).map_err(|_| Error::Serialization)?;
        std::fs::write(&index_path, &serialized_project)
            .map_err(|_| Error::ProjectWrite(index_path.clone()))?;
        let mut working_copy_path = compass_project_working_path(id);
        working_copy_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        std::fs::write(&working_copy_path, &serialized_project)
            .map_err(|_| Error::ProjectWrite(working_copy_path.clone()))?;
        Ok(new_project)
    }

    pub fn load_working_project(id: Uuid) -> Result<Self, Error> {
        let mut project_path = compass_project_working_path(id);
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let project_data = std::fs::read_to_string(&project_path)
            .map_err(|_| Error::ProjectNotFound(project_path.clone()))?;
        let project: CompassProject =
            toml::from_str(&project_data).map_err(|e| Error::Deserialization(e.to_string()))?;
        Ok(project)
    }

    pub fn load_index_project(id: Uuid) -> Result<Self, Error> {
        let mut project_path = compass_project_working_path(id);
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let project_data = std::fs::read_to_string(&project_path)
            .map_err(|_| Error::ProjectNotFound(project_path.clone()))?;
        let project: CompassProject =
            toml::from_str(&project_data).map_err(|e| Error::Deserialization(e.to_string()))?;
        Ok(project)
    }

    pub fn is_empty(&self) -> bool {
        self.project.mak_file.is_none()
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, str::FromStr};

    use super::*;

    #[test]
    fn test_project_import() {
        let id = Uuid::new_v4();
        let project = CompassProject::import_compass_project(
            &PathBuf::from_str("assets/test_data/Fulfords.mak").unwrap(),
            id,
        )
        .unwrap();
        let serialized_project =
            toml::to_string_pretty(&project).expect("Failed to serialize imported project");
        println!("Imported project: {:?}", serialized_project);
        panic!();
    }
}
