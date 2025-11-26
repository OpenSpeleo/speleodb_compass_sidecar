use crate::state::ApiInfo;
use reqwest::Client;
use serde::Deserialize;
use speleodb_compass_common::api_types::ProjectInfo;
use std::{sync::LazyLock, time::Duration};
use uuid::Uuid;

static API_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build API client")
});

fn get_api_client() -> Client {
    API_CLIENT.clone()
}

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
    let body = serde_json::json!({"email": email, "password": password});
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;
    handle_auth_response(response).await
}

pub async fn release_project_mutex(api_info: &ApiInfo, project_id: &Uuid) -> Result<(), String> {
    log::info!("Releasing project mutex for project: {}", project_id);
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token()?;
    let url = format!("{}/api/v1/projects/{}/release/", base, project_id);
    let client = get_api_client();

    // Fire and forget
    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| {
            log::warn!("Failed to release mutex (network error): {}", e);
            format!("Network error while releasing mutex: {}", e)
        })?;

    let status = resp.status();
    if status.is_success() {
        log::info!("Successfully released mutex for project: {}", project_id);
        Ok(())
    } else {
        log::warn!("Mutex release returned status {}: {}", status.as_u16(), url);
        Err(format!(
            "Failed to release mutex, server returned status: {}",
            status
        ))
    }
}

pub async fn fetch_projects(api_info: &ApiInfo) -> Result<Vec<ProjectInfo>, String> {
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token()?;
    let url = format!("{}{}", base, "/api/v1/projects/");
    let client = get_api_client();

    let resp = client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    let status = resp.status();

    #[derive(Deserialize)]
    pub struct ProjectsResponse {
        pub data: Vec<ProjectInfo>,
        // Ignore extra fields like timestamp and url
    }

    if status.is_success() {
        match resp.json::<ProjectsResponse>().await {
            Ok(project_response) => Ok(project_response.data),
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        }
    } else {
        Err(format!("Request failed with status {}", status.as_u16()))
    }
}

pub async fn acquire_project_mutex(api_info: &ApiInfo, project_id: Uuid) -> Result<(), String> {
    log::info!("Acquiring project mutex for project: {}", project_id);
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token()?;
    let url = format!("{}/api/v1/projects/{}/acquire/", base, project_id);
    let client = get_api_client();

    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| format!("Network error attempting to lock project: {e}"))?;

    let status = resp.status();

    if status.is_success() {
        // Successfully acquired the mutex
        Ok(())
    } else if status.as_u16() == 409 || status.as_u16() == 423 {
        // 409 Conflict or 423 Locked - mutex is already held by another user
        Err("Project is already locked by another user".to_string())
    } else {
        Err(format!(
            "Mutex acquisition failed with status {}",
            status.as_u16()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api;
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

        api::authorize_with_token(&instance, &oauth).await.unwrap();
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
        std::env::set_var("TEST_SPELEODB_INSTANCE", &instance);
        std::env::set_var(
            "TEST_SPELEODB_OAUTH",
            "0000000000000000000000000000000000000000",
        ); // Fetch projects from real API (uses env vars directly)
        let api_info = ApiInfo::from_env().unwrap();
        let _result = fetch_projects(&api_info)
            .await
            .expect_err("This shouldn't work");

        // Restore the valid token immediately
        std::env::set_var("TEST_SPELEODB_OAUTH", valid_oauth);
    }
}
