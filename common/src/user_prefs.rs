use crate::{COMPASS_HOME_DIR, Error};
use log::info;
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};
use url::Url;
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
// TODO:: Add newtype to encapsulate Oauth token validaiton
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct OauthToken(String);

impl AsRef<str> for OauthToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct UserPrefs {
    instance: Url,
    oauth_token: Option<String>,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            instance: Url::parse("https://speleodb.com").unwrap(),
            oauth_token: None,
        }
    }
}

impl UserPrefs {
    pub fn new(instance: Url, oauth_token: Option<String>) -> Self {
        Self {
            instance,
            oauth_token,
        }
    }

    pub fn load() -> Result<Option<Self>, Error> {
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
            return Ok(Some(UserPrefs {
                instance: instance,
                oauth_token: Some(oauth_token),
            }));
        }
        if user_prefs_file_path().exists() {
            let s = std::fs::read_to_string(user_prefs_file_path())
                .map_err(|_| Error::UserPrefsRead(user_prefs_file_path().to_path_buf()))?;
            let s: UserPrefs =
                toml::from_str(&s).map_err(|e| Error::Deserialization(e.to_string()))?;
            info!("User preferences loaded successfully");
            Ok(Some(s))
        } else {
            info!("No user preferences found");
            Ok(None)
        }
    }

    #[cfg(test)]
    pub fn from_env() -> Result<Self, Error> {
        let instance = std::env::var("TEST_SPELEODB_INSTANCE").ok();
        let oauth = std::env::var("TEST_SPELEODB_OAUTH").ok();
        if let Some(instance) = instance
            && let Some(oauth_token) = oauth
        {
            let instance = Url::parse(&instance).map_err(|e| {
                Error::Deserialization(format!("Invalid URL in TEST_SPELEODB_INSTANCE: {}", e))
            })?;
            Ok(UserPrefs {
                instance: instance,
                oauth_token: Some(oauth_token),
            })
        } else {
            Err(Error::NoAuthToken)
        }
    }

    pub fn instance(&self) -> &Url {
        &self.instance
    }

    pub fn oauth_token(&self) -> Option<&str> {
        self.oauth_token.as_deref()
    }

    /// Save a user preferences object to disk in TOML format.
    pub fn save(prefs: &Self) -> Result<(), Error> {
        let s = toml::to_string_pretty(&prefs).map_err(|_| Error::Serialization)?;
        std::fs::write(user_prefs_file_path(), s)
            .map_err(|_| Error::UserPrefsWrite(user_prefs_file_path().to_path_buf()))?;

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
                .map_err(|_| Error::UserPrefsWrite(user_prefs_file_path().to_path_buf()))?;
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
        let prefs = UserPrefs {
            instance: instance_url.clone(),
            oauth_token: Some(OAUTH_TOKEN.to_string()),
        };

        // Save preferences
        let save_result = UserPrefs::save(&prefs);
        assert!(
            save_result.is_ok(),
            "save_user_prefs should succeed: {:?}",
            save_result
        );

        // Load preferences
        let load_result = UserPrefs::load().expect("Expected to load user prefs");
        let loaded = load_result.expect("User prefs should be 'Some'");
        assert_eq!(loaded.instance, instance_url);
        assert_eq!(loaded.oauth_token.unwrap(), OAUTH_TOKEN);
    }

    #[test]
    #[ignore]
    fn test_forget_user_prefs() {
        // Ensure directory exists
        let _ = ensure_app_dir_exists();

        // Create and save test preferences
        let prefs = UserPrefs::default();
        let _ = UserPrefs::save(&prefs);

        // Forget preferences
        UserPrefs::forget().expect("forget_user_prefs should succeed");

        // Try to load - should get None
        let load_result = UserPrefs::load().expect("Expected no errors loading prefs");
        assert!(load_result.is_none(), "Preferences should be deleted");
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

        let result = UserPrefs::load().expect("Loading should not fail, even with no prefs set");
        assert!(
            result.is_none(),
            "Should return None when no preferences exist"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_save_user_prefs_sets_permissions() {
        use std::os::unix::fs::PermissionsExt;

        // Ensure directory exists
        let _ = ensure_app_dir_exists();

        // Save preferences
        let prefs = UserPrefs::default();
        let _ = UserPrefs::save(&prefs);

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
