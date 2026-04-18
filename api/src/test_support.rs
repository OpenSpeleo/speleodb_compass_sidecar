//! Test infrastructure for the api crate.
//!
//! Compiled only with `#[cfg(test)]`. Provides:
//! - `.env` autoloading via a `ctor`-registered initializer.
//! - `ApiInfo` builders for the happy path (`test_api_info`) and for
//!   exercising unauthorized failures (`unauthorized_api_info`).
//! - A lazily-created shared fixture project (`fixture_project_id`) used by
//!   read-only tests. There is no project-delete endpoint server-side, so
//!   the fixture is created at most once per `cargo test` invocation and
//!   then survives indefinitely. Tests that mutate state (upload, etc.)
//!   create their own fresh project to keep the shared fixture clean.
//!   Set `TEST_SPELEODB_PROJECT_ID` to a UUID to reuse a permanent fixture
//!   instead of creating a new one (recommended for CI to avoid littering
//!   staging with auto-created projects).
//! - A `build_minimal_compass_zip` helper that produces a tiny, valid
//!   Compass project ZIP in a temp file for upload-path tests.

use std::{future::Future, io::Write, panic};

use common::{ApiInfo, api_types::ProjectInfo};
use tempfile::NamedTempFile;
use tokio::sync::OnceCell;
use url::Url;
use uuid::Uuid;

use crate::project;

static FIXTURE_PROJECT: OnceCell<Uuid> = OnceCell::const_new();

/// Load `.env` from the workspace root before any test runs.
#[ctor::ctor]
fn load_test_env() {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let workspace_root = std::path::Path::new(&manifest_dir).parent().unwrap();
        let env_path = workspace_root.join(".env");
        if env_path.exists() {
            let _ = dotenvy::from_path(&env_path);
        }
    }
    let _ = dotenvy::dotenv();
}

/// Return `true` iff the credentials needed to talk to a real SpeleoDB
/// instance are available. Tests should early-return when this is `false`.
pub(crate) fn ensure_test_env_vars() -> bool {
    let ok = std::env::var("TEST_SPELEODB_INSTANCE").is_ok()
        && std::env::var("TEST_SPELEODB_OAUTH").is_ok();
    if !ok {
        eprintln!("Skipping: TEST_SPELEODB_INSTANCE or TEST_SPELEODB_OAUTH not set");
    }
    ok
}

/// Configured test instance URL. Caller must gate with `ensure_test_env_vars`.
pub(crate) fn test_instance() -> Url {
    Url::parse(&std::env::var("TEST_SPELEODB_INSTANCE").expect("TEST_SPELEODB_INSTANCE not set"))
        .expect("TEST_SPELEODB_INSTANCE must be a valid URL")
}

/// `ApiInfo` carrying the real test credentials.
pub(crate) fn test_api_info() -> ApiInfo {
    let oauth = std::env::var("TEST_SPELEODB_OAUTH").expect("TEST_SPELEODB_OAUTH not set");
    let email = std::env::var("TEST_SPELEODB_EMAIL").ok();
    ApiInfo::new(test_instance(), email, Some(oauth))
}

/// `ApiInfo` carrying the real test instance URL but a token that is
/// guaranteed to be rejected. Used to exercise 401/403 paths.
pub(crate) fn unauthorized_api_info() -> ApiInfo {
    ApiInfo::new(
        test_instance(),
        Some("nobody@example.invalid".to_string()),
        Some("0".repeat(40)),
    )
}

/// Lazily resolve one shared fixture project per `cargo test` invocation
/// and return its UUID. Subsequent calls return the cached UUID without
/// hitting the server.
///
/// If `TEST_SPELEODB_PROJECT_ID` is set to a valid UUID, it is reused
/// directly — no project is created. Otherwise a fresh project is created
/// on the configured instance and survives forever (no delete endpoint).
pub(crate) async fn fixture_project_id(api_info: &ApiInfo) -> Uuid {
    *FIXTURE_PROJECT
        .get_or_init(|| async {
            if let Ok(raw) = std::env::var("TEST_SPELEODB_PROJECT_ID")
                && let Ok(id) = Uuid::parse_str(raw.trim())
            {
                return id;
            }
            let suffix = Uuid::new_v4().simple().to_string()[..8].to_owned();
            let info = project::create_project(
                api_info,
                format!("sidecar-ci-fixture-{suffix}"),
                "Auto-created shared fixture for the speleodb-compass-sidecar test suite".into(),
                "US".into(),
                None,
                None,
            )
            .await
            .expect("fixture project creation must succeed");
            info.id
        })
        .await
}

/// Acquire the project mutex, run `body`, then always release it before
/// returning or resuming a panic from the body.
pub(crate) async fn with_acquired_project_mutex<F, Fut, T>(
    api_info: &ApiInfo,
    project_id: Uuid,
    body: F,
) -> T
where
    F: FnOnce(ProjectInfo) -> Fut + Send + 'static,
    Fut: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let info = project::acquire_project_mutex(api_info, project_id)
        .await
        .expect("acquire must succeed");
    let outcome = tokio::spawn(body(info)).await;

    project::release_project_mutex(api_info, project_id)
        .await
        .expect("release must succeed");

    match outcome {
        Ok(value) => value,
        Err(err) if err.is_panic() => panic::resume_unwind(err.into_panic()),
        Err(err) => panic!("mutex-guarded test task failed: {err}"),
    }
}

/// Build a minimal but server-acceptable Compass project ZIP in a temp file.
/// Contains a `compass.toml` and a stub `.MAK` file so the upload endpoint
/// recognizes the payload as a valid project.
pub(crate) fn build_minimal_compass_zip() -> NamedTempFile {
    let mut temp = NamedTempFile::new().expect("must create temp file");
    {
        let file = temp.as_file_mut();
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("compass.toml", opts)
            .expect("start compass.toml");
        zip.write_all(
            b"[speleodb]\nid = \"00000000-0000-0000-0000-000000000000\"\n\
              version = \"1.0.0\"\n\n[project]\nmak_file = \"test.mak\"\n\
              dat_files = []\nplt_files = []\n",
        )
        .expect("write compass.toml");
        zip.start_file("test.mak", opts).expect("start test.mak");
        zip.write_all(b"#test\n").expect("write test.mak");
        zip.finish().expect("zip finalize");
    }
    temp
}
