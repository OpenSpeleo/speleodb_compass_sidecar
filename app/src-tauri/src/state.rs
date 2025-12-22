use common::{
    Error, LoadingState, UI_STATE_NOTIFICATION_KEY, UiState, UserPrefs, api_types::ProjectInfo,
};
use log::warn;
use std::{collections::HashMap, sync::Mutex};
use tauri::{AppHandle, Emitter, ipc::private::tracing::info};
use url::Url;

pub struct AppState {
    loading_state: Mutex<LoadingState>,
    api_info: Mutex<UserPrefs>,
    project_info: Mutex<HashMap<uuid::Uuid, ProjectInfo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            loading_state: Mutex::new(LoadingState::NotStarted),
            api_info: Mutex::new(UserPrefs::default()),
            project_info: Mutex::new(HashMap::new()),
        }
    }

    /// Asynchronously initialize the application state.
    pub async fn init_app_state(&self, app_handle: &AppHandle) {
        let mut loading_state = self.loading_state();
        if let LoadingState::Failed(e) = &loading_state {
            log::warn!(
                "Previous initialization failed with error: {}. Retrying initialization.",
                e
            );
            self.set_loading_state(LoadingState::NotStarted, &app_handle);
            loading_state = LoadingState::NotStarted;
        }
        loop {
            match &loading_state {
                LoadingState::Failed(e) => {
                    log::warn!(
                        "Previous initialization failed with error: {}. Retrying initialization.",
                        e
                    );
                    self.set_loading_state(LoadingState::NotStarted, &app_handle);
                }
                LoadingState::Unauthenticated | LoadingState::Ready => {
                    log::info!("App state already initialized",);
                    self.emit_app_state_change(app_handle);
                    break;
                }
                _ => loading_state = self.init_internal(&app_handle).await,
            }
        }
    }

    pub fn api_info(&self) -> UserPrefs {
        self.api_info.lock().unwrap().clone()
    }

    pub fn update_user_prefs(&self, prefs: UserPrefs) -> Result<(), Error> {
        UserPrefs::save(&prefs)?;
        *self.api_info.lock().unwrap() = prefs;
        Ok(())
    }

    pub fn forget_user_prefs(&self) -> Result<(), Error> {
        UserPrefs::forget()?;
        *self.api_info.lock().unwrap() = UserPrefs::default();
        Ok(())
    }

    pub fn update_project_info(&self, project_info: &ProjectInfo) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.insert(project_info.id, project_info.clone());
    }

    pub fn get_project(&self, project_id: uuid::Uuid) -> Option<ProjectInfo> {
        let project_lock = self.project_info.lock().unwrap();
        project_lock.get(&project_id).cloned()
    }

    pub fn clear_projects(&self) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.clear();
    }

    fn emit_app_state_change(&self, app_handle: &AppHandle) {
        let loading_state = self.loading_state();
        let user_prefs = self.api_info();
        let project_info = self
            .project_info
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();
        let selected_project = None; // Placeholder for selected project
        let ui_state = UiState::new(loading_state, user_prefs, project_info, selected_project);
        app_handle
            .emit(UI_STATE_NOTIFICATION_KEY, &ui_state)
            .unwrap();
    }

    fn loading_state(&self) -> LoadingState {
        self.loading_state.lock().unwrap().clone()
    }

    /// Internal function used to update loading state and emit state change event.
    fn set_loading_state(&self, state: LoadingState, app_handle: &AppHandle) -> LoadingState {
        *self.loading_state.lock().unwrap() = state.clone();
        self.emit_app_state_change(app_handle);
        state
    }

    async fn check_for_updates(&self) -> Result<bool, Error> {
        info!("Checking for app updates");
        //TODO: Implement update checking logic
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // For now, just return false to indicate no updates available
        Ok(false)
    }

    async fn update_app(&self) -> Result<(), Error> {
        info!("Updating application");
        Ok(())
    }

    fn load_user_preferences(&self) -> UserPrefs {
        info!("Loading user preferences");
        let user_prefs = UserPrefs::load().unwrap_or_default();
        self.update_user_prefs(user_prefs.clone())
            .unwrap_or_else(|e| {
                log::warn!("Failed to save loaded user preferences: {}", e);
            });
        user_prefs
    }

    async fn authenticate_user(&self) -> Result<(), String> {
        let prefs = self.api_info();
        info!("Authenticating user");
        let Some(token) = prefs.oauth_token() else {
            log::warn!("No OAuth token found in user preferences");
            return Err("No OAuth token found".to_string());
        };
        match api::auth::authorize_with_token(prefs.instance(), &token).await {
            Ok(_) => {
                log::info!("User authenticated successfully");
                let prefs = UserPrefs::new(prefs.instance().clone(), Some(token.to_string()));
                if self.update_user_prefs(prefs).is_err() {
                    log::warn!("Failed to save user preferences after authentication");
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to authenticate user with saved token: {}", e);
                Err(e)
            }
        }
    }

    async fn load_user_projects(&self) -> Result<Vec<ProjectInfo>, Error> {
        info!("Loading user projects");
        let prefs = self.api_info();
        match api::project::fetch_projects(&prefs).await {
            Ok(projects) => {
                for project in &projects {
                    self.update_project_info(project);
                }
                Ok(projects)
            }
            Err(e) => {
                warn!("Failed to load user projects: {}", e);
                Err(e)
            }
        }
    }

    pub async fn init_internal(&self, app_handle: &AppHandle) -> LoadingState {
        let loading_state = self.loading_state();
        match loading_state {
            LoadingState::NotStarted => {
                self.set_loading_state(LoadingState::CheckingForUpdates, app_handle)
            }
            LoadingState::CheckingForUpdates => match self.check_for_updates().await {
                Ok(update_available) => {
                    if update_available {
                        log::info!("Update available, upating...");
                        self.set_loading_state(LoadingState::Updating, app_handle)
                    } else {
                        log::info!("No updates available, attempting to load user preferences");
                        self.set_loading_state(LoadingState::LoadingPrefs, app_handle)
                    }
                }
                Err(e) => {
                    log::warn!("Failed to check for updates: {}", e);
                    self.set_loading_state(LoadingState::Failed(e), app_handle)
                }
            },
            LoadingState::Updating => match self.update_app().await {
                Ok(_) => {
                    log::info!("Update successful, loading user preferences");
                    self.set_loading_state(LoadingState::LoadingPrefs, app_handle)
                }
                Err(e) => {
                    log::warn!("Failed to update application: {}", e);
                    self.set_loading_state(LoadingState::LoadingPrefs, app_handle)
                }
            },
            LoadingState::LoadingPrefs => {
                let prefs = self.load_user_preferences();
                if let Some(_token) = prefs.oauth_token() {
                    info!("User prefs found, attempting to authenticate user");
                    self.set_loading_state(LoadingState::Authenticating, app_handle)
                } else {
                    info!("No user prefs found, starting unauthenticated");
                    self.set_loading_state(LoadingState::Unauthenticated, app_handle)
                }
            }
            LoadingState::Authenticating => match self.authenticate_user().await {
                Ok(_) => self.set_loading_state(LoadingState::LoadingProjects, app_handle),
                Err(e) => {
                    // TODO:: Handle different authentication errors appropriately
                    log::warn!("Authentication failed: {}", e);
                    self.set_loading_state(LoadingState::Unauthenticated, app_handle)
                }
            },
            LoadingState::LoadingProjects => match self.load_user_projects().await {
                Ok(_) => {
                    log::info!("User projects loaded successfully, app is ready");
                    self.set_loading_state(LoadingState::Ready, app_handle)
                }
                Err(e) => {
                    log::warn!("Failed to load user projects: {}", e);
                    self.set_loading_state(LoadingState::Failed(e), app_handle)
                }
            },
            _ => {
                log::info!("App already initialized or in progress");
                loading_state
            }
        }
    }
}
/*
// Load user prefs
ui_state.loading_state = LoadingState::LoadingPrefs;
app_handle
    .emit(UI_STATE_NOTIFICATION_KEY, &ui_state)
    .unwrap();
let prefs = UserPrefs::load().unwrap_or_default();

if let Some(token) = prefs.oauth_token() {
    log::info!("User prefs found, attempting to authenticate user");
    ui_state.loading_state = LoadingState::Authenticating;
    app_handle
        .emit(UI_STATE_NOTIFICATION_KEY, &ui_state)
        .unwrap();

} else {
    log::info!("No user prefs found, starting unauthenticated");
    app_handle.emit("event::authentication", false).unwrap();
}
Ok(())
 */
