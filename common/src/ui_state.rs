// Re-export api types used directly in the UI
pub use crate::api_types::{ActiveMutex, ProjectInfo, ProjectSaveResult, ProjectType};

use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const UI_STATE_NOTIFICATION_KEY: &str = "event::ui_state";
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum LocalProjectStatus {
    Unknown,
    RemoteOnly,
    EmptyLocal,
    Dirty,
    UpToDate,
    OutOfDate,
    DirtyAndOutOfDate,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ProjectStatus {
    local_status: LocalProjectStatus,
    info: ProjectInfo,
}

impl ProjectStatus {
    pub fn new(local_status: LocalProjectStatus, info: ProjectInfo) -> Self {
        Self { local_status, info }
    }

    pub fn id(&self) -> Uuid {
        self.info.id
    }

    pub fn name(&self) -> &str {
        &self.info.name
    }

    pub fn active_mutex(&self) -> &Option<ActiveMutex> {
        &self.info.active_mutex
    }

    pub fn permission(&self) -> &str {
        &self.info.permission
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum LoadingState {
    NotStarted,
    CheckingForUpdates,
    Updating,
    LoadingPrefs,
    Authenticating,
    LoadingProjects,
    Unauthenticated,
    Ready,
    Failed(Error),
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UiState {
    pub loading_state: LoadingState,
    pub platform: Platform,
    pub project_status: Vec<ProjectStatus>,
    pub selected_project: Option<Uuid>,
}

impl UiState {
    pub fn new(
        loading_state: LoadingState,
        project_status: Vec<ProjectStatus>,
        selected_project: Option<Uuid>,
    ) -> Self {
        let platform = if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        };
        Self {
            loading_state,
            platform,
            project_status,
            selected_project,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(LoadingState::NotStarted, vec![], None)
    }
}
