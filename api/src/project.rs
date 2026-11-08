use std::{path::Path, time::Instant};

use common::{
    ApiInfo, Error,
    api_types::{ProjectInfo, ProjectSaveResult},
};
use log::{error, info, warn};
use speleodb_api::{ApiOpError, types};
use uuid::Uuid;

use crate::{http, models::project_info};

/// HTTP status used by the v2 upload endpoint when the working copy already
/// matches the latest server commit. We surface this as `NoChanges` rather
/// than an error so the UI can show the appropriate message.
const NOT_MODIFIED: u16 = 304;

pub async fn create_project(
    api_info: &ApiInfo,
    name: String,
    description: String,
    country: String,
    latitude: Option<String>,
    longitude: Option<String>,
) -> Result<ProjectInfo, Error> {
    info!("Creating new project: {name}");
    let client = http::authenticated(api_info)?;
    let country = serde_json::from_value(serde_json::Value::String(country))
        .map_err(|error| Error::Unprocessable(format!("Invalid project country: {error}")))?;
    let mut request =
        types::V2ProjectsCreateWithJsonRequest::new(country, description, name, "COMPASS".into());
    request.latitude = latitude.filter(|value| !value.is_empty()).map(|value| {
        Some(types::V2ProjectsCreateWithJsonRequestLatitude::String(
            value,
        ))
    });
    request.longitude = longitude.filter(|value| !value.is_empty()).map(|value| {
        Some(types::V2ProjectsCreateWithJsonRequestLongitude::String(
            value,
        ))
    });
    let project = client
        .v2_projects_create_with_json(None, None::<&str>, request)
        .await
        .map_err(http::sdk_error)?;
    project_info(project)
}

pub async fn acquire_project_mutex(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<ProjectInfo, Error> {
    info!("Acquiring project mutex for project: {project_id}");
    let result = http::authenticated(api_info)?
        .v2_projects_acquire_create(project_id.to_string(), None, None::<&str>)
        .await
        .map_err(http::sdk_error)
        .and_then(project_info);
    match result {
        Err(Error::Conflict(_)) => {
            warn!("Mutex already locked by another user for project: {project_id}");
            Err(Error::ProjectMutexLocked(project_id))
        }
        other => other,
    }
}

pub async fn release_project_mutex(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<ProjectInfo, Error> {
    info!("Releasing project mutex for project: {project_id}");
    let project = http::authenticated(api_info)?
        .v2_projects_release_create_with_json(project_id.to_string(), None, None::<&str>, None)
        .await
        .map_err(http::sdk_error)?;
    project_info(project)
}

pub async fn fetch_projects(api_info: &ApiInfo) -> Result<Vec<ProjectInfo>, Error> {
    info!("Fetching projects from server: {}", api_info.instance());
    let client = http::authenticated(api_info)?;
    let response = http::retry_read(|| client.v2_projects_list(None)).await?;
    let mut projects = response
        .into_iter()
        .map(project_info)
        .collect::<Result<Vec<_>, _>>()?;
    retain_compass_projects(&mut projects);
    Ok(projects)
}

fn retain_compass_projects(projects: &mut Vec<ProjectInfo>) {
    projects.retain(|project| project.project_type.is_compass());
}

pub async fn fetch_project_info(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<ProjectInfo, Error> {
    info!("Fetching project info from server for project: {project_id}");
    let client = http::authenticated(api_info)?;
    let project =
        http::retry_read(|| client.v2_project_retrieve(project_id.to_string(), None)).await?;
    project_info(project)
}

pub async fn download_project_zip(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<bytes::Bytes, Error> {
    info!("Downloading project zip for project: {project_id}");
    let started = Instant::now();
    let response = http::transfer_client(api_info)?
        .v2_projects_download_retrieve_by_format("compass_zip", project_id.to_string(), None)
        .await;
    let result = match response {
        Ok(bytes) => Ok(bytes),
        Err(ApiOpError::Api(response))
            if matches!(response.status, 301 | 302 | 303 | 307 | 308) =>
        {
            http::download_redirect(
                api_info.instance(),
                project_id,
                &response.headers,
                http::TRANSFER_TIMEOUT.saturating_sub(started.elapsed()),
            )
            .await
        }
        Err(error) => Err(http::sdk_error(error)),
    };
    info!(
        "Download request completed for project {project_id} in {:?}",
        started.elapsed()
    );
    match result {
        Err(Error::Unprocessable(_)) => Err(Error::NoProjectData(project_id)),
        Err(error) => {
            error!("Download failed for project {project_id}: {error}");
            Err(error)
        }
        other => other,
    }
}

pub async fn upload_project_zip(
    api_info: &ApiInfo,
    project_id: Uuid,
    commit_message: String,
    zip_path: &Path,
) -> Result<ProjectSaveResult, Error> {
    info!("Uploading project ZIP for project: {project_id}");
    let zip_bytes = std::fs::read(zip_path).map_err(|error| Error::FileRead(error.to_string()))?;
    let request = types::ProjectUploadRequestRequest {
        artifact: vec![types::MultipartFile {
            content: zip_bytes.into(),
            file_name: Some("project.zip".into()),
            content_type: Some("application/zip".into()),
        }],
        message: commit_message,
    };
    let started = Instant::now();
    let result = http::transfer_client(api_info)?
        .v2_projects_upload(
            "compass_zip",
            project_id.to_string(),
            None,
            None::<&str>,
            request,
        )
        .await
        .map_err(http::sdk_error);
    info!(
        "Upload request completed for project {project_id} in {:?}",
        started.elapsed()
    );
    match result {
        Ok(_) => {
            info!("Successfully uploaded project: {project_id}");
            Ok(ProjectSaveResult::Saved)
        }
        Err(Error::Api {
            status: NOT_MODIFIED,
            ..
        }) => {
            info!("No changes to upload for project: {project_id}");
            Ok(ProjectSaveResult::NoChanges)
        }
        Err(error) => {
            error!("Upload failed for project {project_id}: {error}");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{
        build_minimal_compass_zip, ensure_test_env_vars, fixture_project_id, test_api_info,
        unauthorized_api_info, with_acquired_project_mutex,
    };
    use common::api_types::ProjectType;
    use serial_test::serial;

    /// A UUID that is virtually guaranteed not to exist server-side, used
    /// to exercise 404-returning endpoints without polluting the database.
    fn unknown_project_id() -> Uuid {
        Uuid::new_v4()
    }

    fn project_with_type(project_type: ProjectType) -> ProjectInfo {
        ProjectInfo {
            id: Uuid::new_v4(),
            name: "Test Project".to_string(),
            description: "Test Description".to_string(),
            is_active: true,
            permission: "ADMIN".to_string(),
            active_mutex: None,
            country: "US".to_string(),
            created_by: "tester@example.com".to_string(),
            creation_date: "2026-01-01T00:00:00Z".to_string(),
            modified_date: "2026-01-01T00:00:00Z".to_string(),
            latitude: None,
            longitude: None,
            fork_from: None,
            visibility: "PRIVATE".to_string(),
            exclude_geojson: false,
            latest_commit: None,
            project_type,
        }
    }

    async fn existing_project_id() -> Uuid {
        fixture_project_id(&test_api_info()).await
    }

    // ─── fetch_projects ────────────────────────────────────────────────────

    #[test]
    fn retain_compass_projects_ignores_unsupported_types() {
        let compass_id = Uuid::new_v4();
        let mut compass = project_with_type(ProjectType::Compass);
        compass.id = compass_id;
        let mut projects = vec![
            project_with_type(ProjectType::Ignored),
            compass,
            project_with_type(ProjectType::Ignored),
        ];

        retain_compass_projects(&mut projects);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, compass_id);
        assert_eq!(projects[0].project_type, ProjectType::Compass);
    }

    #[tokio::test]
    #[serial]
    async fn fetch_projects_success_returns_at_least_fixture() {
        if !ensure_test_env_vars().await {
            return;
        }
        let api_info = test_api_info();
        let fixture_id = fixture_project_id(&api_info).await;
        let projects = fetch_projects(&api_info)
            .await
            .expect("fetch_projects must succeed");
        assert!(
            projects.iter().any(|p| p.id == fixture_id),
            "fixture project must appear in the list"
        );
        assert!(
            projects
                .iter()
                .all(|p| p.project_type == ProjectType::Compass),
            "fetch_projects must filter to COMPASS-type projects"
        );
    }

    #[tokio::test]
    #[serial]
    async fn fetch_projects_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = fetch_projects(&unauthorized_api_info())
            .await
            .expect_err("bogus token must fail");
        assert!(
            matches!(err, Error::Unauthorized(_)),
            "expected Unauthorized, got: {err:?}"
        );
    }

    #[tokio::test]
    async fn fetch_projects_no_token_returns_no_auth_token() {
        let api_info = ApiInfo::default();
        let err = fetch_projects(&api_info)
            .await
            .expect_err("missing token must fail");
        assert!(matches!(err, Error::NoAuthToken));
    }

    // ─── fetch_project_info ────────────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn fetch_project_info_success() {
        if !ensure_test_env_vars().await {
            return;
        }
        let api_info = test_api_info();
        let fixture_id = fixture_project_id(&api_info).await;
        let info = fetch_project_info(&api_info, fixture_id)
            .await
            .expect("fetch_project_info must succeed");
        assert_eq!(info.id, fixture_id);
    }

    #[tokio::test]
    #[serial]
    async fn fetch_project_info_not_found() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = fetch_project_info(&test_api_info(), unknown_project_id())
            .await
            .expect_err("unknown project must fail");
        assert!(
            matches!(err, Error::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn fetch_project_info_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = fetch_project_info(&unauthorized_api_info(), existing_project_id().await)
            .await
            .expect_err("bogus token must fail");
        assert!(matches!(err, Error::Unauthorized(_)));
    }

    // ─── create_project ────────────────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn create_project_success() {
        if !ensure_test_env_vars().await {
            return;
        }
        let api_info = test_api_info();
        let suffix = Uuid::new_v4().simple().to_string()[..8].to_owned();
        let info = create_project(
            &api_info,
            format!("sidecar-ci-create-{suffix}"),
            "Auto-created by create_project_success".into(),
            "US".into(),
            None,
            None,
        )
        .await
        .expect("create_project must succeed");
        assert_eq!(info.project_type, ProjectType::Compass);
        assert!(!info.name.is_empty());
    }

    #[tokio::test]
    #[serial]
    async fn create_project_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = create_project(
            &unauthorized_api_info(),
            "should-fail".into(),
            "".into(),
            "US".into(),
            None,
            None,
        )
        .await
        .expect_err("bogus token must fail");
        assert!(matches!(err, Error::Unauthorized(_)));
    }

    #[tokio::test]
    #[serial]
    async fn create_project_invalid_input_returns_unprocessable_or_api_error() {
        if !ensure_test_env_vars().await {
            return;
        }
        // Empty name is rejected by the server. Different deployments may
        // surface this as 400 or 422, so accept either an Unprocessable or
        // a generic Api error in the 4xx range.
        let err = create_project(
            &test_api_info(),
            String::new(),
            String::new(),
            String::new(),
            None,
            None,
        )
        .await
        .expect_err("empty name must be rejected");
        assert!(
            matches!(
                err,
                Error::Unprocessable(_)
                    | Error::Api {
                        status: 400..=499,
                        ..
                    }
            ),
            "expected client validation error, got: {err:?}"
        );
    }

    // ─── acquire_project_mutex ─────────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn acquire_and_release_project_mutex_success() {
        if !ensure_test_env_vars().await {
            return;
        }
        let api_info = test_api_info();
        let fixture_id = fixture_project_id(&api_info).await;
        let info =
            with_acquired_project_mutex(&api_info, fixture_id, |info| async move { info }).await;
        assert_eq!(info.id, fixture_id);
    }

    #[tokio::test]
    #[serial]
    async fn acquire_project_mutex_not_found() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = acquire_project_mutex(&test_api_info(), unknown_project_id())
            .await
            .expect_err("unknown project must fail");
        assert!(
            matches!(err, Error::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn acquire_project_mutex_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = acquire_project_mutex(&unauthorized_api_info(), existing_project_id().await)
            .await
            .expect_err("bogus token must fail");
        assert!(matches!(err, Error::Unauthorized(_)));
    }

    // ─── release_project_mutex ─────────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn release_project_mutex_not_found() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = release_project_mutex(&test_api_info(), unknown_project_id())
            .await
            .expect_err("unknown project must fail");
        assert!(
            matches!(err, Error::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn release_project_mutex_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = release_project_mutex(&unauthorized_api_info(), existing_project_id().await)
            .await
            .expect_err("bogus token must fail");
        assert!(matches!(err, Error::Unauthorized(_)));
    }

    // ─── download_project_zip ──────────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn download_project_zip_no_data_returns_no_project_data() {
        if !ensure_test_env_vars().await {
            return;
        }
        // Use a fresh project (NOT the shared fixture) so we can guarantee
        // no upload has happened yet.
        let api_info = test_api_info();
        let suffix = Uuid::new_v4().simple().to_string()[..8].to_owned();
        let fresh = create_project(
            &api_info,
            format!("sidecar-ci-empty-{suffix}"),
            "Empty project for download_no_data test".into(),
            "US".into(),
            None,
            None,
        )
        .await
        .expect("fresh project creation must succeed");

        let err = download_project_zip(&api_info, fresh.id)
            .await
            .expect_err("empty project must fail");
        assert!(
            matches!(err, Error::NoProjectData(id) if id == fresh.id),
            "expected NoProjectData({}), got: {err:?}",
            fresh.id
        );
    }

    #[tokio::test]
    #[serial]
    async fn download_project_zip_not_found() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = download_project_zip(&test_api_info(), unknown_project_id())
            .await
            .expect_err("unknown project must fail");
        assert!(
            matches!(err, Error::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn download_project_zip_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let err = download_project_zip(&unauthorized_api_info(), existing_project_id().await)
            .await
            .expect_err("bogus token must fail");
        assert!(matches!(err, Error::Unauthorized(_)));
    }

    // ─── upload_project_zip ────────────────────────────────────────────────

    #[tokio::test]
    #[serial]
    async fn upload_then_download_project_zip_success() {
        if !ensure_test_env_vars().await {
            return;
        }
        let api_info = test_api_info();
        let suffix = Uuid::new_v4().simple().to_string()[..8].to_owned();
        let fresh = create_project(
            &api_info,
            format!("sidecar-ci-upload-{suffix}"),
            "Project for upload+download lifecycle test".into(),
            "US".into(),
            None,
            None,
        )
        .await
        .expect("project creation must succeed");

        let api_info_for_lifecycle = api_info.clone();
        let lifecycle_result = with_acquired_project_mutex(&api_info, fresh.id, move |_| {
            let api_info = api_info_for_lifecycle;
            async move {
                let zip = build_minimal_compass_zip();
                let save_result = upload_project_zip(
                    &api_info,
                    fresh.id,
                    "Test commit from upload_then_download_project_zip_success".into(),
                    zip.path(),
                )
                .await?;
                let bytes = download_project_zip(&api_info, fresh.id).await?;
                Ok::<_, Error>((save_result, bytes))
            }
        })
        .await;
        let (result, bytes) = lifecycle_result.expect("upload/download lifecycle must succeed");
        assert_eq!(result, ProjectSaveResult::Saved);
        assert!(!bytes.is_empty(), "downloaded zip must not be empty");
    }

    #[tokio::test]
    #[serial]
    async fn upload_project_zip_not_found() {
        if !ensure_test_env_vars().await {
            return;
        }
        let zip = build_minimal_compass_zip();
        let err = upload_project_zip(
            &test_api_info(),
            unknown_project_id(),
            "Test commit".into(),
            zip.path(),
        )
        .await
        .expect_err("unknown project must fail");
        assert!(
            matches!(err, Error::NotFound(_)),
            "expected NotFound, got: {err:?}"
        );
    }

    #[tokio::test]
    #[serial]
    async fn upload_project_zip_unauthorized() {
        if !ensure_test_env_vars().await {
            return;
        }
        let zip = build_minimal_compass_zip();
        let err = upload_project_zip(
            &unauthorized_api_info(),
            existing_project_id().await,
            "Test commit".into(),
            zip.path(),
        )
        .await
        .expect_err("bogus token must fail");
        assert!(matches!(err, Error::Unauthorized(_)));
    }

    #[tokio::test]
    async fn upload_project_zip_missing_file_returns_file_read() {
        let err = upload_project_zip(
            &ApiInfo::default(),
            Uuid::new_v4(),
            "x".into(),
            Path::new("/definitely/does/not/exist.zip"),
        )
        .await
        .expect_err("missing zip path must fail");
        assert!(
            matches!(err, Error::FileRead(_)),
            "expected FileRead, got: {err:?}"
        );
    }
}
