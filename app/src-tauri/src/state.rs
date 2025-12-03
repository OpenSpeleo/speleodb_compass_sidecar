use common::{Error, UserPrefs, api_types::ProjectRevisionInfo};
use std::{collections::HashMap, sync::Mutex};
use tauri::{AppHandle, Emitter, ipc::private::tracing::info};

pub struct AppState {
    api_info: Mutex<UserPrefs>,
    project_info: Mutex<HashMap<uuid::Uuid, ProjectRevisionInfo>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            api_info: Mutex::new(UserPrefs::default()),
            project_info: Mutex::new(HashMap::new()),
        }
    }

    pub async fn init_app_state(&self, app_handle: AppHandle) -> Result<(), Error> {
        info!("Attempting to load user preferences from disk");
        let prefs = UserPrefs::load().unwrap_or_default();
        if let Some(token) = prefs.oauth_token() {
            log::info!("User prefs found, attempting to authenticate user");
            match api::auth::authorize_with_token(prefs.instance(), token).await {
                Ok(_) => {
                    log::info!("User authenticated successfully");
                    app_handle.emit("event::authentication", true).unwrap();
                }
                Err(e) => {
                    log::warn!("Failed to authenticate user with saved token: {}", e);
                    app_handle.emit("event::authentication", false).unwrap();
                }
            }
        } else {
            log::info!("No user prefs found, starting unauthenticated");
            app_handle.emit("event::authentication", false).unwrap();
        }
        self.update_user_prefs(prefs)?;

        Ok(())
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

    pub fn update_project(&self, project_info: &ProjectRevisionInfo) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.insert(project_info.project.id, project_info.clone());
    }

    pub fn get_project(&self, project_id: uuid::Uuid) -> Option<ProjectRevisionInfo> {
        let project_lock = self.project_info.lock().unwrap();
        project_lock.get(&project_id).cloned()
    }

    pub fn clear_projects(&self) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.clear();
    }
}
