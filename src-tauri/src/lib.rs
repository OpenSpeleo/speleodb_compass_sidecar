// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn save_user_prefs(prefs: serde_json::Value) -> Result<(), String> {
    let mut path = speleodb_compass_common::SDB_USER_DIR.clone();
    path.push("user_prefs.json");
    let s = serde_json::to_string_pretty(&prefs).map_err(|e| e.to_string())?;
    std::fs::write(&path, s).map_err(|e| e.to_string())?;

    // On Unix, tighten permissions so only the owner can read/write the prefs file.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&path) {
            let mut perms = meta.permissions();
            // rw------- (owner read/write)
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(&path, perms).map_err(|e| e.to_string())?;
        }
    }

    // Log the successful save with full path so the frontend/devs can verify persistence.
    log::info!("Preferences saved in {}", path.display());

    Ok(())
}

#[tauri::command]
fn load_user_prefs() -> Result<Option<String>, String> {
    let mut path = speleodb_compass_common::SDB_USER_DIR.clone();
    path.push("user_prefs.json");
    if path.exists() {
        let s = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        Ok(Some(s))
    } else {
        Ok(None)
    }
}

#[tauri::command]
fn forget_user_prefs() -> Result<(), String> {
    let mut path = speleodb_compass_common::SDB_USER_DIR.clone();
    path.push("user_prefs.json");
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn fetch_projects() -> serde_json::Value {
    use reqwest::Client;
    use std::time::Duration;

    // Load user prefs to get instance URL and OAuth token
    let mut path = speleodb_compass_common::SDB_USER_DIR.clone();
    path.push("user_prefs.json");
    
    let prefs_str = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return serde_json::json!({"ok": false, "error": "No saved credentials found"}),
    };
    
    let prefs: serde_json::Value = match serde_json::from_str(&prefs_str) {
        Ok(p) => p,
        Err(e) => return serde_json::json!({"ok": false, "error": format!("Failed to parse preferences: {}", e)}),
    };
    
    let instance = match prefs.get("instance").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return serde_json::json!({"ok": false, "error": "No instance URL in preferences"}),
    };
    
    let oauth = match prefs.get("oauth").and_then(|v| v.as_str()) {
        Some(s) if !s.is_empty() => s,
        _ => return serde_json::json!({"ok": false, "error": "No OAuth token in preferences"}),
    };
    
    let base = instance.trim_end_matches('/');
    let url = format!("{}{}", base, "/api/v1/projects/");
    
    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"ok": false, "error": format!("Failed to build HTTP client: {}", e)}),
    };
    
    let resp = match client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return serde_json::json!({"ok": false, "error": format!("Network request failed: {}", e)}),
    };
    
    let status = resp.status();
    
    if status.is_success() {
        match resp.json::<serde_json::Value>().await {
            Ok(json) => json,
            Err(e) => serde_json::json!({"ok": false, "error": format!("Failed to parse response: {}", e)}),
        }
    } else {
        serde_json::json!({"ok": false, "error": format!("Request failed with status {}", status.as_u16()), "status": status.as_u16()})
    }
}

#[tauri::command]
async fn native_auth_request(
    email: String,
    password: String,
    oauth: String,
    instance: String,
) -> serde_json::Value {
    use reqwest::Client;
    use std::time::Duration;

    if instance.trim().is_empty() {
        return serde_json::json!({"ok": false, "error": "Instance URL is empty"});
    }

    let base = instance.trim_end_matches('/');
    let url = format!("{}{}", base, "/api/v1/user/auth-token/");

    let client = match Client::builder().timeout(Duration::from_secs(10)).build() {
        Ok(c) => c,
        Err(e) => return serde_json::json!({"ok": false, "error": format!("Failed to build HTTP client: {}", e)}),
    };

    let resp = if !oauth.is_empty() {
        match client.get(&url).header("Authorization", format!("Token {}", oauth)).send().await {
            Ok(r) => r,
            Err(e) => return serde_json::json!({"ok": false, "error": format!("Network request failed: {}", e)}),
        }
    } else {
        let body = serde_json::json!({"email": email, "password": password});
        match client.post(&url).json(&body).send().await {
            Ok(r) => r,
            Err(e) => return serde_json::json!({"ok": false, "error": format!("Network request failed: {}", e)}),
        }
    };

    let status = resp.status();
    
    if status.is_success() {
        // Prefer header named Auth-Token
        if let Some(hv) = resp.headers().get("Auth-Token") {
            if let Ok(s) = hv.to_str() {
                if !s.is_empty() {
                    return serde_json::json!({"ok": true, "token": s.to_string()});
                }
            }
        }

        // Fall back to JSON body parsing
        let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!(null));
        if let Some(token) = parse_token_from_json(&json) {
            return serde_json::json!({"ok": true, "token": token});
        }

        serde_json::json!({"ok": false, "error": "Authentication succeeded but token not found in response"})
    } else {
        serde_json::json!({"ok": false, "error": format!("Authentication failed with status {}", status.as_u16()), "status": status.as_u16()})
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
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

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

    #[tokio::test]
    async fn native_auth_request_returns_header_token() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v1/user/auth-token/"))
            .respond_with(ResponseTemplate::new(200).append_header("Auth-Token", "hdrtoken"))
            .mount(&server)
            .await;

        let instance = server.uri();
        let res = native_auth_request(String::new(), String::new(), "ignored".to_string(), instance).await;
        assert!(res.get("ok").and_then(|v| v.as_bool()) == Some(true));
        assert_eq!(res.get("token").and_then(|v| v.as_str()), Some("hdrtoken"));
    }

    #[tokio::test]
    async fn native_auth_request_parses_json_token() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v1/user/auth-token/"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"token": "jsontoken"})))
            .mount(&server)
            .await;

        let instance = server.uri();
        let res = native_auth_request("u".to_string(), "p".to_string(), String::new(), instance).await;
        assert!(res.get("ok").and_then(|v| v.as_bool()) == Some(true));
        assert_eq!(res.get("token").and_then(|v| v.as_str()), Some("jsontoken"));
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Ensure the hidden application directory exists in the user's home directory.
    if let Err(e) = speleodb_compass_common::ensure_app_dir_exists() {
        eprintln!(
            "Failed to create application directory '{}', full path '{}': {:#}",
            speleodb_compass_common::SPELEODB_COMPASS_DIR_NAME,
            speleodb_compass_common::SDB_USER_DIR.display(),
            e
        );
    }

    // Initialize file logging (info level by default). If logger initialization fails,
    // fallback to stderr printing the error.
    if let Err(e) = speleodb_compass_common::init_file_logger("info") {
        eprintln!("Failed to initialize file logger: {:#}", e);
    } else {
        // Log a startup message once the logger is initialized.
        log::info!("Application starting. Logging to: {}", speleodb_compass_common::SDB_USER_DIR.display());
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, save_user_prefs, load_user_prefs, forget_user_prefs, native_auth_request, fetch_projects])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
