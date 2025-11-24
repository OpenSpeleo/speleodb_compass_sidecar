use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{Error, path_for_project};
const SPELEODB_COMPASS_PROJECT_FILE: &'static str = "compass.toml";
const SPELEODB_COMPASS_VERSION: Version = Version::new(0, 0, 1);

#[derive(Clone, Debug, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Project {
    name: String,
    description: String,
    mak_file: Option<String>,
    dat_files: Vec<String>,
    plt_files: Vec<String>,
}

impl Project {
    pub fn new(name: String, description: String) -> Self {
        let mak_file = None;
        let dat_files = vec![];
        let plt_files = vec![];
        Self {
            name: name,
            description,
            mak_file,
            dat_files,
            plt_files,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompassProject {
    speleodb: SpeleoDb,
    project: Project,
}

impl CompassProject {
    pub fn create_compass_project(
        name: String,
        description: String,
        id: Uuid,
    ) -> Result<Self, Error> {
        let mut project_path = path_for_project(id);
        if project_path.exists() {
            return Err(Error::ProjectAlreadyExists(project_path));
        }
        std::fs::create_dir_all(&project_path).map_err(|_| Error::CreateProjectDirectory)?;
        let new_project = Self {
            speleodb: SpeleoDb {
                id,
                version: SPELEODB_COMPASS_VERSION,
            },
            project: Project::new(name, description),
        };
        project_path.push(SPELEODB_COMPASS_PROJECT_FILE);
        let serialized_project =
            toml::to_string_pretty(&new_project).map_err(|_| Error::Serialization)?;
        std::fs::write(project_path, &serialized_project).map_err(|_| Error::ProjectWrite)?;
        Ok(new_project)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_project_creation() {
        let name = "Test Project".to_string();
        let description = "Super awesome cave".to_string();
        let id = Uuid::new_v4();
        let project = CompassProject::create_compass_project(name, description, id).unwrap();
    }
}
