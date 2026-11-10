//! Project the SDK's wire models into the existing desktop/IPC contract.
//!
//! Keep generated enums and timestamp types at this boundary so upgrading the
//! SDK does not change persisted project metadata or frontend payload shapes.

use common::{
    Error,
    api_types::{ActiveMutex, CommitInfo, CommitTreeEntry, ProjectInfo, ProjectType},
};
use speleodb_api::types::{Project, ProjectActiveMutex, ProjectLatestCommit};

pub(crate) fn project_info(project: Project) -> Result<ProjectInfo, Error> {
    // Desktop permissions have always been required. Do not turn a missing or
    // explicitly null SDK permission into a usable-looking project snapshot.
    let permission = project
        .permission
        .flatten()
        .ok_or_else(|| Error::Deserialization("Project permission is missing or null".into()))?;
    let active_mutex = project
        .active_mutex
        .map(|mutex| match mutex {
            ProjectActiveMutex::ProjectActiveMutexInlineVariant(mutex) => Ok(ActiveMutex {
                user: mutex.user,
                creation_date: mutex.creation_date.to_rfc3339(),
                modified_date: mutex.modified_date.to_rfc3339(),
            }),
            ProjectActiveMutex::String(_) => Err(Error::Deserialization(
                "Project active_mutex must be an object or null".into(),
            )),
        })
        .transpose()?;
    let latest_commit = project
        .latest_commit
        .map(|commit| match commit {
            ProjectLatestCommit::ProjectLatestCommitInlineVariant(commit) => Ok(CommitInfo {
                id: commit.id,
                message: commit.message,
                author_name: commit.author_name,
                // The v2 contract exposes authored_date, not commit_date. Keep
                // the existing optional field absent rather than substitute a
                // distinct authored timestamp.
                commit_date: None,
                dt_since: commit.dt_since,
                // The desktop uses emptiness to detect an empty project even
                // though it deliberately stores no per-entry Git tree fields.
                tree: commit
                    .tree
                    .into_iter()
                    .map(|_| CommitTreeEntry {})
                    .collect(),
            }),
            ProjectLatestCommit::String(_) => Err(Error::Deserialization(
                "Project latest_commit must be an object or null".into(),
            )),
        })
        .transpose()?;

    Ok(ProjectInfo {
        id: project.id,
        name: project.name,
        description: project.description,
        is_active: project.is_active,
        permission,
        active_mutex,
        country: project.country.to_string(),
        created_by: project.created_by,
        creation_date: project.creation_date.to_rfc3339(),
        modified_date: project.modified_date.to_rfc3339(),
        latitude: project.latitude,
        longitude: project.longitude,
        fork_from: project.fork_from.map(|id| id.to_string()),
        visibility: project.visibility.to_string(),
        exclude_geojson: project.exclude_geojson,
        latest_commit,
        project_type: if project.r#type == speleodb_api::types::ProjectType::Compass {
            ProjectType::Compass
        } else {
            ProjectType::Ignored
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn project_json() -> Value {
        json!({
            "id": "9e12fe62-ad38-471b-a625-7ed9960ab3e4",
            "name": "South Pole Cave",
            "description": "Survey project",
            "is_active": true,
            "permission": "ADMIN",
            "active_mutex": null,
            "country": "US",
            "created_by": "test@example.invalid",
            "creation_date": "2026-02-27T10:08:23.160083-05:00",
            "modified_date": "2026-02-27T10:18:12-05:00",
            "latitude": 28.5,
            "longitude": -82.5,
            "fork_from": "de629745-fdc0-4768-aa0b-07f637339de0",
            "visibility": "PUBLIC",
            "exclude_geojson": false,
            "latest_commit": null,
            "type": "COMPASS",
            "color": "#4daf4a",
            "commit_count": 0
        })
    }

    #[test]
    fn projects_keep_the_desktop_contract_and_normalize_timestamp_offsets() {
        let json = project_json();
        let mut expected: ProjectInfo = serde_json::from_value(json.clone()).unwrap();
        expected.creation_date = "2026-02-27T15:08:23.160083+00:00".into();
        expected.modified_date = "2026-02-27T15:18:12+00:00".into();
        let projected = project_info(serde_json::from_value(json).unwrap()).unwrap();
        assert_eq!(projected, expected);
        let serialized = serde_json::to_value(projected).unwrap();
        assert_eq!(serialized["type"], "COMPASS");
        assert!(serialized.get("color").is_none());
        assert!(serialized.get("commit_count").is_none());
    }

    #[test]
    fn known_non_compass_project_types_remain_ignored() {
        for project_type in [
            "ARIANE",
            "STALAGMIGHT",
            "STICKMAPS",
            "THERION",
            "WALLS",
            "OTHER",
        ] {
            let mut json = project_json();
            json["type"] = project_type.into();
            let project = project_info(serde_json::from_value(json).unwrap()).unwrap();
            assert_eq!(project.project_type, ProjectType::Ignored);
        }
    }

    #[test]
    fn missing_or_null_permissions_still_fail_deserialization() {
        for permission in [None, Some(Value::Null)] {
            let mut json = project_json();
            if let Some(permission) = permission {
                json["permission"] = permission;
            } else {
                json.as_object_mut().unwrap().remove("permission");
            }
            let project: Project = serde_json::from_value(json).unwrap();
            assert!(matches!(
                project_info(project),
                Err(Error::Deserialization(_))
            ));
        }
    }

    #[test]
    fn mutex_and_commit_objects_preserve_lock_and_project_emptiness_information() {
        let mut json = project_json();
        json["active_mutex"] = json!({
            "user": "editor@example.invalid",
            "creation_date": "2026-02-27T10:08:23-05:00",
            "modified_date": "2026-02-27T10:18:12-05:00"
        });
        json["latest_commit"] = json!({
            "author_email": "editor@example.invalid",
            "author_name": "Editor",
            "authored_date": "2026-02-27T15:18:00Z",
            "dt_since": "a minute ago",
            "formats": [],
            "id": "0123456789abcdef0123456789abcdef01234567",
            "message": "Upload Compass survey",
            "parent_ids": [],
            "tree": [{
                "mode": "100644",
                "object": "fedcba9876543210fedcba9876543210fedcba98",
                "path": "SURVEY.DAT",
                "type": "blob"
            }],
            "url": null
        });
        let mut expected: ProjectInfo = serde_json::from_value(json.clone()).unwrap();
        expected.creation_date = "2026-02-27T15:08:23.160083+00:00".into();
        expected.modified_date = "2026-02-27T15:18:12+00:00".into();
        let mutex = expected.active_mutex.as_mut().unwrap();
        mutex.creation_date = "2026-02-27T15:08:23+00:00".into();
        mutex.modified_date = "2026-02-27T15:18:12+00:00".into();
        let project = project_info(serde_json::from_value(json).unwrap()).unwrap();
        assert_eq!(project, expected);
        assert_eq!(project.latest_commit.as_ref().unwrap().tree.len(), 1);
        let serialized = serde_json::to_value(project).unwrap();
        assert!(serialized["latest_commit"].get("commit_date").is_none());
        assert!(serialized["latest_commit"].get("tree").is_none());
    }

    #[test]
    fn string_mutex_or_commit_values_are_rejected_instead_of_dropping_state() {
        for field in ["active_mutex", "latest_commit"] {
            let mut json = project_json();
            json[field] = "unexpected reference".into();
            let project: Project = serde_json::from_value(json).unwrap();
            assert!(matches!(
                project_info(project),
                Err(Error::Deserialization(_))
            ));
        }
    }
}
