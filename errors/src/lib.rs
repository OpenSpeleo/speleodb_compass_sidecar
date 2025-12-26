use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Error, PartialEq, Serialize)]
pub enum Error {
    #[error("No auth token set")]
    NoAuthToken,
    #[error("Project directory already exists at {0}")]
    ProjectAlreadyExists(PathBuf),
    #[error("Project not found: {0}")]
    ProjectNotFound(PathBuf),
    #[error("Couldn't create storage directory for project")]
    CreateDirectory(PathBuf),
    #[error("Error deserializing data: {0}")]
    Deserialization(String),
    #[error("Error serializing TOML")]
    Serialization,
    #[error("No user preferences found")]
    NoUserPreferences,
    #[error("Error reading user preferece file")]
    ApiInfoRead(PathBuf),
    #[error("Error writing user preference file")]
    ApiInfoWrite(PathBuf),
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
    Api(u16),
    #[error("File read failed: {0}")]
    FileRead(String),
    #[error("File write failed: {0}")]
    FileWrite(String),
    #[error("No project data found for : {0}")]
    NoProjectData(Uuid),
    #[error("Project mutex already locked")]
    ProjectMutexLocked(Uuid),
    #[error("Zip File Error: {0}")]
    ZipFile(String),
}
