use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize, Error, Serialize)]
pub enum Error {
    #[error("Project directory already exists at {0}")]
    ProjectAlreadyExists(PathBuf),
    #[error("Couldn't create storage directory for project")]
    CreateProjectDirectory,
    #[error("Error deserializing TOML")]
    Deserialization,
    #[error("Error serializing TOML")]
    Serialization,
    #[error("Error reading user preferece file")]
    UserPrefsRead,
    #[error("Error writing user preference file")]
    UserPrefsWrite,
    #[error("Error writing project file")]
    ProjectWrite,
    #[error("Error setting file permissions")]
    FilePermissionSet,
    #[error("No project selected")]
    NoProjectSelected,
}
