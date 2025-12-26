use common::{ApiInfo, Error};
use log::{info, warn};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};
use url::Url;

use crate::paths::COMPASS_HOME_DIR;
#[cfg(test)]
const USER_PREFS_FILE_NAME: &str = "user_prefs_test.json";
#[cfg(not(test))]
const USER_PREFS_FILE_NAME: &str = "user_prefs.json";

/// Lazily-initialized full path to the user preferences file (COMPASS_HOME_DIR + USER_PREFS_FILE_NAME).
///
/// This is a runtime-initialized static because the user's home directory is not known at compile time.
static USER_PREFS_FILE_PATH_BUFFER: LazyLock<PathBuf> = LazyLock::new(|| {
    let mut path = COMPASS_HOME_DIR.clone();
    path.push(USER_PREFS_FILE_NAME);
    path
});

pub fn user_prefs_file_path() -> &'static Path {
    &USER_PREFS_FILE_PATH_BUFFER
}

/// User preferences structure
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserPrefs {
    api_info: ApiInfo,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            api_info: ApiInfo::default(),
        }
    }
}

impl UserPrefs {
    pub fn new(api_info: ApiInfo) -> Self {
        Self { api_info }
    }

    pub fn load() -> Result<Self, Error> {
        // Try to get credentials from environment variables first (for testing)
        let instance = std::env::var("TEST_SPELEODB_INSTANCE").ok();
        let oauth = std::env::var("TEST_SPELEODB_OAUTH").ok();
        if let Some(instance) = instance
            && let Some(oauth_token) = oauth
        {
            info!("User preferences loaded from environment variables");
            let instance = Url::parse(&instance).map_err(|e| {
                Error::Deserialization(format!("Invalid URL in TEST_SPELEODB_INSTANCE: {}", e))
            })?;
            return Ok(UserPrefs::new(ApiInfo::new(instance, Some(oauth_token))));
        }
        if user_prefs_file_path().exists() {
            let user_preferences_string = std::fs::read_to_string(user_prefs_file_path())
                .map_err(|_| Error::ApiInfoRead(user_prefs_file_path().to_path_buf()))?;
            let prefs: UserPrefs = toml::from_str(&user_preferences_string)
                .map_err(|e| Error::Deserialization(e.to_string()))?;
            info!("User preferences loaded successfully");
            Ok(prefs)
        } else {
            warn!("No user preferences found");
            Err(Error::NoUserPreferences)
        }
    }

    pub fn api_info(&self) -> &ApiInfo {
        &self.api_info
    }

    /// Save a user preferences object to disk in TOML format.
    pub fn save(&self) -> Result<(), Error> {
        let s = toml::to_string_pretty(self).map_err(|_| Error::Serialization)?;
        std::fs::write(user_prefs_file_path(), s)
            .map_err(|_| Error::ApiInfoWrite(user_prefs_file_path().to_path_buf()))?;

        // On Unix, tighten permissions so only the owner can read/write the prefs file.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = std::fs::metadata(user_prefs_file_path()) {
                let mut perms = meta.permissions();
                // rw------- (owner read/write)
                perms.set_mode(0o600);
                std::fs::set_permissions(user_prefs_file_path(), perms)
                    .map_err(|_| Error::FilePermissionSet)?;
            }
        }

        // Log the successful save with full path so the frontend/devs can verify persistence.
        log::info!(
            "Preferences successfully saved in {}",
            user_prefs_file_path().display()
        );

        Ok(())
    }

    pub fn forget() -> Result<(), Error> {
        if user_prefs_file_path().exists() {
            std::fs::remove_file(user_prefs_file_path())
                .map_err(|_| Error::ApiInfoWrite(user_prefs_file_path().to_path_buf()))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ensure_app_dir_exists;

    #[test]
    #[ignore]
    fn test_save_and_load_user_prefs() {
        // Ensure directory exists and clear any existing preferences
        ensure_app_dir_exists().expect("App dir created successfully");
        UserPrefs::forget().expect("Successfully delete user prefs file");
        const INSTANCE_URL: &str = "https://test.example.com";
        const OAUTH_TOKEN: &str = "0123456789abcdef0123456789abcdef01234567";
        let instance_url = Url::parse(INSTANCE_URL).unwrap();
        // Create test preferences
        let prefs = UserPrefs::new(ApiInfo::new(
            instance_url.clone(),
            Some(OAUTH_TOKEN.to_string()),
        ));

        // Save preferences
        let save_result = prefs.save();
        assert!(
            save_result.is_ok(),
            "save_user_prefs should succeed: {:?}",
            save_result
        );

        // Load preferences
        let loaded = UserPrefs::load().expect("Expected to load user prefs");
        let api_info = loaded.api_info();
        assert_eq!(api_info.instance(), &instance_url);
        assert_eq!(api_info.oauth_token().unwrap(), OAUTH_TOKEN);
    }

    #[test]
    #[ignore]
    fn test_forget_user_prefs() {
        // Ensure directory exists
        let _ = ensure_app_dir_exists();

        // Create and save test preferences
        let prefs = UserPrefs::default();
        let _ = prefs.save();

        // Forget preferences
        UserPrefs::forget().expect("forget_user_prefs should succeed");

        // Try to load - should get None
        let Err(Error::NoUserPreferences) = UserPrefs::load() else {
            panic!("Should return error loading prefs when preferences are deleted");
        };
    }

    #[test]
    #[ignore]
    fn test_forget_user_prefs_when_none_exist() {
        // Should not error even if file doesn't exist
        let result = UserPrefs::forget();
        assert!(
            result.is_ok(),
            "forget_user_prefs should succeed even if file doesn't exist"
        );
    }

    #[test]
    #[ignore]
    fn test_load_user_prefs_when_none_exist() {
        // Delete prefs first
        let _ = UserPrefs::forget();

        let result = UserPrefs::load();
        let Err(Error::NoUserPreferences) = result else {
            panic!("Should return None when no preferences exist");
        };
    }

    #[cfg(unix)]
    #[test]
    fn test_save_user_prefs_sets_permissions() {
        use std::os::unix::fs::PermissionsExt;

        // Ensure directory exists
        let _ = ensure_app_dir_exists();

        // Save preferences
        let prefs = UserPrefs::default();
        let _ = prefs.save();

        // Check file permissions
        let metadata = std::fs::metadata(&user_prefs_file_path())
            .expect("Should be able to read file metadata");
        let permissions = metadata.permissions();
        let mode = permissions.mode();

        // Check that only owner has read/write (0o600 = 384 in decimal)
        assert_eq!(
            mode & 0o777,
            0o600,
            "File should have 0o600 permissions (owner read/write only)"
        );
    }
}
