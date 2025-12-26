use common::{
    ApiInfo, Error,
    api_types::ProjectInfo,
    ui_state::{LoadingState, UI_STATE_NOTIFICATION_KEY, UiState},
};
use log::warn;
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{AppHandle, Emitter, ipc::private::tracing::info};
use uuid::Uuid;

use crate::{project_management::ProjectManager, user_prefs::UserPrefs};

pub struct AppState {
    loading_state: Mutex<LoadingState>,
    api_info: Mutex<ApiInfo>,
    project_info: Mutex<HashMap<uuid::Uuid, ProjectManager>>,
    active_project: Mutex<Option<uuid::Uuid>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            loading_state: Mutex::new(LoadingState::NotStarted),
            api_info: Mutex::new(ApiInfo::default()),
            project_info: Mutex::new(HashMap::new()),
            active_project: Mutex::new(None),
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

    pub fn api_info(&self) -> ApiInfo {
        self.api_info.lock().unwrap().clone()
    }

    pub fn update_user_prefs(&self, prefs: UserPrefs, app_handle: &AppHandle) -> Result<(), Error> {
        prefs.save()?;
        *self.api_info.lock().unwrap() = prefs.api_info().clone();
        self.emit_app_state_change(app_handle);
        Ok(())
    }

    pub async fn authenticated(&self, app_handle: &AppHandle) -> () {
        self.set_loading_state(LoadingState::LoadingProjects, app_handle);
        self.init_app_state(app_handle).await;
    }

    pub fn sign_out(&self, app_handle: &AppHandle) -> Result<(), Error> {
        UserPrefs::forget()?;
        {
            let mut project_lock = self.project_info.lock().unwrap();
            project_lock.clear();
        }
        {
            let user_prefs = ApiInfo::default();
            *self.api_info.lock().unwrap() = user_prefs;
        }
        self.set_loading_state(LoadingState::NotStarted, app_handle);
        Ok(())
    }

    pub fn update_project_info(&self, project_info: &ProjectInfo) {
        let mut project_lock = self.project_info.lock().unwrap();
        if project_lock.contains_key(&project_info.id) {
            let existing_project = project_lock.get_mut(&project_info.id).unwrap();
            existing_project.update_project_info(project_info).unwrap();
        } else {
            let new_project = ProjectManager::initialize_from_info(project_info.clone());
            project_lock.insert(project_info.id, new_project);
        }
    }

    pub fn set_active_project(&self, project_id: Option<Uuid>, app_handle: &AppHandle) {
        *self.active_project.lock().unwrap() = project_id;
        self.emit_app_state_change(app_handle);
    }

    pub fn get_active_project(&self) -> Option<uuid::Uuid> {
        *self.active_project.lock().unwrap()
    }

    pub fn emit_app_state_change(&self, app_handle: &AppHandle) {
        let loading_state = self.loading_state();
        let project_info = self
            .project_info
            .lock()
            .unwrap()
            .values()
            .map(|p| p.get_ui_status())
            .collect();
        let active_project_id = self.get_active_project();
        let ui_state = UiState::new(loading_state, project_info, active_project_id);
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

    fn load_user_preferences(&self, app_handle: &AppHandle) -> UserPrefs {
        info!("Loading user preferences");
        let user_prefs = UserPrefs::load().unwrap_or_default();
        self.update_user_prefs(user_prefs.clone(), app_handle)
            .unwrap_or_else(|e| {
                log::warn!("Failed to save loaded user preferences: {}", e);
            });
        user_prefs
    }

    async fn authenticate_user(&self, app_handle: &AppHandle) -> Result<(), String> {
        let api_info = self.api_info();
        info!("Authenticating user");
        let Some(token) = api_info.oauth_token() else {
            log::warn!("No OAuth token found in user preferences");
            return Err("No OAuth token found".to_string());
        };
        match api::auth::authorize_with_token(api_info.instance(), &token).await {
            Ok(api_info) => {
                log::info!("User authenticated successfully");
                let prefs = UserPrefs::new(api_info);
                if self.update_user_prefs(prefs, app_handle).is_err() {
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
        let sec_delay = 0;
        match loading_state {
            LoadingState::NotStarted => {
                self.set_loading_state(LoadingState::CheckingForUpdates, app_handle)
            }
            LoadingState::CheckingForUpdates => match self.check_for_updates().await {
                Ok(update_available) => {
                    if update_available {
                        log::info!("Update available, upating...");
                        tokio::time::sleep(std::time::Duration::from_secs(sec_delay)).await;
                        self.set_loading_state(LoadingState::Updating, app_handle)
                    } else {
                        log::info!("No updates available, attempting to load user preferences");
                        tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                        self.set_loading_state(LoadingState::LoadingPrefs, app_handle)
                    }
                }
                Err(e) => {
                    log::warn!("Failed to check for updates: {}", e);
                    tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::Failed(e), app_handle)
                }
            },
            LoadingState::Updating => match self.update_app().await {
                Ok(_) => {
                    log::info!("Update successful, loading user preferences");
                    tokio::time::sleep(std::time::Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::LoadingPrefs, app_handle)
                }
                Err(e) => {
                    log::warn!("Failed to update application: {}", e);
                    tokio::time::sleep(std::time::Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::LoadingPrefs, app_handle)
                }
            },
            LoadingState::LoadingPrefs => {
                let prefs = self.load_user_preferences(app_handle);
                if let Some(_token) = prefs.api_info().oauth_token() {
                    info!("User prefs found, attempting to authenticate user");
                    tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::Authenticating, app_handle)
                } else {
                    info!("No user prefs found, starting unauthenticated");
                    tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::Unauthenticated, app_handle)
                }
            }
            LoadingState::Authenticating => {
                tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                match self.authenticate_user(app_handle).await {
                    Ok(_) => self.set_loading_state(LoadingState::LoadingProjects, app_handle),
                    Err(e) => {
                        // TODO:: Handle different authentication errors appropriately
                        log::warn!("Authentication failed: {}", e);
                        self.set_loading_state(LoadingState::Unauthenticated, app_handle)
                    }
                }
            }
            LoadingState::LoadingProjects => {
                tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                match self.load_user_projects().await {
                    Ok(_) => {
                        log::info!("User projects loaded successfully, app is ready");
                        self.set_loading_state(LoadingState::Ready, app_handle)
                    }
                    Err(e) => {
                        log::warn!("Failed to load user projects: {}", e);
                        self.set_loading_state(LoadingState::Failed(e), app_handle)
                    }
                }
            }
            _ => {
                log::info!("App already initialized or in progress");
                loading_state
            }
        }
    }
}
