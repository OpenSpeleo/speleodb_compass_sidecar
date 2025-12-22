use crate::{Error, UserPrefs, api_types::ProjectInfo, user_prefs};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize, Serialize)]
pub enum Platform {
    Windows,
    MacOS,
    Linux,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UiState {
    pub loading_state: LoadingState,
    pub platform: Platform,
    pub user_prefs: UserPrefs,
    pub project_info: Vec<ProjectInfo>,
    pub selected_project: Option<ProjectInfo>,
}

impl UiState {
    pub fn new(
        loading_state: LoadingState,
        user_prefs: UserPrefs,
        project_info: Vec<ProjectInfo>,
        selected_project: Option<ProjectInfo>,
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
            user_prefs,
            project_info,
            selected_project,
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new(LoadingState::NotStarted, UserPrefs::default(), vec![], None)
    }
}
