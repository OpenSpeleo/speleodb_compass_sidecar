// WASM controller now delegates network calls to native Tauri backend.
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::Url;
// serde_json::Value not required in this module; network logic moved to native backend

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Prefs {
    pub instance: String,
    pub oauth: String,
}

pub struct SpeleoDBController {}

impl SpeleoDBController {
    pub async fn authenticate(
        &self,
        email: &str,
        password: &str,
        oauth: &str,
        target_instance: &str,
    ) -> Result<(), String> {
        // Validate instance URL
        if Url::new(target_instance).is_err() {
            return Err("SpeleoDB instance must be a valid URL".into());
        }

        // Validation: either oauth token (40 hex) OR email+password
        let oauth_ok = validate_oauth(oauth);
        let pass_ok = validate_email_password(email, password);

        if !(oauth_ok ^ pass_ok) {
            return Err("Must provide exactly one auth method: either email+password or a 40-char OAUTH token".into());
        }

        // Build auth URL (assume AUTH_TOKEN_ENDPOINT). Trim trailing slash on instance.
    const AUTH_TOKEN_ENDPOINT: &str = "/api/v1/user/auth-token/"; // actual API path

        let base = target_instance.trim_end_matches('/');
        let _url = format!("{}{}", base, AUTH_TOKEN_ENDPOINT);

        // Use the native Tauri backend to perform the network request to avoid CORS and webview restrictions.
        #[derive(Serialize)]
        struct NativeArgs<'a> {
            email: &'a str,
            password: &'a str,
            oauth: &'a str,
            instance: &'a str,
        }

        let args = NativeArgs {
            email,
            password,
            oauth,
            instance: target_instance,
        };

        // Call the Tauri invoke - it's async and will return a JsValue
        let serialized_args = serde_wasm_bindgen::to_value(&args)
            .map_err(|e| format!("Failed to serialize args: {:?}", e))?;
        
        let rv = invoke("native_auth_request", serialized_args).await;

        // Try to accept either a plain string token or an object containing the token
        let mut token_opt: Option<String> = None;
        if let Some(s) = rv.as_string() {
            token_opt = Some(s);
        } else {
            // Attempt to deserialize the JS value into JSON and search for common token fields
            if let Ok(json) = serde_wasm_bindgen::from_value::<serde_json::Value>(rv.clone()) {
                // If backend returned an {ok:false, error:...} object, surface the error to the user
                if json.get("ok").and_then(|v| v.as_bool()) == Some(false) {
                    let err_msg = json.get("error").and_then(|v| v.as_str()).unwrap_or("Authentication failed");
                    return Err(err_msg.to_string());
                }
                // Also check for success field from API response
                if json.get("success").and_then(|v| v.as_bool()) == Some(false) {
                    let err_msg = json.get("error").and_then(|v| v.as_str()).unwrap_or("Authentication failed");
                    return Err(err_msg.to_string());
                }

                let candidates = ["token", "auth_token", "access_token", "token_value", "auth"];
                for key in &candidates {
                    if let Some(v) = json.get(*key) {
                        if let Some(s) = v.as_str() {
                            if !s.is_empty() {
                                token_opt = Some(s.to_string());
                                break;
                            }
                        }
                        if v.is_object() {
                            if let Some(s) = v.get("value").and_then(|vv| vv.as_str()) {
                                if !s.is_empty() {
                                    token_opt = Some(s.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(token) = token_opt {
            // Save prefs with instance and token only (no email/password)
            let prefs = Prefs {
                instance: target_instance.to_string(),
                oauth: token,
            };
            #[derive(Serialize)]
            struct SaveArgs<'a> {
                prefs: &'a Prefs,
            }
            let args = SaveArgs { prefs: &prefs };
            let _save_rv = invoke("save_user_prefs", serde_wasm_bindgen::to_value(&args).unwrap_or(JsValue::NULL)).await;
            Ok(())
        } else {
            Err(format!("native_auth_request failed or returned non-token response: {:?}", rv))
        }
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
    parts.len() == 2 && parts[1].contains('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oauth_valid_examples() {
        assert!(validate_oauth(&"0123456789abcdef0123456789abcdef01234567"));
        assert!(!validate_oauth(&"g".repeat(40)));
        assert!(!validate_oauth("short"));
    }

    #[test]
    fn email_password_validation() {
        assert!(validate_email_password("user@example.com", "secret"));
        assert!(!validate_email_password("userexample.com", "secret"));
        assert!(!validate_email_password("user@localhost", "secret"));
        assert!(!validate_email_password("user@example.com", ""));
    }
}
