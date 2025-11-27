use std::path::Path;

use crate::{Error, api_info::ApiInfo, get_api_client};
use common::api_types::{ProjectInfo, ProjectRevisionInfo, ProjectSaveResult};
use log::info;
use serde::Deserialize;
use uuid::Uuid;

pub async fn acquire_project_mutex(api_info: &ApiInfo, project_id: Uuid) -> Result<(), String> {
    log::info!("Acquiring project mutex for project: {}", project_id);
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token().map_err(|e| e.to_string())?;
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

pub async fn create_project(
    api_info: &ApiInfo,
    name: String,
    description: String,
    country: String,
    latitude: Option<String>,
    longitude: Option<String>,
) -> Result<ProjectInfo, Error> {
    log::info!("Creating new project: {}", name);
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token()?;
    let url = format!("{}{}", base, "/api/v1/projects/");
    let client = get_api_client();

    let mut body = serde_json::Map::new();
    body.insert("name".to_string(), serde_json::json!(name));
    body.insert("description".to_string(), serde_json::json!(description));
    body.insert("country".to_string(), serde_json::json!(country));
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

    log::info!("Creating project: {}", name);

    let resp = client
        .post(&url)
        .header("Authorization", format!("Token {}", oauth))
        .json(&body)
        .send()
        .await?;

    let status = resp.status();

    if status.is_success() {
        #[derive(Deserialize)]
        pub struct ProjectInfoResponse {
            pub data: ProjectInfo,
            // Ignore extra fields like timestamp and url
        }
        let json = resp.json::<ProjectInfoResponse>().await?;

        // Return the project data wrapped in our standard format
        Ok(json.data)
    } else {
        Err(Error::Api(status.as_u16()))
    }
}

pub async fn release_project_mutex(api_info: &ApiInfo, project_id: &Uuid) -> Result<(), String> {
    info!("Releasing project mutex for project: {}", project_id);
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token().map_err(|e| e.to_string())?;
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
    let oauth = api_info.get_api_token().map_err(|e| e.to_string())?;
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

pub async fn get_project_revisions(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<ProjectRevisionInfo, String> {
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token().map_err(|e| e.to_string())?;
    let url = format!("{}/api/v1/projects/{}/revisions/", base, project_id);
    let client = get_api_client();

    let resp = client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| format!("Network request failed: {}", e))?;

    let status = resp.status();

    #[derive(Deserialize)]
    struct RevisionsResponse {
        pub data: ProjectRevisionInfo,
    }

    if status.is_success() {
        match resp.json::<RevisionsResponse>().await {
            Ok(revisions_response) => Ok(revisions_response.data),
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        }
    } else {
        Err(format!("Request failed with status {}", status.as_u16()))
    }
}

pub async fn download_project_zip(
    api_info: &ApiInfo,
    project_id: Uuid,
) -> Result<bytes::Bytes, Error> {
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token()?;
    let url = format!(
        "{}/api/v1/projects/{}/download/compass_zip/",
        base, project_id
    );
    let client = get_api_client();

    let resp = client
        .get(&url)
        .header("Authorization", format!("Token {}", oauth))
        .send()
        .await
        .map_err(|e| Error::Request(e))?;

    let status = resp.status();

    // Handle 422 - Project has no compass data yet (new/empty project)
    if status.as_u16() == 422 {
        return Err(Error::NoProjectData(project_id));
    }

    if !status.is_success() {
        return Err(Error::Api(status.as_u16()));
    }

    // Get the bytes
    let bytes = resp.bytes().await.map_err(|e| Error::Request(e))?;
    Ok(bytes)
}

pub async fn upload_project_zip(
    api_info: &ApiInfo,
    project_id: Uuid,
    commit_message: String,
    zip_path: &Path,
) -> Result<ProjectSaveResult, Error> {
    log::info!("Uploading project ZIP for project: {}", project_id);
    let base = api_info.get_api_instance();
    let oauth = api_info.get_api_token()?;
    let url = format!(
        "{}/api/v1/projects/{}/upload/compass_zip/",
        base, project_id
    );
    let client = get_api_client();
    // Read ZIP file
    let zip_bytes = std::fs::read(&zip_path).map_err(Error::Io)?;

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
        .map_err(|e| Error::Request(e))?;

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
