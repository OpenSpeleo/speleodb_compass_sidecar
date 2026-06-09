use serde::{Deserialize, Deserializer, Serialize, Serializer};
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

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    Compass,
    Ignored,
}

impl ProjectType {
    pub fn is_compass(&self) -> bool {
        matches!(self, Self::Compass)
    }
}

impl Serialize for ProjectType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Compass => serializer.serialize_str("COMPASS"),
            Self::Ignored => serializer.serialize_str("IGNORED"),
        }
    }
}

impl<'de> Deserialize<'de> for ProjectType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "COMPASS" {
            Ok(Self::Compass)
        } else {
            Ok(Self::Ignored)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT_JSON_TEMPLATE: &str = r##"
{
  "id": "9e12fe62-ad38-471b-a625-7ed9960ab3e4",
  "country": "US",
  "visibility": "PRIVATE",
  "permission": "ADMIN",
  "active_mutex": null,
  "created_by": "matt.hansen@karstunderwater.org",
  "type": "__PROJECT_TYPE__",
  "name": "South Pole Cave",
  "description": "South end of equator pond, Chaz",
  "exclude_geojson": false,
  "is_active": true,
  "creation_date": "2026-02-27T10:08:23.160083-05:00",
  "modified_date": "2026-02-27T10:18:12.889598-05:00",
  "fork_from": null,
  "latest_commit": null
}
"##;

    fn project_json(project_type: &str) -> String {
        PROJECT_JSON_TEMPLATE.replace("__PROJECT_TYPE__", project_type)
    }

    #[test]
    fn project_info_with_unknown_type_deserializes_as_ignored() {
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

        assert_eq!(info.project_type, ProjectType::Ignored);
        assert_eq!(info.name, "South Pole Cave");
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

    #[test]
    fn project_type_deserializes_compass_as_supported() {
        let project: ProjectInfo = serde_json::from_str(&project_json("COMPASS"))
            .expect("COMPASS project should deserialize");
        assert_eq!(project.project_type, ProjectType::Compass);
        assert!(project.project_type.is_compass());
    }

    #[test]
    fn project_type_deserializes_ariane_as_ignored() {
        let project: ProjectInfo = serde_json::from_str(&project_json("ARIANE"))
            .expect("ARIANE project should deserialize as ignored");
        assert_eq!(project.project_type, ProjectType::Ignored);
        assert!(!project.project_type.is_compass());
    }

    #[test]
    fn project_type_deserializes_other_as_ignored() {
        let project: ProjectInfo = serde_json::from_str(&project_json("OTHER"))
            .expect("OTHER project should deserialize as ignored");
        assert_eq!(project.project_type, ProjectType::Ignored);
        assert!(!project.project_type.is_compass());
    }

    #[test]
    fn project_type_serializes_compass_for_create_requests() {
        let serialized =
            serde_json::to_string(&ProjectType::Compass).expect("ProjectType should serialize");
        assert_eq!(serialized, "\"COMPASS\"");
    }
}
