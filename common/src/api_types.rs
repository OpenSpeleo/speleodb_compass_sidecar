use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ActiveMutex {
    pub user: String,
    pub creation_date: String,
    pub modified_date: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ProjectInfo {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub is_active: bool,
    pub permission: String,
    pub active_mutex: Option<ActiveMutex>,
    pub country: String,
    pub created_by: String,
    pub creation_date: String,
    pub modified_date: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latitude: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub longitude: Option<f64>,
    pub fork_from: Option<String>,
    pub visibility: String,
    pub exclude_geojson: bool,
    pub latest_commit: Option<CommitInfo>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author_name: String,
    pub dt_since: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProjectSaveResult {
    Saved,
    NoChanges,
}
