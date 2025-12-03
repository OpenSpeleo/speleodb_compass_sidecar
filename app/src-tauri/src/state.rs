use common::{Error, UserPrefs, api_types::ProjectRevisionInfo};
use std::{collections::HashMap, sync::Mutex};

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
