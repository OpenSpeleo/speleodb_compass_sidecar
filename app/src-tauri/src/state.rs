use crate::API_BASE_URL;
use common::{Error, UserPrefs, api_types::ProjectRevisionInfo};
#[cfg(test)]
use std::env::VarError;
use std::{collections::HashMap, sync::Mutex};

pub struct ApiInfo {
    instance: Mutex<String>,
    token: Mutex<Option<String>>,
}

impl Default for ApiInfo {
    fn default() -> Self {
        Self {
            instance: Mutex::new(API_BASE_URL.to_string()),
            token: Mutex::new(None),
        }
    }
}

impl ApiInfo {
    pub fn set(&self, user_prefs: &UserPrefs) {
        let mut instance_lock = self.instance.lock().unwrap();
        let mut token_lock = self.token.lock().unwrap();
        *instance_lock = user_prefs.instance.clone();
        *token_lock = user_prefs.oauth_token.clone();
    }

    pub fn get_api_instance(&self) -> String {
        let instance_lock = self.instance.lock().unwrap();
        instance_lock.clone()
    }

    pub fn get_api_token(&self) -> Result<String, Error> {
        let token_lock = self.token.lock().unwrap();
        match &*token_lock {
            Some(token) => Ok(token.clone()),
            None => Err(Error::NoAuthToken),
        }
    }

    pub fn reset(&self) {
        let mut instance_lock = self.instance.lock().unwrap();
        let mut token_lock = self.token.lock().unwrap();
        *instance_lock = API_BASE_URL.to_string();
        *token_lock = None;
    }

    #[cfg(test)]
    pub fn from_env() -> Result<Self, VarError> {
        let instance = std::env::var("TEST_SPELEODB_INSTANCE")?;
        let oauth = std::env::var("TEST_SPELEODB_OAUTH")?;
        Ok(Self {
            instance: Mutex::new(instance),
            token: Mutex::new(Some(oauth)),
        })
    }
}

pub struct ProjectInfoManager {
    project_info: Mutex<HashMap<uuid::Uuid, ProjectRevisionInfo>>,
}

impl ProjectInfoManager {
    pub fn new() -> Self {
        Self {
            project_info: Mutex::new(HashMap::new()),
        }
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
