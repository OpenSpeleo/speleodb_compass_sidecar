// WASM controller now delegates network calls to native Tauri backend.
use crate::{Error, invoke};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::Serialize;
use speleodb_compass_common::{CompassProject, api_types::ProjectInfo};
use uuid::Uuid;
use web_sys::Url;

pub struct SpeleoDBController {}

impl SpeleoDBController {
    pub async fn fetch_projects(&self) -> Result<Vec<ProjectInfo>, String> {
        // Call the Tauri backend to fetch projects
        #[derive(Serialize)]
        struct FetchProjectsArgs {}

        let args = FetchProjectsArgs {};

        match invoke("fetch_projects", &args).await {
            Ok(projects) => Ok(projects),
            Err(e) => {
                error!("Failed to fetch projects: {}", e);
                Err(e.to_string())
            }
        }
    }

    pub async fn authenticate(
        &self,
        email: Option<&str>,
        password: Option<&str>,
        oauth: Option<&str>,
        target_instance: &str,
    ) -> Result<(), String> {
        // Validate instance URL
        if Url::new(target_instance).is_err() {
            return Err("SpeleoDB instance must be a valid URL".into());
        }
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
            instance: &'a str,
        }

        let args = NativeArgs {
            email,
            password,
            oauth,
            instance: target_instance,
        };

        let _token: String = match invoke::<_, String>("auth_request", &args).await {
            Ok(token) => token,
            Err(e) => {
                error!("Authentication failed: {}", e);
                return Err(e.to_string());
            }
        };
        info!("{_token}");
        Ok(())
    }

    pub async fn acquire_project_mutex(&self, project_id: &str) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }
        info!("Acquiring mutex for project: {}", project_id);
        let args = Args { project_id };

        let _: () = invoke("acquire_project_mutex", &args)
            .await
            .map_err(|e| e.to_string())?;
        info!("Mutex acquired for project: {}", project_id);
        Ok(())
    }

    pub async fn update_project(&self, project_id: &str) -> Result<CompassProject, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }

        let args = Args { project_id };

        let project: CompassProject = invoke("update_project_index", &args)
            .await
            .map_err(|e| e.to_string())?;

        Ok(project)
    }

    pub async fn unzip_project(&self, zip_path: &str, project_id: &str) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            zip_path: &'a str,
            project_id: &'a str,
        }

        let args = Args {
            zip_path,
            project_id,
        };

        let json: serde_json::Value = invoke("unzip_project", &args).await.unwrap();

        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err_msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to unzip project");
            return Err(err_msg.to_string());
        }

        let path = json
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("No path in response")?;

        Ok(path.to_string())
    }

    pub async fn open_project(&self, project_id: Uuid) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            project_id: Uuid,
        }

        let args = Args { project_id };

        let json: serde_json::Value = invoke("open_project", &args).await.unwrap();

        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err_msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to open folder");
            return Err(err_msg.to_string());
        }

        Ok(())
    }

    pub async fn zip_project(&self, project_id: &str) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }

        let args = Args { project_id };

        let json: serde_json::Value = invoke("zip_project_folder", &args).await.unwrap();

        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err_msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to ZIP project");
            return Err(err_msg.to_string());
        }

        let path = json
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("No path in response")?;

        Ok(path.to_string())
    }

    pub async fn upload_project(
        &self,
        project_id: &str,
        message: &str,
        zip_path: &str,
    ) -> Result<u16, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
            commit_message: &'a str,
            zip_path: &'a str,
        }

        let args = Args {
            project_id,
            commit_message: message,
            zip_path,
        };

        let json: serde_json::Value = invoke("upload_project_zip", &args).await.unwrap();

        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err_msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to upload project");
            return Err(err_msg.to_string());
        }

        let status = json.get("status").and_then(|v| v.as_u64()).unwrap_or(200) as u16;
        Ok(status)
    }

    pub async fn release_mutex(&self, project_id: &str) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }

        let args = Args { project_id };

        let _json: serde_json::Value = invoke("release_project_mutex", &args).await.unwrap();
        Ok(())
    }

    pub async fn import_compass_project(&self, id: Uuid) -> Result<CompassProject, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            id: Uuid,
        }
        let args = Args { id };
        invoke("import_compass_project", &args).await
    }

    pub async fn set_active_project(&self, project_id: &str) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }
        let args = Args { project_id };
        let _: () = invoke("set_active_project", &args).await.unwrap();
        Ok(())
    }

    pub async fn clear_active_project(&self) -> Result<(), String> {
        let _: () = invoke("clear_active_project", &()).await.unwrap();
        Ok(())
    }

    /// Determine if auto-login should be attempted based on stored credentials.
    pub fn should_auto_login(
        &self,
        email: Option<&str>,
        password: Option<&str>,
        oauth: Option<&str>,
    ) -> bool {
        info!("email: {email:?}, password: {password:?}, oauth: {oauth:?}");
        let oauth_ok = oauth.is_some_and(validate_oauth);
        let pass_ok = email.is_some_and(|email| {
            password.is_some_and(|password| validate_email_password(email, password))
        });
        oauth_ok || pass_ok
    }

    pub async fn create_project(
        &self,
        name: &str,
        description: &str,
        country: &str,
        latitude: Option<&str>,
        longitude: Option<&str>,
    ) -> Result<ProjectInfo, String> {
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

        let json: serde_json::Value = invoke("create_project", &args).await.unwrap();

        // Check if operation was successful
        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to create project");
            return Err(msg.to_string());
        }

        // Extract project data
        let project_data = json.get("data").ok_or("No project data in response")?;

        // Deserialize to Project
        let project: ProjectInfo = serde_json::from_value(project_data.clone())
            .map_err(|e| format!("Failed to parse project data: {}", e))?;

        Ok(project)
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

    #[test]
    fn should_auto_login_oauth() {
        let controller = SpeleoDBController {};
        let valid_oauth = "0123456789abcdef0123456789abcdef01234567";
        assert!(controller.should_auto_login(None, None, Some(valid_oauth)));
    }

    #[test]
    fn should_auto_login_email_password() {
        let controller = SpeleoDBController {};
        assert!(controller.should_auto_login(Some("user@example.com"), Some("password"), None));
    }

    #[test]
    fn should_auto_login_fail_empty() {
        let controller = SpeleoDBController {};
        assert!(!controller.should_auto_login(None, None, None));
    }

    #[test]
    fn should_auto_login_fail_partial_email() {
        let controller = SpeleoDBController {};
        assert!(!controller.should_auto_login(Some("user@example.com"), None, None));
        assert!(!controller.should_auto_login(None, Some("password"), None));
    }

    #[test]
    fn should_auto_login_conflict_uses_oauth() {
        let controller = SpeleoDBController {};
        let valid_oauth = "0123456789abcdef0123456789abcdef01234567";
        // Should succeed and use OAuth if both are provided (OAuth takes precedence)
        assert!(controller.should_auto_login(
            Some("user@example.com"),
            Some("password"),
            Some(valid_oauth)
        ));
    }
}
