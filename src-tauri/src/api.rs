use crate::state::ApiInfo;
use log::info;
use reqwest::Client;
use std::{sync::LazyLock, time::Duration};

#[cfg(debug_assertions)]
const API_BASE_URL: &str = "https://stage.speleodb.org";
#[cfg(not(debug_assertions))]
const API_BASE_URL: &str = "https://www.speleodb.com";

static API_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build API client")
});

fn get_api_client() -> Client {
    API_CLIENT.clone()
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

pub async fn authorize_with_token(instance: &str, oauth: &str) -> Result<String, String> {
    let url = format!("{}{}", instance, "/api/v1/user/auth-token/");
    let client = get_api_client();

    let response = match client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Network request failed: {}", e));
        }
    };

    let status = response.status();
    #[derive(serde::Deserialize)]
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

pub async fn authorize_with_email(
    instance: &str,
    email: &str,
    password: &str,
) -> Result<String, String> {
    let client = get_api_client();
    let url = format!("{}{}", instance, "/api/v1/user/auth-token/");
    let body = serde_json::json!({"email": email, "password": password});
    let _response = match client.post(&url).json(&body).send().await {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Network request failed: {}", e));
        }
    };
    Ok("yay".to_string())
}
