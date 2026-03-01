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
    #[serde(rename = "type")]
    pub project_type: ProjectType,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CommitInfo {
    pub id: String,
    pub message: String,
    pub author_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit_date: Option<String>,
    pub dt_since: String,
    #[serde(default, skip_serializing)]
    pub tree: Vec<CommitTreeEntry>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CommitTreeEntry {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ProjectSaveResult {
    Saved,
    NoChanges,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectType {
    Ariane,
    Compass,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commit_info_deserializes_without_commit_date() {
        let toml_source = r#"
id = "abc123"
message = "Imported local project"
author_name = "Test User"
dt_since = "just now"
"#;

        let commit: CommitInfo = toml::from_str(toml_source)
            .expect("commit info should deserialize without commit_date");
        assert_eq!(commit.commit_date, None);
    }

    #[test]
    fn commit_info_skips_serializing_absent_commit_date() {
        let commit = CommitInfo {
            id: "abc123".to_string(),
            message: "Imported local project".to_string(),
            author_name: "Test User".to_string(),
            commit_date: None,
            dt_since: "just now".to_string(),
            tree: vec![],
        };

        let serialized = toml::to_string(&commit).expect("commit info should serialize");
        assert!(
            !serialized.contains("commit_date"),
            "commit_date should be omitted when not present"
        );
    }
}
