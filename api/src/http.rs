//! SpeleoDB SDK transport policy and sidecar error adaptation.
//!
//! The generated SDK owns every SpeleoDB request, wire model and response decoder.
//! This module retains the sidecar's timeouts, Token authentication, instance
//! prefixes and status-to-domain-error mapping. Signed storage downloads use a
//! separate anonymous transport so API credentials cannot cross the handoff.

use std::{future::Future, sync::LazyLock, time::Duration};

use common::{ApiInfo, Error};
use reqwest::{Client, StatusCode, header::HeaderMap};
use serde::Deserialize;
use speleodb_api::{ApiOpError, HttpClient};
use url::Url;

pub(crate) const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60);

static API_CLIENT: LazyLock<Client> =
    LazyLock::new(|| transport(Duration::from_secs(10), reqwest::redirect::Policy::none()));
static TRANSFER_CLIENT: LazyLock<Client> =
    LazyLock::new(|| transport(TRANSFER_TIMEOUT, reqwest::redirect::Policy::none()));
static DOWNLOAD_CLIENT: LazyLock<Client> =
    LazyLock::new(|| transport(TRANSFER_TIMEOUT, reqwest::redirect::Policy::limited(10)));

fn transport(timeout: Duration, redirects: reqwest::redirect::Policy) -> Client {
    Client::builder()
        .user_agent(crate::API_USER_AGENT)
        .timeout(timeout)
        .redirect(redirects)
        .build()
        .expect("Failed to build API client")
}

fn instance_base_url(instance: &Url) -> String {
    let mut base = instance.clone();
    base.set_query(None);
    base.set_fragment(None);
    base.as_str().trim_end_matches('/').to_owned()
}

fn sdk_client(instance: &Url, transport: &Client) -> HttpClient {
    HttpClient::with_client(transport.clone())
        .with_base_url(instance_base_url(instance))
        // Preserve the previous client's response sizes, including ZIPs >8 MiB.
        .with_max_response_body_bytes(usize::MAX)
}

pub(crate) fn client(instance: &Url) -> HttpClient {
    sdk_client(instance, &API_CLIENT)
}

pub(crate) fn authenticated(api_info: &ApiInfo) -> Result<HttpClient, Error> {
    let token = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    Ok(client(api_info.instance()).with_header("Authorization", format!("Token {token}")))
}

pub(crate) fn transfer_client(api_info: &ApiInfo) -> Result<HttpClient, Error> {
    let token = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    Ok(sdk_client(api_info.instance(), &TRANSFER_CLIENT)
        .with_header("Authorization", format!("Token {token}")))
}

/// Retry ordinary read-only SDK operations, including interrupted response bodies.
/// Keep writes and transfers outside this policy so they are not replayed and
/// retain their existing transfer timeout budget.
pub(crate) async fn retry_read<T, E, F, Fut>(mut request: F) -> Result<T, Error>
where
    E: std::fmt::Debug,
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, ApiOpError<E>>>,
{
    let mut retries = 0;
    loop {
        let response = request().await;
        if retries < 2
            && matches!(&response, Err(ApiOpError::Transport(error)) if error.is_retryable())
        {
            retries += 1;
            log::warn!("Read-only API transport failed; retrying ({retries}/2)");
            tokio::time::sleep(Duration::from_millis(250 * retries)).await;
            continue;
        }
        return response.map_err(sdk_error);
    }
}

pub(crate) fn sdk_error<E: std::fmt::Debug>(error: ApiOpError<E>) -> Error {
    match error {
        ApiOpError::Transport(error) => {
            let mut message = error.to_string();
            let mut source = std::error::Error::source(&error);
            while let Some(cause) = source {
                let detail = cause.to_string();
                if !message.ends_with(&detail) {
                    message.push_str(": ");
                    message.push_str(&detail);
                }
                source = cause.source();
            }
            Error::NetworkRequest(message)
        }
        ApiOpError::Api(response) => {
            if (200..300).contains(&response.status) {
                return Error::Deserialization(
                    response
                        .parse_error
                        .unwrap_or_else(|| "Unexpected API response".into()),
                );
            }
            map_status_to_error(
                StatusCode::from_u16(response.status).expect("SDK reports a valid HTTP status"),
                Some(&response.body),
            )
        }
    }
}

/// Resolve a signed-storage handoff, then fetch it without API credentials.
/// The endpoint path is used only to resolve relative Location headers; the
/// request to that endpoint is always made by the generated SDK operation.
pub(crate) async fn download_redirect(
    instance: &Url,
    project_id: uuid::Uuid,
    headers: &HeaderMap,
    timeout: Duration,
) -> Result<bytes::Bytes, Error> {
    let location = headers
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| Error::NetworkRequest("Download redirect has no valid Location".into()))?;
    let endpoint = format!(
        "{}/api/v2/projects/{project_id}/download/compass_zip/",
        instance_base_url(instance)
    );
    let location = Url::parse(&endpoint)
        .and_then(|endpoint| endpoint.join(location))
        .map_err(|error| Error::NetworkRequest(error.to_string()))?;
    if !matches!(location.scheme(), "http" | "https")
        || !location.username().is_empty()
        || location.password().is_some()
    {
        return Err(Error::NetworkRequest("Invalid signed download URL".into()));
    }
    let response = DOWNLOAD_CLIENT
        .get(location)
        .timeout(timeout)
        .send()
        .await
        .map_err(|error| Error::NetworkRequest(error.without_url().to_string()))?;
    let status = response.status();
    if status.is_success() {
        return response
            .bytes()
            .await
            .map_err(|error| Error::Deserialization(error.without_url().to_string()));
    }
    let body = response.text().await.ok();
    Err(map_status_to_error(status, body.as_deref()))
}

/// Maximum length (in bytes) of an error message stored in `Error` variants.
///
/// Servers fronted by CDNs (Cloudflare, nginx) can return multi-kilobyte
/// HTML error pages on 502/503; without a cap, the entire blob ends up in
/// the user-facing modal and in Sentry breadcrumbs. 512 bytes is enough for
/// any reasonable JSON error message and short enough to render cleanly.
const MAX_ERROR_MESSAGE_BYTES: usize = 512;

/// Single source of truth for HTTP-status → `Error` mapping.
///
/// Pulls the message from a JSON `{"error": "..."}` body when present;
/// otherwise falls back to the raw body text or the status canonical reason.
pub(crate) fn map_status_to_error(status: StatusCode, body: Option<&str>) -> Error {
    let raw = extract_message(body)
        .or_else(|| body.map(str::to_owned))
        .unwrap_or_else(|| {
            status
                .canonical_reason()
                .unwrap_or("unknown error")
                .to_string()
        });
    let message = cap_error_message(raw);
    match status.as_u16() {
        401 | 403 => Error::Unauthorized(message),
        404 => Error::NotFound(message),
        409 | 423 => Error::Conflict(message),
        422 => Error::Unprocessable(message),
        s => Error::Api { status: s, message },
    }
}

/// Truncate `s` to at most `MAX_ERROR_MESSAGE_BYTES`, snapping back to the
/// nearest UTF-8 char boundary so we never produce invalid UTF-8.
fn cap_error_message(s: String) -> String {
    if s.len() <= MAX_ERROR_MESSAGE_BYTES {
        return s;
    }
    let mut end = MAX_ERROR_MESSAGE_BYTES;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    let mut truncated = s[..end].to_owned();
    truncated.push_str("…[truncated]");
    truncated
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: Option<String>,
}

fn extract_message(body: Option<&str>) -> Option<String> {
    serde_json::from_str::<ErrorBody>(body?).ok()?.error
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_unauthorized_status_with_json_body() {
        let err = map_status_to_error(StatusCode::UNAUTHORIZED, Some(r#"{"error": "bad token"}"#));
        assert!(matches!(err, Error::Unauthorized(ref m) if m == "bad token"));
    }

    #[test]
    fn map_forbidden_collapses_to_unauthorized() {
        let err = map_status_to_error(StatusCode::FORBIDDEN, Some(r#"{"error": "nope"}"#));
        assert!(matches!(err, Error::Unauthorized(ref m) if m == "nope"));
    }

    #[test]
    fn map_not_found_status() {
        let err = map_status_to_error(StatusCode::NOT_FOUND, Some(r#"{"error": "missing"}"#));
        assert!(matches!(err, Error::NotFound(ref m) if m == "missing"));
    }

    #[test]
    fn map_unprocessable_status() {
        let err = map_status_to_error(
            StatusCode::UNPROCESSABLE_ENTITY,
            Some(r#"{"error": "bad"}"#),
        );
        assert!(matches!(err, Error::Unprocessable(ref m) if m == "bad"));
    }

    #[test]
    fn map_conflict_and_locked_collapse_to_conflict() {
        let conflict = map_status_to_error(StatusCode::CONFLICT, Some(r#"{"error": "c"}"#));
        let locked = map_status_to_error(StatusCode::LOCKED, Some(r#"{"error": "l"}"#));
        assert!(matches!(conflict, Error::Conflict(ref m) if m == "c"));
        assert!(matches!(locked, Error::Conflict(ref m) if m == "l"));
    }

    #[test]
    fn map_server_error_falls_through_to_api_variant() {
        let err = map_status_to_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            Some(r#"{"error": "boom"}"#),
        );
        assert!(matches!(
            err,
            Error::Api { status: 500, ref message } if message == "boom"
        ));
    }

    #[test]
    fn falls_back_to_raw_body_when_not_json() {
        let err = map_status_to_error(StatusCode::BAD_REQUEST, Some("plain text"));
        assert!(matches!(
            err,
            Error::Api { status: 400, ref message } if message == "plain text"
        ));
    }

    #[test]
    fn falls_back_to_canonical_reason_when_body_missing() {
        let err = map_status_to_error(StatusCode::BAD_GATEWAY, None);
        assert!(matches!(
            err,
            Error::Api { status: 502, ref message } if message == "Bad Gateway"
        ));
    }

    #[test]
    fn handles_empty_string_body() {
        let err = map_status_to_error(StatusCode::INTERNAL_SERVER_ERROR, Some(""));
        assert!(matches!(err, Error::Api { status: 500, .. }));
    }

    #[test]
    fn caps_oversized_html_error_body() {
        // Simulate a Cloudflare-style HTML 502 page. Without a cap, the full
        // multi-kilobyte payload would land in the user-facing modal.
        let html = "<!DOCTYPE html><html><body>".to_string() + &"x".repeat(8192) + "</body></html>";
        let err = map_status_to_error(StatusCode::BAD_GATEWAY, Some(&html));
        let Error::Api { message, .. } = err else {
            panic!("expected Api variant for 502");
        };
        assert!(
            message.len() < 1024,
            "message must be capped, got {} bytes",
            message.len()
        );
        assert!(
            message.ends_with("…[truncated]"),
            "truncated marker missing: {message}"
        );
    }

    #[test]
    fn caps_oversized_json_error_message() {
        // Even a JSON-parsed message is capped — a buggy server returning a
        // huge `{"error": "..."}` would otherwise bypass the safeguard.
        let big = "y".repeat(4096);
        let body = format!(r#"{{"error": "{big}"}}"#);
        let err = map_status_to_error(StatusCode::INTERNAL_SERVER_ERROR, Some(&body));
        let Error::Api { message, .. } = err else {
            panic!("expected Api variant for 500");
        };
        assert!(message.len() < 1024, "message must be capped: {message}");
        assert!(message.ends_with("…[truncated]"));
    }

    #[test]
    fn cap_respects_utf8_char_boundaries() {
        // Multi-byte chars near the cut point must not produce invalid UTF-8.
        // 'é' is 2 bytes; pad with multi-byte chars so the naive cut would
        // land mid-codepoint.
        let s = "é".repeat(MAX_ERROR_MESSAGE_BYTES);
        let capped = cap_error_message(s);
        // String is valid UTF-8 by construction; the assertion is that we
        // didn't panic during the slice. Sanity-check the suffix.
        assert!(capped.ends_with("…[truncated]"));
    }
}
