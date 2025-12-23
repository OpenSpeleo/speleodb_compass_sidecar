use crate::{Error, api_types::ProjectInfo};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const UI_STATE_NOTIFICATION_KEY: &str = "event::ui_state";

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
    pub project_info: Vec<ProjectInfo>,
    pub selected_project: Option<Uuid>,
}

impl UiState {
    pub fn new(
        loading_state: LoadingState,
        project_info: Vec<ProjectInfo>,
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
            project_info,
            selected_project,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(LoadingState::NotStarted, vec![], None)
    }
}
