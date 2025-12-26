use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use common::{Error, api_types::CommitInfo};

use crate::{paths::compass_project_path, project_management::SPELEODB_PROJECT_REVISION_FILE};

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SpeleoDbProjectRevision {
    pub revision: String,
}

impl SpeleoDbProjectRevision {
    pub fn revision_for_project(id: Uuid) -> Option<SpeleoDbProjectRevision> {
        std::fs::read_to_string(SpeleoDbProjectRevision::path_for_project(id))
            .ok()
            .map(|revision| Self { revision })
    }

    pub fn save_revision_for_project(&self, id: Uuid) -> Result<(), Error> {
        let path = SpeleoDbProjectRevision::path_for_project(id);
        std::fs::write(&path, &self.revision).map_err(|_| Error::ProjectWrite(path.clone()))
    }

    fn path_for_project(id: Uuid) -> PathBuf {
        let mut revision_path = compass_project_path(id);
        revision_path.push(SPELEODB_PROJECT_REVISION_FILE);
        revision_path
    }
}

impl From<&CommitInfo> for SpeleoDbProjectRevision {
    fn from(commit_info: &CommitInfo) -> Self {
        Self {
            revision: commit_info.id.clone(),
        }
    }
}
