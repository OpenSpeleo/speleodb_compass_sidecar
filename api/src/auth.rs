use common::{ApiInfo, Error};
use log::{error, info};
use reqwest::RequestBuilder;
use serde::Deserialize;
use serde_json::json;
use url::Url;

use crate::{get_api_client, http};

#[derive(Deserialize)]
struct TokenResponse {
    token: String,
    user: String,
}

async fn handle_auth_response(instance: Url, builder: RequestBuilder) -> Result<ApiInfo, String> {
    match http::send_json::<TokenResponse>(builder).await {
        Ok(resp) => Ok(ApiInfo::new(instance, Some(resp.user), Some(resp.token))),
        Err(err) => {
            error!("Authorization failed: {err}");
            Err(format_auth_error(&err))
        }
    }
}

/// Translate a typed `Error` into the user-facing string the UI expects.
///
/// The mapping mirrors the original (pre-v2) behavior: 400/401/403 all
/// indicate bad credentials from a user's perspective and collapse to a
/// single friendly message. Every HTTP-derived `Error` variant the `http`
/// module can produce must be matched explicitly here — otherwise the
/// `Display` impl leaks `"Unprocessable entity: ..."`-style strings into
/// the auth modal.
fn format_auth_error(err: &Error) -> String {
    const INVALID_CREDENTIALS: &str =
        "Invalid credentials. Please check your email/password or OAuth token and try again.";
    const GENERIC_AUTH_FAILED: &str = "Authentication failed. Please try again.";
    match err {
        Error::Unauthorized(_) => INVALID_CREDENTIALS.to_string(),
        Error::Api { status: 400, .. } => INVALID_CREDENTIALS.to_string(),
        Error::NotFound(_) => {
            "Authentication endpoint not found. Please verify the instance URL is correct."
                .to_string()
        }
        Error::Api { status: 429, .. } => {
            "Too many login attempts. Please wait a moment and try again.".to_string()
        }
        Error::Api { status, .. } if (500..=599).contains(status) => {
            format!("The server encountered an error (HTTP {status}). Please try again later.")
        }
        Error::Api { status, .. } => {
            format!("Authentication failed (HTTP {status}). Please try again.")
        }
        Error::Unprocessable(_) | Error::Conflict(_) => GENERIC_AUTH_FAILED.to_string(),
        Error::NetworkRequest(msg) => format!("Network request failed: {msg}"),
        Error::Deserialization(msg) => format!("Unexpected response body: {msg}"),
        other => other.to_string(),
    }
}

pub async fn authorize_with_token(instance: Url, oauth: &str) -> Result<ApiInfo, String> {
    info!("Attempting to authorize with: {instance} using OAuth token");
    let url = http::v2_url(&instance, "user/auth-token/");
    let builder = get_api_client()
        .get(url)
        .header("Authorization", format!("Token {oauth}"));
    handle_auth_response(instance, builder).await
}

pub async fn authorize_with_email(
    instance: Url,
    email: &str,
    password: &str,
) -> Result<ApiInfo, String> {
    info!("Attempting to authorize with: {instance} using email/password");
    let url = http::v2_url(&instance, "user/auth-token/");
    let body = json!({"email": email, "password": password});
    let builder = get_api_client().post(url).json(&body);
    handle_auth_response(instance, builder).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{ensure_test_env_vars, test_instance};
    use serial_test::serial;

    #[test]
    fn format_auth_error_unauthorized_uses_user_friendly_text() {
        let msg = format_auth_error(&Error::Unauthorized("bad token".to_string()));
        assert!(msg.contains("Invalid credentials"));
    }

    #[test]
    fn format_auth_error_400_collapses_to_invalid_credentials() {
        // The server returns 400 for malformed/invalid email+password, so
        // the auth flow treats it the same as 401/403 from a user POV.
        let msg = format_auth_error(&Error::Api {
            status: 400,
            message: "no such user".to_string(),
        });
        assert!(msg.contains("Invalid credentials"));
    }

    #[test]
    fn format_auth_error_not_found_mentions_instance_url() {
        let msg = format_auth_error(&Error::NotFound("missing".to_string()));
        assert!(msg.contains("instance URL"));
    }

    #[test]
    fn format_auth_error_429_mentions_too_many_attempts() {
        let msg = format_auth_error(&Error::Api {
            status: 429,
            message: "rate".to_string(),
        });
        assert!(msg.contains("Too many login attempts"));
    }

    #[test]
    fn format_auth_error_5xx_mentions_server_error() {
        let msg = format_auth_error(&Error::Api {
            status: 503,
            message: "down".to_string(),
        });
        assert!(msg.contains("server encountered an error"));
        assert!(msg.contains("503"));
    }

    #[test]
    fn format_auth_error_unprocessable_collapses_to_generic_auth_failed() {
        // 422 from the http layer must not leak the typed Display string
        // ("Unprocessable entity: ...") into the auth modal.
        let msg = format_auth_error(&Error::Unprocessable("invalid email format".to_string()));
        assert!(
            !msg.contains("Unprocessable"),
            "raw Display must not leak: {msg}"
        );
        assert!(
            msg.contains("Authentication failed"),
            "expected generic auth-failed text, got: {msg}"
        );
    }

    #[test]
    fn format_auth_error_conflict_collapses_to_generic_auth_failed() {
        // 409/423 from the http layer must not leak the typed Display string
        // ("Conflict: ...") into the auth modal.
        let msg = format_auth_error(&Error::Conflict("locked".to_string()));
        assert!(
            !msg.contains("Conflict:"),
            "raw Display must not leak: {msg}"
        );
        assert!(
            msg.contains("Authentication failed"),
            "expected generic auth-failed text, got: {msg}"
        );
    }

    #[test]
    fn format_auth_error_network_failure_passes_through() {
        let msg = format_auth_error(&Error::NetworkRequest("timeout".to_string()));
        assert!(msg.contains("Network request failed"));
        assert!(msg.contains("timeout"));
    }

    #[tokio::test]
    #[serial]
    async fn authorize_with_token_success() {
        if !ensure_test_env_vars() {
            return;
        }
        let instance = test_instance();
        let oauth = std::env::var("TEST_SPELEODB_OAUTH").unwrap();
        let api_info = authorize_with_token(instance, &oauth)
            .await
            .expect("real OAuth token must succeed");
        assert!(api_info.email().is_some(), "server must return user email");
        assert!(api_info.oauth_token().is_some(), "token must round-trip");
    }

    #[tokio::test]
    #[serial]
    async fn authorize_with_token_invalid_returns_friendly_message() {
        if !ensure_test_env_vars() {
            return;
        }
        let instance = test_instance();
        let bogus = "0".repeat(40);
        let err = authorize_with_token(instance, &bogus)
            .await
            .expect_err("bogus token must fail");
        assert!(
            err.contains("Invalid credentials"),
            "expected friendly invalid-credentials message, got: {err}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn authorize_with_email_success() {
        if !ensure_test_env_vars() {
            return;
        }
        let (Ok(email), Ok(password)) = (
            std::env::var("TEST_SPELEODB_EMAIL"),
            std::env::var("TEST_SPELEODB_PASSWORD"),
        ) else {
            println!("Skipping: TEST_SPELEODB_EMAIL or TEST_SPELEODB_PASSWORD not set");
            return;
        };
        let instance = test_instance();
        let api_info = authorize_with_email(instance, &email, &password)
            .await
            .expect("real email/password must succeed");
        assert_eq!(api_info.email(), Some(email.as_str()));
    }

    #[tokio::test]
    #[serial]
    async fn authorize_with_email_invalid_returns_friendly_message() {
        if !ensure_test_env_vars() {
            return;
        }
        let instance = test_instance();
        let err = authorize_with_email(instance, "nobody@example.invalid", "wrong")
            .await
            .expect_err("invalid email/password must fail");
        assert!(
            err.contains("Invalid credentials"),
            "expected friendly invalid-credentials message, got: {err}"
        );
    }
}
