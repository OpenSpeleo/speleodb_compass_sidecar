use semver::Version;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
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
    mak_file: String,
    dat_files: Vec<String>,
    plt_files: Vec<String>,
}

impl Project {
    pub fn new(
        name: &str,
        description: &str,
        mak_file: String,
        dat_files: Vec<String>,
        plt_files: Vec<String>,
    ) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            mak_file,
            dat_files,
            plt_files,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompassProject {
    pub speleodb: SpeleoDb,
    pub project: Project,
}
