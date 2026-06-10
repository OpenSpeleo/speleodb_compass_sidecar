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
#[non_exhaustive]
pub enum ProjectType {
    Ariane,
    Compass,
    /// Any project type the SpeleoDB server defines that this client does not
    /// model. New server-side types (e.g. `"OTHER"`) decode here instead of
    /// failing deserialization of the entire project list. The sidecar ignores
    /// everything that isn't [`ProjectType::Compass`], so these simply get
    /// filtered out downstream.
    #[serde(other)]
    Other,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression: the SpeleoDB v2 API can return project types beyond the
    /// ones this client knows about (e.g. `"OTHER"`). Because the project list
    /// is deserialized as a whole `Vec<ProjectInfo>` *before* any filtering, a
    /// single unknown `type` value used to fail the entire response, which
    /// stalled app launch on the loading screen. Unknown types must decode to
    /// `ProjectType::Other` instead of erroring.
    #[test]
    fn project_type_deserializes_unknown_value_as_other() {
        let ty: ProjectType = serde_json::from_str("\"OTHER\"").expect("unknown type must decode");
        assert_eq!(ty, ProjectType::Other);
    }

    #[test]
    fn project_info_with_unknown_type_deserializes() {
        // The exact offending object captured from the server (a project with
        // `"type": "OTHER"` plus newly-added fields like `color`/`commit_count`).
        let json = r##"{
            "id": "9e12fe62-ad38-471b-a625-7ed9960ab3e4",
            "country": "US",
            "visibility": "PRIVATE",
            "commit_count": 0,
            "latest_commit": null,
            "permission": "ADMIN",
            "active_mutex": null,
            "created_by": "matt.hansen@karstunderwater.org",
            "type": "OTHER",
            "name": "South Pole Cave",
            "description": "South end of equator pond, Chaz",
            "color": "#4daf4a",
            "exclude_geojson": false,
            "is_active": true,
            "creation_date": "2026-02-27T10:08:23.160083-05:00",
            "modified_date": "2026-02-27T10:18:12.889598-05:00",
            "fork_from": null
        }"##;
        let info: ProjectInfo = serde_json::from_str(json)
            .expect("unknown project type must not fail the whole object");
        assert_eq!(info.project_type, ProjectType::Other);
        assert_eq!(info.name, "South Pole Cave");
    }

    #[test]
    fn mixed_project_list_with_unknown_type_deserializes() {
        // A list containing a type the client doesn't model must still decode
        // so the known (COMPASS) projects survive the later filtering step.
        let json = r#"[
            {"id":"9e12fe62-ad38-471b-a625-7ed9960ab3e4","country":"US","visibility":"PRIVATE",
             "latest_commit":null,"permission":"ADMIN","active_mutex":null,"created_by":"a@b.c",
             "type":"OTHER","name":"Other Project","description":"","exclude_geojson":false,
             "is_active":true,"creation_date":"2026-01-01T00:00:00Z","modified_date":"2026-01-01T00:00:00Z",
             "fork_from":null},
            {"id":"00000000-0000-0000-0000-000000000001","country":"US","visibility":"PRIVATE",
             "latest_commit":null,"permission":"ADMIN","active_mutex":null,"created_by":"a@b.c",
             "type":"COMPASS","name":"Compass Project","description":"","exclude_geojson":false,
             "is_active":true,"creation_date":"2026-01-01T00:00:00Z","modified_date":"2026-01-01T00:00:00Z",
             "fork_from":null}
        ]"#;
        let projects: Vec<ProjectInfo> =
            serde_json::from_str(json).expect("list with an unknown type must decode");
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].project_type, ProjectType::Other);
        assert_eq!(projects[1].project_type, ProjectType::Compass);
    }

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
