use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Api error: {0}")]
    Api(u16),
    #[error("Http request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("No project data found for : {0}")]
    NoProjectData(Uuid),
    #[error("No auth token available")]
    NoAuthToken,
    #[error("Project mutex already locked")]
    ProjectMutexLocked(Uuid),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
