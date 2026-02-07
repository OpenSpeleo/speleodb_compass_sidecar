// WASM controller now delegates network calls to native Tauri backend.
use crate::{Error, invoke};
use common::ui_state::ProjectSaveResult;
use log::{error, info};
use once_cell::sync::Lazy;
use serde::Serialize;
use url::Url;
use uuid::Uuid;

/// Struct for invocations that require only a project ID.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectIdArgs {
    project_id: Uuid,
}

impl ProjectIdArgs {
    fn new(project_id: Uuid) -> Self {
        Self { project_id }
    }
}

pub struct SpeleoDBController {}

impl SpeleoDBController {
    pub async fn ensure_initialized(&self) {
        let _: () = invoke("ensure_initialized", &()).await.unwrap();
    }

    pub async fn authenticate(
        &self,
        email: Option<&str>,
        password: Option<&str>,
        oauth: Option<&str>,
        instance: &Url,
    ) -> Result<(), String> {
        // Validate instance URL

        info!("token: {oauth:?}");
        // Validation: either oauth token (40 hex) OR email+password
        let oauth_ok = oauth.is_some_and(validate_oauth);
        let pass_ok = email.is_some_and(|email| {
            password.is_some_and(|password| validate_email_password(email, password))
        });
        info!("Auth Ok: {oauth_ok}, Pass Ok: {pass_ok}");

        if !(oauth_ok ^ pass_ok) {
            return Err("Must provide exactly one auth method: either email+password or a 40-char OAUTH token".into());
        }

        // Use the native Tauri backend to perform the network request to avoid CORS and webview restrictions.
        #[derive(Serialize)]
        struct NativeArgs<'a> {
            email: Option<&'a str>,
            password: Option<&'a str>,
            oauth: Option<&'a str>,
            instance: &'a Url,
        }

        let args = NativeArgs {
            email,
            password,
            oauth,
            instance,
        };

        match invoke::<_, ()>("auth_request", &args).await {
            Ok(()) => Ok(()),
            Err(e) => {
                error!("Authentication failed: {}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn open_project(&self, project_id: Uuid) -> Result<(), String> {
        let args = ProjectIdArgs::new(project_id);
        let _: () = invoke("open_project", &args)
            .await
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    pub async fn save_project(
        &self,
        project_id: Uuid,
        commit_message: &str,
    ) -> Result<ProjectSaveResult, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: Uuid,
            commit_message: &'a str,
        }
        let args = Args {
            project_id,
            commit_message,
        };

        let result: ProjectSaveResult = invoke("save_project", &args)
            .await
            .map_err(|e| e.to_string())?;
        Ok(result)
    }

    pub async fn discard_changes(&self) -> Result<(), String> {
        invoke::<_, ()>("discard_changes", &())
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn import_compass_project(&self, id: Uuid) -> Result<(), Error> {
        let args = ProjectIdArgs::new(id);
        invoke("import_compass_project", &args).await
    }

    pub async fn set_active_project(&self, project_id: Uuid) -> Result<(), String> {
        let args = ProjectIdArgs::new(project_id);
        let _: () = invoke("set_active_project", &args).await.unwrap();
        Ok(())
    }

    pub async fn clear_active_project(&self) -> Result<(), String> {
        let _: () = invoke("clear_active_project", &()).await.unwrap();
        Ok(())
    }

    pub async fn create_project(
        &self,
        name: &str,
        description: &str,
        country: &str,
        latitude: Option<&str>,
        longitude: Option<&str>,
    ) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            name: &'a str,
            description: &'a str,
            country: &'a str,
            latitude: Option<&'a str>,
            longitude: Option<&'a str>,
        }

        let args = Args {
            name,
            description,
            country,
            latitude,
            longitude,
        };

        invoke("create_project", &args)
            .await
            .map_err(|e| e.to_string())
    }
}

pub static SPELEO_DB_CONTROLLER: Lazy<SpeleoDBController> = Lazy::new(|| SpeleoDBController {});

/// Validate an OAuth token: exactly 40 hex characters.
pub fn validate_oauth(oauth: &str) -> bool {
    oauth.len() == 40 && oauth.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validate email+password: email must contain a single `@` and a `.` in the domain and password non-empty.
pub fn validate_email_password(email: &str, password: &str) -> bool {
    if email.is_empty() || password.is_empty() {
        return false;
    }
    let parts: Vec<&str> = email.split('@').collect();
    parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    // OAuth validation tests
    #[test]
    fn oauth_valid_examples() {
        assert!(validate_oauth("0123456789abcdef0123456789abcdef01234567"));
        assert!(!validate_oauth(&"g".repeat(40)));
        assert!(!validate_oauth("short"));
    }

    #[test]
    fn oauth_uppercase_hex_valid() {
        assert!(validate_oauth("0123456789ABCDEF0123456789ABCDEF01234567"));
    }

    #[test]
    fn oauth_mixed_case_valid() {
        assert!(validate_oauth("0123456789aBcDeF0123456789AbCdEf01234567"));
    }

    #[test]
    fn oauth_invalid_length_too_short() {
        assert!(!validate_oauth("0123456789abcdef"));
    }

    #[test]
    fn oauth_invalid_length_too_long() {
        assert!(!validate_oauth("0123456789abcdef0123456789abcdef012345678"));
    }

    #[test]
    fn oauth_empty_string() {
        assert!(!validate_oauth(""));
    }

    #[test]
    fn oauth_non_hex_chars() {
        assert!(!validate_oauth("0123456789abcdef0123456789abcdef0123456g"));
        assert!(!validate_oauth("0123456789abcdef0123456789abcdef0123456 "));
        assert!(!validate_oauth("0123456789abcdef0123456789abcdef0123456-"));
    }

    #[test]
    fn oauth_all_zeros() {
        assert!(validate_oauth("0000000000000000000000000000000000000000"));
    }

    #[test]
    fn oauth_all_fs() {
        assert!(validate_oauth("ffffffffffffffffffffffffffffffffffffffff"));
    }

    // Email/password validation tests
    #[test]
    fn email_password_validation() {
        assert!(validate_email_password("user@example.com", "secret"));
        assert!(!validate_email_password("userexample.com", "secret"));
        assert!(!validate_email_password("user@localhost", "secret"));
        assert!(!validate_email_password("user@example.com", ""));
    }

    #[test]
    fn email_password_both_empty() {
        assert!(!validate_email_password("", ""));
    }

    #[test]
    fn email_password_empty_email() {
        assert!(!validate_email_password("", "password"));
    }

    #[test]
    fn email_password_empty_password() {
        assert!(!validate_email_password("user@example.com", ""));
    }

    #[test]
    fn email_missing_at_symbol() {
        assert!(!validate_email_password("userexample.com", "password"));
    }

    #[test]
    fn email_multiple_at_symbols() {
        assert!(!validate_email_password("user@@example.com", "password"));
        assert!(!validate_email_password("us@er@example.com", "password"));
    }

    #[test]
    fn email_missing_dot_in_domain() {
        assert!(!validate_email_password("user@localhost", "password"));
        assert!(!validate_email_password("user@domain", "password"));
    }

    #[test]
    fn email_valid_formats() {
        assert!(validate_email_password("user@example.com", "pass"));
        assert!(validate_email_password("user.name@example.com", "pass"));
        assert!(validate_email_password("user+tag@example.co.uk", "pass"));
        assert!(validate_email_password("user_name@sub.example.com", "pass"));
    }

    #[test]
    fn email_ending_with_at() {
        assert!(!validate_email_password("user@", "password"));
    }

    #[test]
    fn email_starting_with_at() {
        assert!(!validate_email_password("@example.com", "password"));
    }

    #[test]
    fn password_single_char() {
        assert!(validate_email_password("user@example.com", "x"));
    }

    #[test]
    fn password_long() {
        let long_pass = "a".repeat(1000);
        assert!(validate_email_password("user@example.com", &long_pass));
    }

    #[test]
    fn password_special_chars() {
        assert!(validate_email_password("user@example.com", "p@$$w0rd!"));
        assert!(validate_email_password("user@example.com", "🔒secure"));
    }
}
