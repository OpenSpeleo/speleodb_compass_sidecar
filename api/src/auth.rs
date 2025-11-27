use serde::Deserialize;
use serde_json::json;

use crate::get_api_client;

async fn handle_auth_response(response: reqwest::Response) -> Result<String, String> {
    let status = response.status();
    #[derive(Deserialize)]
    struct TokenResponse {
        token: String,
    }
    if status.is_success() {
        // Fall back to JSON body parsing
        let token = response
            .json::<TokenResponse>()
            .await
            .map_err(|e| format!("Unexpected response body: {e}"))?;
        return Ok(token.token);
    } else {
        Err(format!("Authorization failed with status: {}", status))
    }
}

pub async fn authorize_with_token(instance: &str, oauth: &str) -> Result<String, String> {
    let url = format!("{}{}", instance, "/api/v1/user/auth-token/");
    let client = get_api_client();

    let response = client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;
    handle_auth_response(response).await
}

pub async fn authorize_with_email(
    instance: &str,
    email: &str,
    password: &str,
) -> Result<String, String> {
    let client = get_api_client();
    let url = format!("{}{}", instance, "/api/v1/user/auth-token/");
    let body = json!({"email": email, "password": password});
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;
    handle_auth_response(response).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api_info::ApiInfo, project::fetch_projects};
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

    #[tokio::test]
    #[serial]
    async fn auth_request_with_real_oauth() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        let instance = std::env::var("TEST_SPELEODB_INSTANCE").unwrap();
        let oauth = std::env::var("TEST_SPELEODB_OAUTH").unwrap();

        authorize_with_token(&instance, &oauth).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn fetch_projects_with_real_api() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        let api_info = ApiInfo::from_env().unwrap();

        // Fetch projects from real API (uses env vars directly)
        let _result = fetch_projects(&api_info).await.unwrap();
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
        unsafe {
            std::env::set_var("TEST_SPELEODB_INSTANCE", &instance);
            std::env::set_var(
                "TEST_SPELEODB_OAUTH",
                "0000000000000000000000000000000000000000",
            ); // Fetch projects from real API (uses env vars directly)
        }
        let api_info = ApiInfo::from_env().unwrap();
        let _result = fetch_projects(&api_info)
            .await
            .expect_err("This shouldn't work");
        unsafe {
            // Restore the valid token immediately
            std::env::set_var("TEST_SPELEODB_OAUTH", valid_oauth);
        }
    }
}
