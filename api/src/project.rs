use std::path::Path;

use common::{
    ApiInfo,
    api_types::{ProjectInfo, ProjectSaveResult, ProjectType},
};
use errors::Error;
use log::{error, info};
use serde::Deserialize;
use uuid::Uuid;

use crate::get_api_client;

pub async fn acquire_project_mutex(api_info: &ApiInfo, project_id: Uuid) -> Result<(), Error> {
    log::info!("Acquiring project mutex for project: {}", project_id);
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let route = format!("api/v1/projects/{}/acquire/", project_id);
    let url = base.join(&route).unwrap();
    let client = get_api_client();

    let resp = client
        .post(url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;

    let status = resp.status();

    if status.is_success() {
        // Successfully acquired the mutex
        Ok(())
    } else if status.as_u16() == 409 || status.as_u16() == 423 {
        // 409 Conflict or 423 Locked - mutex is already held by another user
        Err(Error::ProjectMutexLocked(project_id))
    } else {
        Err(Error::Api(status.as_u16()))
    }
}

pub async fn create_project(
    api_info: &ApiInfo,
    name: String,
    description: String,
    country: String,
    latitude: Option<String>,
    longitude: Option<String>,
) -> Result<ProjectInfo, Error> {
    log::info!("Creating new project: {}", name);
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let url = base.join("api/v1/projects/").unwrap();
    let client = get_api_client();

    let mut body = serde_json::Map::new();
    body.insert("name".to_string(), serde_json::json!(name));
    body.insert("description".to_string(), serde_json::json!(description));
    body.insert("country".to_string(), serde_json::json!(country));
    body.insert("type".to_string(), serde_json::json!(&ProjectType::Compass));
    if let Some(lat) = latitude {
        if !lat.is_empty() {
            body.insert("latitude".to_string(), serde_json::json!(lat));
        }
    }
    if let Some(lon) = longitude {
        if !lon.is_empty() {
            body.insert("longitude".to_string(), serde_json::json!(lon));
        }
    }

    let resp = client
        .post(url)
        .header("Authorization", format!("Token {}", oauth))
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            error!("Error attempting to create new project: {e:?}");
            Error::NetworkRequest(e.to_string())
        })?;

    let status = resp.status();

    if status.is_success() {
        #[derive(Deserialize)]
        pub struct ProjectInfoResponse {
            pub data: ProjectInfo,
            // Ignore extra fields like timestamp and url
        }
        let json = resp.json::<ProjectInfoResponse>().await.map_err(|e| {
            error!("Failed to deserialize project creation response: {e:?}");
            Error::Deserialization(e.to_string())
        })?;

        // Return the project data wrapped in our standard format
        Ok(json.data)
    } else {
        let status_code = status.as_u16();
        error!("Project creation failed with status code: {}", status_code);
        Err(Error::Api(status.as_u16()))
    }
}

pub async fn release_project_mutex(api_info: &ApiInfo, project_id: Uuid) -> Result<(), Error> {
    info!("Releasing project mutex for project: {}", project_id);
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let url = format!("{}api/v1/projects/{}/release/", base, project_id);
    let client = get_api_client();

    // Fire and forget
    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;

    let status = resp.status();
    if status.is_success() {
        log::info!("Successfully released mutex for project: {}", project_id);
        Ok(())
    } else {
        log::warn!("Mutex release returned status {}: {}", status.as_u16(), url);
        Err(Error::Api(status.as_u16()))
    }
}

pub async fn fetch_projects(api_info: &ApiInfo) -> Result<Vec<ProjectInfo>, Error> {
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let url = base.join("api/v1/projects/").unwrap();
    info!("Fetching projects from server: {url}");
    let client = get_api_client();

    let resp = client
        .get(url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;

    let status = resp.status();

    #[derive(Deserialize)]
    pub struct ProjectsResponse {
        pub data: Vec<ProjectInfo>,
        // Ignore extra fields like timestamp and url
    }

    if status.is_success() {
        let mut projects = resp
            .json::<ProjectsResponse>()
            .await
            .map_err(|e| Error::Deserialization(e.to_string()))?
            .data;
        // Filter to only Compass projects
        projects.retain(|project| project.project_type == ProjectType::Compass);
        Ok(projects)
    } else {
        Err(Error::Api(status.as_u16()))
    }
}

pub async fn fetch_project_info(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<ProjectInfo, Error> {
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let url = base
        .join(&format!("api/v1/projects/{}/", project_id))
        .unwrap();
    info!("Fetching project info from server: {url}");
    let client = get_api_client();

    let resp = client
        .get(url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;

    let status = resp.status();

    #[derive(Deserialize)]
    pub struct ProjectInfoResponse {
        pub data: ProjectInfo,
        // Ignore extra fields like timestamp and url
    }

    if status.is_success() {
        Ok(resp
            .json::<ProjectInfoResponse>()
            .await
            .map_err(|e| Error::Deserialization(e.to_string()))?
            .data)
    } else {
        Err(Error::Api(status.as_u16()))
    }
}

pub async fn download_project_zip(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<bytes::Bytes, Error> {
    info!("Downloading project zip for project: {project_id}");
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let url = format!(
        "{}api/v1/projects/{}/download/compass_zip/",
        base, project_id
    );
    let client = get_api_client();

    let resp = client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;

    let status = resp.status();

    // Handle 422 - Project has no compass data yet (new/empty project)
    if status.as_u16() == 422 {
        return Err(Error::NoProjectData(project_id));
    }

    if !status.is_success() {
        return Err(Error::Api(status.as_u16()));
    }

    // Get the bytes
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| Error::Deserialization(e.to_string()))?;
    Ok(bytes)
}

pub async fn upload_project_zip(
    api_info: &ApiInfo,
    project_id: Uuid,
    commit_message: String,
    zip_path: &Path,
) -> Result<ProjectSaveResult, Error> {
    log::info!("Uploading project ZIP for project: {}", project_id);
    let base = api_info.instance();
    let oauth = api_info.oauth_token().ok_or(Error::NoAuthToken)?;
    let url = format!(
        "{}/api/v1/projects/{}/upload/compass_zip/",
        base, project_id
    );
    let client = get_api_client();
    // Read ZIP file
    let zip_bytes = std::fs::read(&zip_path).map_err(|e| Error::FileRead(e.to_string()))?;

    // Create multipart form
    let part = reqwest::multipart::Part::bytes(zip_bytes)
        .file_name("project.zip")
        .mime_str("application/zip")
        .unwrap();

    let form = reqwest::multipart::Form::new()
        .text("message", commit_message)
        .part("artifact", part);

    info!("Uploading project to: {}", url);

    let resp = client
        .put(&url)
        .header("Authorization", format!("Token {}", oauth))
        .multipart(form)
        .send()
        .await
        .map_err(|e| Error::NetworkRequest(e.to_string()))?;

    let status = resp.status();

    if status.is_success() {
        info!("Successfully uploaded project: {}", project_id);
        Ok(ProjectSaveResult::Saved)
    } else if status == reqwest::StatusCode::NOT_MODIFIED {
        info!("No changes to upload for project: {}", project_id);
        Ok(ProjectSaveResult::NoChanges)
    } else {
        Err(Error::Api(status.as_u16()))
    }
}
