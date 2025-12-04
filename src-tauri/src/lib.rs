mod api;
mod commands;

use commands::{
    acquire_project_mutex, clear_active_project, create_project, download_project_zip,
    fetch_projects, forget_user_prefs, load_user_prefs, native_auth_request, open_project_folder,
    open_with_compass, release_project_mutex, save_user_prefs, select_zip_file, set_active_project,
    unzip_project, upload_project_zip, zip_project_folder,
};
use log::error;
use speleodb_compass_common::{compass_home, UserPrefs};

// Global state for active project
lazy_static::lazy_static! {
    static ref ACTIVE_PROJECT_ID: std::sync::Arc<std::sync::Mutex<Option<String>>> = std::sync::Arc::new(std::sync::Mutex::new(None));
}

async fn release_project_mutex_internal(project_id: &str) {
    use reqwest::Client;
    use std::time::Duration;

    log::info!("Releasing project mutex for project: {}", project_id);

    // Load user prefs
    let prefs = match UserPrefs::load() {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to load user preferences for mutex release: {}", e);
            return;
        }
    };

    let prefs = match prefs {
        Some(p) => p,
        _ => {
            error!("No user preferences found for mutex release");
            return;
        }
    };

    let oauth = match prefs.oauth_token {
        Some(t) => t,
        _ => {
            log::warn!("No OAuth token found in user preferences for mutex release");
            return;
        }
    };
    let base = prefs.instance.trim_end_matches('/');
    let url = format!("{}/api/v1/projects/{}/release/", base, project_id);

    let client = match Client::builder().timeout(Duration::from_secs(5)).build() {
        Ok(c) => c,
        Err(e) => {
            log::warn!("Failed to build HTTP client for mutex release: {}", e);
            return;
        }
    };

    // Fire and forget
    match client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            if status.is_success() {
                log::info!("Successfully released mutex for project: {}", project_id);
            } else {
                log::warn!("Mutex release returned status {}: {}", status.as_u16(), url);
            }
        }
        Err(e) => {
            log::warn!("Failed to release mutex (network error): {}", e);
        }
    }
}

fn parse_token_from_json(v: &serde_json::Value) -> Option<String> {
    // Check if the response has a "success" field that's false
    if let Some(success) = v.get("success").and_then(|v| v.as_bool()) {
        if !success {
            return None;
        }
    }

    let candidates = ["token", "auth_token", "access_token", "token_value", "auth"];
    for key in &candidates {
        if let Some(val) = v.get(*key) {
            if let Some(s) = val.as_str() {
                if !s.is_empty() {
                    return Some(s.to_string());
                }
            }
            if val.is_object() {
                if let Some(s) = val.get("value").and_then(|vv| vv.as_str()) {
                    if !s.is_empty() {
                        return Some(s.to_string());
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // Load .env file before running tests
    #[ctor::ctor]
    fn init() {
        // Try to load .env from workspace root (two levels up from src-tauri/src)
        if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
            let workspace_root = std::path::Path::new(&manifest_dir).parent().unwrap();
            let env_path = workspace_root.join(".env");
            if env_path.exists() {
                let _ = dotenvy::from_path(&env_path);
            }
        }
        // Fallback to current directory
        let _ = dotenvy::dotenv();
    }

    // Helper function to ensure test env vars are loaded. Returns true if loaded, false otherwise.
    fn ensure_test_env_vars() -> bool {
        // If env vars are missing, try to reload from .env
        if std::env::var("TEST_SPELEODB_INSTANCE").is_err()
            || std::env::var("TEST_SPELEODB_OAUTH").is_err()
        {
            if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
                let workspace_root = std::path::Path::new(&manifest_dir).parent().unwrap();
                let env_path = workspace_root.join(".env");
                if env_path.exists() {
                    let _ = dotenvy::from_path(&env_path);
                }
            }
        }

        std::env::var("TEST_SPELEODB_INSTANCE").is_ok()
            && std::env::var("TEST_SPELEODB_OAUTH").is_ok()
    }

    #[test]
    fn parse_token_top_level_string() {
        let v: serde_json::Value = serde_json::json!({"token": "abc123"});
        assert_eq!(parse_token_from_json(&v), Some("abc123".to_string()));
    }

    #[test]
    fn parse_token_nested_value() {
        let v: serde_json::Value = serde_json::json!({"token": {"value": "nested"}});
        assert_eq!(parse_token_from_json(&v), Some("nested".to_string()));
    }

    #[test]
    fn parse_no_token() {
        let v: serde_json::Value = serde_json::json!({"ok": true});
        assert_eq!(parse_token_from_json(&v), None);
    }

    #[test]
    fn parse_token_auth_token_field() {
        let v: serde_json::Value = serde_json::json!({"auth_token": "myauth"});
        assert_eq!(parse_token_from_json(&v), Some("myauth".to_string()));
    }

    #[test]
    fn parse_token_access_token_field() {
        let v: serde_json::Value = serde_json::json!({"access_token": "access123"});
        assert_eq!(parse_token_from_json(&v), Some("access123".to_string()));
    }

    #[test]
    fn parse_token_empty_string() {
        let v: serde_json::Value = serde_json::json!({"token": ""});
        assert_eq!(parse_token_from_json(&v), None);
    }

    #[test]
    fn parse_token_success_false() {
        let v: serde_json::Value = serde_json::json!({"success": false, "token": "shouldnotparse"});
        assert_eq!(parse_token_from_json(&v), None);
    }

    #[test]
    fn parse_token_priority_order() {
        // Should return "token" field first if multiple fields exist
        let v: serde_json::Value = serde_json::json!({"token": "first", "auth_token": "second"});
        assert_eq!(parse_token_from_json(&v), Some("first".to_string()));
    }

    #[tokio::test]
    #[serial]
    async fn native_auth_request_with_real_oauth() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        let instance = std::env::var("TEST_SPELEODB_INSTANCE").unwrap();
        let oauth = std::env::var("TEST_SPELEODB_OAUTH").unwrap();

        let res = native_auth_request(
            String::new(),
            String::new(),
            oauth.clone(),
            instance.clone(),
        )
        .await;

        assert!(
            res.get("ok").and_then(|v| v.as_bool()) == Some(true),
            "Auth should succeed, got: {:?}",
            res
        );
        assert!(res.get("token").is_some(), "Should return a token");
    }

    #[tokio::test]
    async fn native_auth_request_empty_instance() {
        let res = native_auth_request(
            "u".to_string(),
            "p".to_string(),
            String::new(),
            "".to_string(),
        )
        .await;
        assert!(res.get("ok").and_then(|v| v.as_bool()) == Some(false));
        assert!(res
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap()
            .contains("empty"));
    }

    #[tokio::test]
    #[serial]
    async fn native_auth_request_with_invalid_oauth() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        let instance = std::env::var("TEST_SPELEODB_INSTANCE").unwrap();

        let res = native_auth_request(
            String::new(),
            String::new(),
            "invalidtoken1234567890123456789012345".to_string(),
            instance,
        )
        .await;
        assert!(res.get("ok").and_then(|v| v.as_bool()) == Some(false));
        // Should fail with authentication error
    }

    #[tokio::test]
    #[serial]
    async fn fetch_projects_with_real_api() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        let _instance = std::env::var("TEST_SPELEODB_INSTANCE").unwrap();
        let _oauth = std::env::var("TEST_SPELEODB_OAUTH").unwrap();

        // Fetch projects from real API (uses env vars directly)
        let result = fetch_projects().await;

        // Verify response structure
        assert!(
            result.get("success").and_then(|v| v.as_bool()) == Some(true),
            "API call should succeed, got: {:?}",
            result
        );
        assert!(
            result.get("data").and_then(|v| v.as_array()).is_some(),
            "Response should have data array"
        );
    }

    #[tokio::test]
    #[serial]
    async fn fetch_projects_with_invalid_token() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        // Save the valid oauth token
        let valid_oauth = std::env::var("TEST_SPELEODB_OAUTH").unwrap();

        let instance = std::env::var("TEST_SPELEODB_INSTANCE").unwrap();

        // Temporarily set environment variables with invalid token
        std::env::set_var("TEST_SPELEODB_INSTANCE", &instance);
        std::env::set_var(
            "TEST_SPELEODB_OAUTH",
            "0000000000000000000000000000000000000000",
        );

        let result = fetch_projects().await;

        // Restore the valid token immediately
        std::env::set_var("TEST_SPELEODB_OAUTH", valid_oauth);

        // The response should indicate failure (likely 401 or 403)
        assert!(
            result.get("ok").and_then(|v| v.as_bool()) == Some(false)
                || result.get("success").is_none(),
            "Should indicate failure with invalid token, got: {:?}",
            result
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ensure the hidden application directory exists in the user's home directory.
    if let Err(e) = speleodb_compass_common::ensure_app_dir_exists() {
        eprintln!(
            "Failed to create application directory '{:?}': {:#}",
            compass_home(),
            e
        );
    }
    // This should be called as early in the execution of the app as possible
    #[cfg(debug_assertions)] // only enable instrumentation in development builds
    let devtools = tauri_plugin_devtools::init();

    // Initialize logging
    let _ = speleodb_compass_common::init_file_logger("info");

    if let Ok(path) = std::env::current_dir() {
        log::info!("Current working directory: {}", path.display());
    }

    // Log where we are logging to
    if compass_home().exists() {
        log::info!("Application starting. Logging to: {:?}", compass_home());
    }

    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            acquire_project_mutex,
            clear_active_project,
            download_project_zip,
            fetch_projects,
            forget_user_prefs,
            load_user_prefs,
            native_auth_request,
            open_project_folder,
            open_with_compass,
            release_project_mutex,
            save_user_prefs,
            select_zip_file,
            set_active_project,
            unzip_project,
            upload_project_zip,
            zip_project_folder,
            create_project,
        ]);
    #[cfg(debug_assertions)]
    {
        builder = builder.plugin(devtools);
    }
    builder
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(project_id) = ACTIVE_PROJECT_ID.lock().unwrap().as_ref() {
                    log::info!(
                        "App exit requested, releasing mutex for project: {}",
                        project_id
                    );
                    let runtime = tokio::runtime::Runtime::new().unwrap();
                    runtime.block_on(async {
                        release_project_mutex_internal(project_id).await;
                    });
                }
            }
        });
}
