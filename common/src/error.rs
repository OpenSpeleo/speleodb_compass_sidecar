use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Project directory already exists at {0}")]
    ProjectAlreadyExists(PathBuf),
    #[error("Couldn't create storage directory for project: {0}")]
    CreateProjectDirectory(#[source] std::io::Error),
    #[error("Error deserializing TOML: {0}")]
    Deserialization(#[from] toml::de::Error),
    #[error("Error serializing TOML: {0}")]
    Serialization(#[from] toml::ser::Error),
    #[error("Error reading user preferece file: {0}")]
    UserPrefsRead(#[source] std::io::Error),
    #[error("Error writing user preference file: {0}")]
    UserPrefsWrite(#[source] std::io::Error),
    #[error("Error writing project file: {0}")]
    ProjectWrite(#[source] std::io::Error),
    #[error("Error setting file permissions: {0}")]
    FilePermissionSet(#[source] std::io::Error),
}
