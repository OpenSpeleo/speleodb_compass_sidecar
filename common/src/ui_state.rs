// Re-export api types used directly in the UI
pub use crate::api_types::{ActiveMutex, ProjectInfo, ProjectSaveResult, ProjectType};

use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const UI_STATE_NOTIFICATION_KEY: &str = "event::ui_state";

/// The status of a local project in relation to its remote counterpart.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub enum LocalProjectStatus {
    /// The status of the local project is unknown.
    /// Never seen in UI
    Unknown,
    /// The project exists only on the remote server.
    /// Depicted in UI bwo cloud icon?
    RemoteOnly,
    /// The project exists only on the local machine, and has no changes
    /// This is the only status that allows compass project import
    /// Shows button to upload
    EmptyLocal,
    /// The local project has unsaved changes.
    /// UI should warn user somehow
    /// Saved/unsaved indicator?
    Dirty,
    /// The local project is synchronized with the remote server.
    UpToDate,
    /// The local project is out of date with the remote server.
    OutOfDate,
    /// The local project has unsaved changes and is out of date with the remote server. Uh Oh...
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

    pub fn local_status(&self) -> LocalProjectStatus {
        self.local_status
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

    pub fn is_dirty(&self) -> bool {
        matches!(
            self.local_status,
            LocalProjectStatus::Dirty | LocalProjectStatus::DirtyAndOutOfDate
        )
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
    pub user_email: Option<String>,
    pub project_status: Vec<ProjectStatus>,
    pub selected_project_id: Option<Uuid>,
    pub compass_open: bool,
}

impl UiState {
    pub fn new(
        loading_state: LoadingState,
        user_email: Option<String>,
        project_status: Vec<ProjectStatus>,
        selected_project: Option<Uuid>,
        compass_open: bool,
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
            user_email,
            project_status,
            selected_project_id: selected_project,
            compass_open,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(LoadingState::NotStarted, None, vec![], None, false)
    }
}
