use std::{env::VarError, sync::Mutex};

use speleodb_compass_common::UserPrefs;

use crate::API_BASE_URL;

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
    pub fn from_env() -> Result<Self, VarError> {
        let instance = std::env::var("TEST_SPELEODB_INSTANCE")?;
        let oauth = std::env::var("TEST_SPELEODB_OAUTH")?;
        Ok(Self {
            instance: Mutex::new(instance),
            token: Mutex::new(Some(oauth)),
        })
    }

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

    pub fn get_api_token(&self) -> Result<String, String> {
        let token_lock = self.token.lock().unwrap();
        match &*token_lock {
            Some(token) => Ok(token.clone()),
            None => Err("No API token set".to_string()),
        }
    }

    pub fn reset(&self) {
        let mut instance_lock = self.instance.lock().unwrap();
        let mut token_lock = self.token.lock().unwrap();
        *instance_lock = API_BASE_URL.to_string();
        *token_lock = None;
    }
}
