// WASM controller now delegates network calls to native Tauri backend.
use crate::{Error, invoke};
use log::{error, info};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use speleodb_compass_common::{CompassProject, ProjectMetadata, UserPrefs};
use web_sys::Url;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ActiveMutex {
    pub user: String,
    pub creation_date: String,
    pub modified_date: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub is_active: bool,
    pub permission: String,
    pub active_mutex: Option<ActiveMutex>,
    pub country: String,
    pub created_by: String,
    pub creation_date: String,
    pub modified_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    pub fork_from: Option<String>,
    pub visibility: String,
    pub exclude_geojson: bool,
}

#[derive(Deserialize, Debug)]
struct ProjectsResponse {
    pub data: Vec<Project>,
    pub success: bool,
    // Ignore extra fields like timestamp and url
}

pub struct SpeleoDBController {}

impl SpeleoDBController {
    pub async fn fetch_projects(&self) -> Result<Vec<Project>, String> {
        // Call the Tauri backend to fetch projects
        #[derive(Serialize)]
        struct FetchProjectsArgs {}

        let args = FetchProjectsArgs {};

        let json: serde_json::Value = invoke("fetch_projects", &args).await.unwrap();

        // Check if it's an error response from our backend
        if json.get("ok").and_then(|v| v.as_bool()) == Some(false) {
            let err_msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to fetch projects");
            return Err(err_msg.to_string());
        }

        // Now try to deserialize to ProjectsResponse using serde_json
        let response: ProjectsResponse = serde_json::from_value(json)
            .map_err(|e| format!("Failed to parse API response: {}", e))?;

        if response.success {
            Ok(response.data)
        } else {
            Err("API returned success: false".to_string())
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
        let oauth_ok = oauth.is_some_and(|oauth| validate_oauth(&oauth));
        let pass_ok = email.is_some_and(|email| {
            password.is_some_and(|password| validate_email_password(&email, &password))
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

    pub async fn acquire_project_mutex(&self, project_id: &str) -> Result<bool, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }

        let args = Args { project_id };

        let json: serde_json::Value = invoke("acquire_project_mutex", &args).await.unwrap();

        // Check if operation was successful
        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let msg = json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to acquire mutex");
            return Err(msg.to_string());
        }

        // Return whether the mutex was locked
        let locked = json
            .get("locked")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(locked)
    }

    pub async fn download_project(&self, project_id: &str) -> Result<String, String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }

        let args = Args { project_id };

        let json: serde_json::Value = invoke("download_project_zip", &args).await.unwrap();

        if json.get("ok").and_then(|v| v.as_bool()) != Some(true) {
            let err_msg = json
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("Failed to download project");

            // Include URL if available for debugging
            let debug_info = if let Some(url) = json.get("url").and_then(|v| v.as_str()) {
                format!("{} (URL: {})", err_msg, url)
            } else {
                err_msg.to_string()
            };

            return Err(debug_info);
        }

        // Check if this is an empty project (422 response)
        if json.get("empty_project").and_then(|v| v.as_bool()) == Some(true) {
            // Return a special error that the frontend can detect
            return Err("EMPTY_PROJECT_422".to_string());
        }

        let path = json
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or("No path in response")?;

        Ok(path.to_string())
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

    pub async fn open_folder(&self, project_id: &str) -> Result<(), String> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args<'a> {
            project_id: &'a str,
        }

        let args = Args { project_id };

        let json: serde_json::Value = invoke("open_project_folder", &args).await.unwrap();

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

    pub async fn import_compass_project(
        &self,
        project_metadata: ProjectMetadata,
    ) -> Result<CompassProject, Error> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Args {
            project_metadata: ProjectMetadata,
        }
        let args = Args { project_metadata };
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
        let oauth_ok = oauth.is_some_and(|token| validate_oauth(&token));
        let pass_ok = email.is_some_and(|email| {
            password.is_some_and(|password| validate_email_password(&email, &password))
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
    ) -> Result<Project, String> {
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
        let project: Project = serde_json::from_value(project_data.clone())
            .map_err(|e| format!("Failed to parse project data: {}", e))?;

        Ok(project)
    }
}

/// Try to parse an auth token from a JSON response body. Checks several common field names.
// parse_auth_token removed; token parsing handled in native backend

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

    // Prefs struct tests
    #[test]
    fn prefs_serialization() {
        let prefs = UserPrefs {
            instance: "https://test.com".to_string(),
            email: None,
            password: None,
            oauth_token: Some("0123456789abcdef0123456789abcdef01234567".to_string()),
        };

        let json = serde_json::to_string(&prefs).unwrap();
        assert!(json.contains("https://test.com"));
        assert!(json.contains("0123456789abcdef0123456789abcdef01234567"));
    }

    #[test]
    fn prefs_deserialization() {
        let json = r#"{"instance":"https://test.com","oauth_token":"token123"}"#;
        let prefs: UserPrefs = serde_json::from_str(json).unwrap();

        assert_eq!(prefs.instance, "https://test.com");
        assert_eq!(prefs.oauth_token, Some("token123".to_string()));
    }

    // Project struct tests
    #[test]
    fn project_deserialization_with_null_mutex() {
        let json = r#"{
            "id": "123",
            "name": "Test",
            "description": "Desc",
            "permission": "ADMIN",
            "active_mutex": null,
            "country": "US",
            "created_by": "user",
            "creation_date": "2025-01-01",
            "modified_date": "2025-01-02",
            "fork_from": null,
            "visibility": "PUBLIC",
            "exclude_geojson": false,
            "is_active": true,
            "latitude": null,
            "longitude": null
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();
        assert_eq!(project.name, "Test");
        assert!(project.active_mutex.is_none());
    }

    #[test]
    fn project_deserialization_with_string_mutex() {
        let json = r#"{
            "id": "123",
            "name": "Test",
            "description": "Desc",
            "permission": "READ_AND_WRITE",
            "active_mutex": {
                "user": "user@example.com",
                "creation_date": "2025-01-01",
                "modified_date": "2025-01-01"
            },
            "country": "US",
            "created_by": "user",
            "creation_date": "2025-01-01",
            "modified_date": "2025-01-02",
            "fork_from": null,
            "visibility": "PRIVATE",
            "exclude_geojson": true,
            "is_active": true,
            "latitude": null,
            "longitude": null
        }"#;

        let project: Project = serde_json::from_str(json).unwrap();
        assert_eq!(project.name, "Test");
        assert!(project.active_mutex.is_some());
        assert_eq!(
            project.active_mutex.as_ref().unwrap().user,
            "user@example.com"
        );
    }

    #[test]
    fn project_clone_works() {
        let project = Project {
            id: "1".to_string(),
            name: "Test".to_string(),
            description: "Desc".to_string(),
            permission: "ADMIN".to_string(),
            active_mutex: None,
            country: "US".to_string(),
            created_by: "user".to_string(),
            creation_date: "2025-01-01".to_string(),
            modified_date: "2025-01-02".to_string(),
            fork_from: None,
            visibility: "PUBLIC".to_string(),
            exclude_geojson: false,
            is_active: true,
            latitude: None,
            longitude: None,
        };

        let cloned = project.clone();
        assert_eq!(project.id, cloned.id);
        assert_eq!(project.name, cloned.name);
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
