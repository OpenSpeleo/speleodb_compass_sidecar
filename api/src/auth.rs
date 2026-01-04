use common::ApiInfo;
use log::{error, info};
use serde::Deserialize;
use serde_json::json;
use url::Url;

use crate::get_api_client;

async fn handle_auth_response(
    instance: Url,
    response: reqwest::Response,
) -> Result<ApiInfo, String> {
    let status = response.status();
    #[derive(Deserialize)]
    struct TokenResponse {
        token: String,
        user: String,
    }
    if status.is_success() {
        let token_response = response
            .json::<TokenResponse>()
            .await
            .map_err(|e| format!("Unexpected response body: {e}"))?;

        let api_info = ApiInfo::new(
            instance,
            Some(token_response.user),
            Some(token_response.token),
        );
        return Ok(api_info);
    } else {
        error!("Authorization failed with status: {}", status);
        Err(format!("Authorization failed with status: {}", status))
    }
}

pub async fn authorize_with_token(instance: Url, oauth: &str) -> Result<ApiInfo, String> {
    let url = instance.join("api/v1/user/auth-token/").unwrap();
    let client = get_api_client();
    info!(
        "Attempting to authorize with: {} using Oauth token",
        instance
    );
    let response = client
        .get(url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;
    handle_auth_response(instance, response).await
}

pub async fn authorize_with_email(
    instance: Url,
    email: &str,
    password: &str,
) -> Result<ApiInfo, String> {
    let client = get_api_client();
    let url = instance.join("api/v1/user/auth-token/").unwrap();
    let body = json!({"email": email, "password": password});
    let response = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;
    handle_auth_response(instance, response).await
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

    #[tokio::test]
    #[serial]
    async fn auth_request_with_real_oauth() {
        if !ensure_test_env_vars() {
            println!("Skipping test: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
            return;
        }

        let instance = Url::parse(&std::env::var("TEST_SPELEODB_INSTANCE").unwrap()).unwrap();
        let oauth = std::env::var("TEST_SPELEODB_OAUTH").unwrap();

        authorize_with_token(instance, &oauth).await.unwrap();
    }
}
