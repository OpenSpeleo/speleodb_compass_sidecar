use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize, Error, Serialize)]
pub enum Error {
    #[error("No auth token set")]
    NoAuthToken,
    #[error("Project directory already exists at {0}")]
    ProjectAlreadyExists(PathBuf),
    #[error("Project not found: {0}")]
    ProjectNotFound(PathBuf),
    #[error("Couldn't create storage directory for project")]
    CreateProjectDirectory(PathBuf),
    #[error("Error deserializing data: {0}")]
    Deserialization(String),
    #[error("Error serializing TOML")]
    Serialization,
    #[error("Error reading user preferece file")]
    UserPrefsRead(PathBuf),
    #[error("Error writing user preference file")]
    UserPrefsWrite(PathBuf),
    #[error("Error importing project file from: {0} to {1}")]
    ProjectImport(PathBuf, PathBuf),
    #[error("Error writing project file")]
    ProjectWrite(PathBuf),
    #[error("Error setting file permissions")]
    FilePermissionSet,
    #[error("No project selected")]
    NoProjectSelected,
    #[error("Referenced file not found: {0}")]
    ProjectFileNotFound(PathBuf),
    #[error("Empty project directory for project ID: {0}")]
    EmptyProjectDirectory(uuid::Uuid),
    #[error("Network request error: {0}")]
    NetworkRequest(String),
    #[error("Api request failed with status code: {0}")]
    ApiRequest(u16),
}
