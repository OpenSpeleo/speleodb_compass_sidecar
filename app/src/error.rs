use wasm_bindgen::JsValue;

#[derive(Clone, Eq, PartialEq, Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Command(String),
    #[error("Failed to parse JSON: {0}")]
    Serde(String),
}

impl From<serde_wasm_bindgen::Error> for Error {
    fn from(e: serde_wasm_bindgen::Error) -> Self {
        Self::Serde(e.to_string())
    }
}

impl From<JsValue> for Error {
    fn from(e: JsValue) -> Self {
        log::error!("Raw backend command error payload: {:?}", e);

        if let Some(message) = e.as_string() {
            return Self::Command(message);
        }

        let backend_error = serde_wasm_bindgen::from_value::<common::Error>(e.clone());
        if let Ok(error) = backend_error {
            return Self::Command(format_backend_error(&error));
        }

        Self::Command("Backend command failed with an unknown error.".to_string())
    }
}

fn format_backend_error(error: &common::Error) -> String {
    match error {
        common::Error::ProjectImport(source, _, details) if is_permission_denied(details) => {
            let source_name = source
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| source.display().to_string());
            format!(
                "Cannot import '{}' because macOS denied file access. \
                Move/copy the survey folder to an accessible location (for example `~/Documents`) \
                and try again.",
                source_name
            )
        }
        _ => error.to_string(),
    }
}

fn is_permission_denied(details: &str) -> bool {
    let lower = details.to_ascii_lowercase();
    lower.contains("permissiondenied")
        || lower.contains("operation not permitted")
        || lower.contains("permission denied")
        || lower.contains("os error 1")
        || lower.contains("os error 13")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn project_import_permission_error_is_humanized() {
        let backend_error = common::Error::ProjectImport(
            PathBuf::from("/tmp/Region_1.DAT"),
            PathBuf::from("/tmp/project/Region_1.DAT"),
            "Operation not permitted (os error 1) (kind: PermissionDenied, raw_os_error: Some(1))"
                .to_string(),
        );

        let message = format_backend_error(&backend_error);
        assert!(
            message.contains("Cannot import"),
            "permission errors should be translated for users"
        );
        assert!(
            message.contains("macOS denied file access"),
            "permission errors should explain the root cause"
        );
    }

    #[test]
    fn non_permission_import_error_uses_backend_message() {
        let backend_error = common::Error::ProjectImport(
            PathBuf::from("/tmp/Region_1.DAT"),
            PathBuf::from("/tmp/project/Region_1.DAT"),
            "No such file or directory (os error 2)".to_string(),
        );

        let message = format_backend_error(&backend_error);
        assert!(
            message.contains("Error importing project file"),
            "non-permission errors should preserve backend detail"
        );
    }

    #[test]
    fn permission_detection_matches_known_error_markers() {
        assert!(is_permission_denied(
            "Operation not permitted (os error 1) (kind: PermissionDenied)"
        ));
        assert!(is_permission_denied("Permission denied (os error 13)"));
        assert!(!is_permission_denied(
            "No such file or directory (os error 2)"
        ));
    }

    #[test]
    fn js_string_command_error_is_forwarded() {
        let error = Error::from(JsValue::from_str("simple command failure"));
        assert_eq!(
            error,
            Error::Command("simple command failure".to_string()),
            "plain string command errors should be preserved as-is"
        );
    }

    #[test]
    fn unknown_js_error_payload_falls_back_to_generic_message() {
        let error = Error::from(JsValue::from_f64(42.0));
        assert_eq!(
            error,
            Error::Command("Backend command failed with an unknown error.".to_string())
        );
    }
}
