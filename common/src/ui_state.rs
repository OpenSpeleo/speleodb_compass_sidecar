// Re-export api types used directly in the UI
pub use crate::api_types::{ActiveMutex, ProjectInfo, ProjectSaveResult, ProjectType};

use crate::Error;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

    pub fn modified_date(&self) -> &str {
        &self.info.modified_date
    }

    pub fn active_mutex(&self) -> &Option<ActiveMutex> {
        &self.info.active_mutex
    }

    pub fn permission(&self) -> &str {
        &self.info.permission
    }

    pub fn latest_commit(&self) -> Option<&crate::api_types::CommitInfo> {
        self.info.latest_commit.as_ref()
    }

    pub fn is_dirty(&self) -> bool {
        matches!(
            self.local_status,
            LocalProjectStatus::Dirty | LocalProjectStatus::DirtyAndOutOfDate
        )
    }
}

/// Describes the application's *initialization* progression. Update-check
/// activity is reported separately via [`UpdateNotification`] so that updater
/// UX never blocks auth/project loading.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum LoadingState {
    NotStarted,
    LoadingPrefs,
    Authenticating,
    LoadingProjects,
    Unauthenticated,
    Ready,
    Failed(Error),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub enum UpdateNotificationPhase {
    Checking,
    Downloading {
        version: String,
        progress_percent: Option<u8>,
    },
    Installing {
        version: String,
    },
    Relaunching {
        version: String,
    },
    UpToDate {
        app_name: String,
    },
    Failed {
        message: String,
    },
}

impl UpdateNotificationPhase {
    pub fn dismissal_key_part(&self) -> &'static str {
        match self {
            Self::Checking => "checking",
            Self::Downloading { .. } => "downloading",
            Self::Installing { .. } => "installing",
            Self::Relaunching { .. } => "relaunching",
            Self::UpToDate { .. } => "up-to-date",
            Self::Failed { .. } => "failed",
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UpdateNotification {
    pub id: u64,
    pub phase: UpdateNotificationPhase,
}

impl UpdateNotification {
    pub fn new(id: u64, phase: UpdateNotificationPhase) -> Self {
        Self { id, phase }
    }

    pub fn dismissal_key(&self) -> String {
        format!("{}:{}", self.id, self.phase.dismissal_key_part())
    }
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
    pub project_downloading: bool,
    pub update_notification: Option<UpdateNotification>,
}

impl UiState {
    pub fn new(
        loading_state: LoadingState,
        user_email: Option<String>,
        project_status: Vec<ProjectStatus>,
        selected_project: Option<Uuid>,
        compass_open: bool,
        project_downloading: bool,
        update_notification: Option<UpdateNotification>,
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
            project_downloading,
            update_notification,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(
            LoadingState::NotStarted,
            None,
            vec![],
            None,
            false,
            false,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{UpdateNotification, UpdateNotificationPhase};

    #[test]
    fn downloading_progress_keeps_same_dismissal_key() {
        let first = UpdateNotification::new(
            7,
            UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: Some(1),
            },
        );
        let second = UpdateNotification::new(
            7,
            UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: Some(42),
            },
        );

        assert_eq!(first.dismissal_key(), second.dismissal_key());
    }

    #[test]
    fn phase_changes_get_different_dismissal_keys() {
        let downloading = UpdateNotification::new(
            7,
            UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: Some(100),
            },
        );
        let installing = UpdateNotification::new(
            7,
            UpdateNotificationPhase::Installing {
                version: "0.2.0".to_string(),
            },
        );

        assert_ne!(downloading.dismissal_key(), installing.dismissal_key());
    }

    #[test]
    fn repeated_manual_checks_get_new_ids() {
        let first = UpdateNotification::new(1, UpdateNotificationPhase::Checking);
        let second = UpdateNotification::new(2, UpdateNotificationPhase::Checking);

        assert_ne!(first.dismissal_key(), second.dismissal_key());
    }
}
