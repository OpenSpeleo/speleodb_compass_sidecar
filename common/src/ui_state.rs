use crate::{Error, UserPrefs, api_types::ProjectInfo};
use serde::{Deserialize, Serialize};

pub const UI_STATE_NOTIFICATION_KEY: &str = "event::ui_state";

#[derive(Debug, Deserialize, Serialize)]
pub enum LoadingState {
    NotStarted,
    CheckingForUpdates,
    Updating,
    LoadingPrefs,
    Authenticating,
    LoadingProjects,
    Unauthenticated,
    Loaded,
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
    pub projects: Vec<ProjectInfo>,
    pub selected_project: Option<ProjectInfo>,
}

impl UiState {
    pub fn new() -> Self {
        let platform = if cfg!(target_os = "windows") {
            Platform::Windows
        } else if cfg!(target_os = "macos") {
            Platform::MacOS
        } else {
            Platform::Linux
        };
        Self {
            loading_state: LoadingState::NotStarted,
            platform,
            user_prefs: UserPrefs::default(),
            projects: vec![],
            selected_project: None,
        }
    }
}
