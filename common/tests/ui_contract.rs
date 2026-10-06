//! The React boundary consumes the existing Serde wire representation unchanged.
use std::path::{Path, PathBuf};

use common::{
    Error,
    api_types::{
        ActiveMutex, CommitInfo, CommitTreeEntry, ProjectInfo, ProjectSaveResult, ProjectType,
    },
    compass_import::{ImportDependency, ImportPreview, ImportSection, InitialImportOutcome},
    ui_state::{
        LoadingState, LocalProjectStatus, Platform, ProjectStatus, UiState, UpdateNotification,
        UpdateNotificationPhase,
    },
};
use serde_json::{Value, json};
use uuid::Uuid;

fn project() -> ProjectInfo {
    ProjectInfo {
        id: Uuid::nil(),
        name: "Cave".into(),
        description: "Survey".into(),
        is_active: true,
        permission: "ADMIN".into(),
        active_mutex: None,
        country: "US".into(),
        created_by: "surveyor@example.com".into(),
        creation_date: "2026-01-01".into(),
        modified_date: "2026-02-01".into(),
        latitude: None,
        longitude: None,
        fork_from: None,
        visibility: "PRIVATE".into(),
        exclude_geojson: false,
        latest_commit: None,
        project_type: ProjectType::Compass,
    }
}

fn errors() -> Vec<Error> {
    vec![
        Error::NoAuthToken,
        Error::ProjectAlreadyExists(PathBuf::from("survey")),
        Error::ProjectNotFound(PathBuf::from("survey")),
        Error::CreateDirectory(PathBuf::from("survey")),
        Error::Deserialization("detail".into()),
        Error::Serialization("detail".into()),
        Error::NoUserPreferences,
        Error::ApiInfoRead(PathBuf::from("prefs")),
        Error::ApiInfoWrite(PathBuf::from("prefs")),
        Error::ProjectImport {
            src_path: PathBuf::from("A.DAT"),
            dst_path: PathBuf::from("B.DAT"),
            details: "denied".into(),
            is_permission_error: false,
        },
        Error::ProjectImport {
            src_path: PathBuf::from("A.DAT"),
            dst_path: PathBuf::from("B.DAT"),
            details: "denied".into(),
            is_permission_error: true,
        },
        Error::ProjectWrite(PathBuf::from("survey")),
        Error::FilePermissionSet,
        Error::NoProjectSelected,
        Error::ProjectFileNotFound(PathBuf::from("survey")),
        Error::EmptyProjectDirectory(Uuid::nil()),
        Error::NetworkRequest("detail".into()),
        Error::Unauthorized("detail".into()),
        Error::NotFound("detail".into()),
        Error::Unprocessable("detail".into()),
        Error::Conflict("detail".into()),
        Error::Api {
            status: 503,
            message: "detail".into(),
        },
        Error::FileRead("detail".into()),
        Error::FileWrite("detail".into()),
        Error::NoProjectData(Uuid::nil()),
        Error::ProjectMutexLocked(Uuid::nil()),
        Error::ZipFile("detail".into()),
        Error::OsCommand("detail".into()),
        Error::CompassNotFound,
        Error::CompassExecutable("detail".into()),
        Error::CompassProject("detail".into()),
        Error::ImportSourceChanged("A.DAT changed".into()),
        Error::NoAppHandle,
    ]
}

fn contract() -> Value {
    let minimal = project();
    let mut complete = minimal.clone();
    complete.active_mutex = Some(ActiveMutex {
        user: "surveyor@example.com".into(),
        creation_date: "2026-01-01".into(),
        modified_date: "2026-02-01".into(),
    });
    complete.latitude = Some(45.5);
    complete.longitude = Some(-73.5);
    complete.fork_from = Some("source".into());
    complete.latest_commit = Some(CommitInfo {
        id: "abc123".into(),
        message: "Surveyed".into(),
        author_name: "Surveyor".into(),
        commit_date: Some("2026-02-01T12:00:00-05:00".into()),
        dt_since: "just now".into(),
        tree: vec![CommitTreeEntry {}],
    });
    let local_statuses = [
        LocalProjectStatus::Unknown,
        LocalProjectStatus::RemoteOnly,
        LocalProjectStatus::EmptyLocal,
        LocalProjectStatus::Dirty,
        LocalProjectStatus::UpToDate,
        LocalProjectStatus::OutOfDate,
        LocalProjectStatus::DirtyAndOutOfDate,
    ];
    let states: Vec<_> = [Platform::Windows, Platform::MacOS, Platform::Linux]
        .into_iter()
        .map(|platform| UiState {
            loading_state: LoadingState::Ready,
            platform,
            user_email: Some("surveyor@example.com".into()),
            project_status: local_statuses
                .into_iter()
                .map(|status| ProjectStatus::new(status, complete.clone()))
                .collect(),
            selected_project_id: Some(Uuid::nil()),
            compass_open: true,
            project_downloading: true,
            update_notification: Some(UpdateNotification::new(
                7,
                UpdateNotificationPhase::Checking,
            )),
        })
        .collect();
    let phases = vec![
        UpdateNotificationPhase::Checking,
        UpdateNotificationPhase::Downloading {
            version: "26.9.24".into(),
            progress_percent: None,
        },
        UpdateNotificationPhase::Downloading {
            version: "26.9.24".into(),
            progress_percent: Some(0),
        },
        UpdateNotificationPhase::Downloading {
            version: "26.9.24".into(),
            progress_percent: Some(100),
        },
        UpdateNotificationPhase::Installing {
            version: "26.9.24".into(),
        },
        UpdateNotificationPhase::Relaunching {
            version: "26.9.24".into(),
        },
        UpdateNotificationPhase::UpToDate {
            app_name: "SpeleoDB Compass Sidecar".into(),
        },
        UpdateNotificationPhase::Failed {
            message: "offline".into(),
        },
    ];
    let notifications: Vec<_> = phases
        .into_iter()
        .map(|phase| {
            let notification = UpdateNotification::new(7, phase);
            json!({ "payload": notification, "dismissal_key": notification.dismissal_key() })
        })
        .collect();
    let empty = UiState {
        platform: Platform::Windows,
        ..UiState::default()
    };
    json!({
        "empty_state": empty,
        "ui_states": states,
        "projects": [minimal, complete],
        "project_types": [ProjectType::Compass, ProjectType::Ignored],
        "commit_without_date": CommitInfo { id: "abc".into(), message: "Saved".into(), author_name: "Surveyor".into(), commit_date: None, dt_since: "now".into(), tree: vec![] },
        "loading_states": [LoadingState::NotStarted, LoadingState::LoadingPrefs, LoadingState::Authenticating, LoadingState::LoadingProjects, LoadingState::Unauthenticated, LoadingState::Ready, LoadingState::Failed(Error::NetworkRequest("offline".into()))],
        "notifications": notifications,
        "preview": ImportPreview { preview_id: Uuid::nil(), source_name: "Cave.mak".into(), source_directory: "Surveys".into(), sections: vec![ImportSection { id: 0, name: "A.DAT".into(), relative_path: "A.DAT".into(), dependencies: vec![] }, ImportSection { id: 1, name: "B.DAT".into(), relative_path: "B.DAT".into(), dependencies: vec![ImportDependency { section_id: 0, reason: "Shared station".into() }] }], full_import_reason: None },
        "import_outcomes": [InitialImportOutcome::Synced { save_result: ProjectSaveResult::Saved }, InitialImportOutcome::Synced { save_result: ProjectSaveResult::NoChanges }, InitialImportOutcome::LocalOnly { error: Error::NetworkRequest("offline".into()) }, InitialImportOutcome::UploadedNeedsRefresh { error: Error::Unauthorized("expired".into()) }],
        "errors": errors().into_iter().map(|error| json!({ "payload": error, "display": error.to_string() })).collect::<Vec<_>>()
    })
}

#[test]
fn react_fixtures_match_serde_and_rust_display() {
    let expected = contract();
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../app/src/lib/__fixtures__/ui-contract.json");
    if std::env::var_os("UPDATE_UI_CONTRACT_FIXTURES").is_some() {
        std::fs::write(
            &path,
            format!("{}\n", serde_json::to_string_pretty(&expected).unwrap()),
        )
        .unwrap();
    }
    let fixture: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(
        fixture, expected,
        "Regenerate intentionally with UPDATE_UI_CONTRACT_FIXTURES=1"
    );
    for state in fixture["ui_states"].as_array().unwrap() {
        let decoded: UiState = serde_json::from_value(state.clone()).unwrap();
        assert_eq!(serde_json::to_value(decoded).unwrap(), *state);
    }
    for entry in fixture["errors"].as_array().unwrap() {
        let decoded: Error = serde_json::from_value(entry["payload"].clone()).unwrap();
        assert_eq!(decoded.to_string(), entry["display"].as_str().unwrap());
    }
}
