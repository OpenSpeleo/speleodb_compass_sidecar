//! Project-bound preview sessions and the staged initial-import transaction.

use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, atomic::Ordering},
};

use common::{
    ApiInfo, Error,
    api_types::ProjectInfo,
    compass_import::{ImportPreview, InitialImportOutcome},
};
use tauri::AppHandle;
use uuid::Uuid;

use crate::{
    paths::{compass_project_path, compass_project_working_path},
    project_management::{
        ProjectManager,
        import::{self, AnalyzedImport},
    },
    state::{AppState, SIGN_OUT_MENU_ID},
};

pub(crate) enum ProjectSaveFailure {
    Upload(Error),
    Synchronization(Error),
}

impl ProjectSaveFailure {
    pub(crate) fn into_error(self) -> Error {
        match self {
            Self::Upload(error) | Self::Synchronization(error) => error,
        }
    }

    fn into_import_outcome(self) -> InitialImportOutcome {
        match self {
            Self::Upload(error) => InitialImportOutcome::LocalOnly { error },
            Self::Synchronization(error) => InitialImportOutcome::UploadedNeedsRefresh { error },
        }
    }
}

struct StoredPreview {
    id: Uuid,
    project_id: Uuid,
    api_info: ApiInfo,
    analysis: Arc<AnalyzedImport>,
}

#[derive(Default)]
pub(crate) struct ImportPreviewStore {
    generation: u64,
    preview: Option<StoredPreview>,
}

impl ImportPreviewStore {
    fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.preview = None;
    }
}

impl AppState {
    pub(crate) fn try_project_operation(&self) -> Result<tokio::sync::MutexGuard<'_, ()>, Error> {
        self.project_operations.try_lock().map_err(|_| {
            Error::Conflict(
                "Another project action is running. Please wait for it to finish.".into(),
            )
        })
    }

    pub(crate) fn invalidate_import_previews(&self) {
        self.import_previews.lock().unwrap().invalidate();
    }

    pub(crate) fn cancel_import_preview(&self, preview_id: Uuid) {
        let mut store = self.import_previews.lock().unwrap();
        if store
            .preview
            .as_ref()
            .is_some_and(|preview| preview.id == preview_id)
        {
            store.invalidate();
        }
    }

    pub(crate) fn ensure_active_project(&self, project_id: Uuid) -> Result<(), Error> {
        if self.get_active_project_id() != Some(project_id) {
            return Err(Error::CompassProject(
                "The selected project changed. Open the project and try again.".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn initial_import_is_running(&self) -> bool {
        self.initial_import_running.load(Ordering::SeqCst)
    }

    pub(crate) async fn preview_initial_import(
        &self,
        project_id: Uuid,
        mak_path: PathBuf,
    ) -> Result<ImportPreview, Error> {
        let (generation, api_info) = {
            let _operation = self.project_operations.lock().await;
            self.ensure_active_project(project_id)?;
            let api_info = self.api_info();
            let info = self
                .get_project_info(project_id)
                .ok_or(Error::NoProjectSelected)?;
            validate_initial_target(&info, &api_info, self.compass_is_open())?;
            require_empty_directory(&compass_project_working_path(project_id))?;
            let mut store = self.import_previews.lock().unwrap();
            store.invalidate();
            (store.generation, api_info)
        };
        let analysis = Arc::new(
            tauri::async_runtime::spawn_blocking(move || import::analyze(&mak_path))
                .await
                .map_err(|error| Error::OsCommand(error.to_string()))??,
        );
        // Background refreshes also own this gate while fetching project data.
        // Wait for them instead of throwing away a completed source analysis.
        let _operation = self.project_operations.lock().await;
        self.ensure_active_project(project_id)?;
        let mut store = self.import_previews.lock().unwrap();
        if store.generation != generation || !same_session(&api_info, &self.api_info()) {
            return Err(Error::CompassProject(
                "This import preview is no longer current. Choose the file again.".into(),
            ));
        }
        let id = Uuid::new_v4();
        let preview = analysis.preview(id);
        store.preview = Some(StoredPreview {
            id,
            project_id,
            api_info,
            analysis,
        });
        Ok(preview)
    }

    pub(crate) async fn confirm_initial_import(
        &self,
        app_handle: &AppHandle,
        preview_id: Uuid,
        selected_section_ids: Vec<usize>,
    ) -> Result<InitialImportOutcome, Error> {
        let _operation = self.project_operations.lock().await;
        let (project_id, api_info, analysis) = {
            let store = self.import_previews.lock().unwrap();
            let preview = store
                .preview
                .as_ref()
                .filter(|preview| preview.id == preview_id)
                .ok_or_else(|| {
                    Error::CompassProject("This preview expired. Choose the file again.".into())
                })?;
            self.ensure_active_project(preview.project_id)?;
            if !same_session(&preview.api_info, &self.api_info()) {
                return Err(Error::Unauthorized(
                    "The signed-in account changed. Choose the file again.".into(),
                ));
            }
            (
                preview.project_id,
                preview.api_info.clone(),
                Arc::clone(&preview.analysis),
            )
        };
        let _running = InitialImportGuard::new(self, app_handle.clone());
        let info = api::project::fetch_project_info(&api_info, project_id).await?;
        validate_initial_target(&info, &api_info, self.compass_is_open())?;
        require_empty_directory(&compass_project_working_path(project_id))?;
        self.set_project_info(info);

        // No project files are published until every selected input and its
        // content fingerprint have been verified in an isolated sibling stage.
        tauri::async_runtime::spawn_blocking(move || {
            let stage = ImportStage::new(&compass_project_path(project_id))?;
            analysis.stage(project_id, &selected_section_ids, &stage.path)?;
            stage.publish(&compass_project_working_path(project_id))
        })
        .await
        .map_err(|error| Error::OsCommand(error.to_string()))??;

        // From here forward errors represent a recoverable local import. Never
        // make a source-import retry available after publication.
        self.invalidate_import_previews();
        let outcome = match self
            .save_project_by_id(project_id, &api_info, "Imported local project".into())
            .await
        {
            Ok(save_result) => InitialImportOutcome::Synced { save_result },
            Err(error) => error.into_import_outcome(),
        };
        self.emit_app_state_change().await;
        Ok(outcome)
    }
}

fn same_session(left: &ApiInfo, right: &ApiInfo) -> bool {
    left.instance() == right.instance()
        && left.email() == right.email()
        && left.oauth_token() == right.oauth_token()
}

fn validate_initial_target(
    info: &ProjectInfo,
    api: &ApiInfo,
    compass_open: bool,
) -> Result<(), Error> {
    if compass_open {
        return Err(Error::CompassProject(
            "Close Compass before importing.".into(),
        ));
    }
    if !info.project_type.is_compass()
        || !matches!(info.permission.as_str(), "ADMIN" | "READ_AND_WRITE")
    {
        return Err(Error::Unauthorized("This project is not editable.".into()));
    }
    let email = api.email().ok_or(Error::NoAuthToken)?;
    if api.oauth_token().is_none() {
        return Err(Error::NoAuthToken);
    }
    if !info
        .active_mutex
        .as_ref()
        .is_some_and(|mutex| mutex.user == email)
    {
        return Err(Error::ProjectMutexLocked(info.id));
    }
    if ProjectManager::initialize_from_info(info.clone()).remote_project_has_compass_data() {
        return Err(Error::CompassProject(
            "This project already contains data. Reopen it to view the latest version.".into(),
        ));
    }
    Ok(())
}

/// Refuse links and unexpected content. remove_dir at publication repeats the
/// emptiness check atomically, protecting against files added by other programs.
fn require_empty_directory(path: &Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Error::FileRead(error.to_string())),
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
            match fs::read_dir(path)
                .map_err(|error| Error::FileRead(error.to_string()))?
                .next()
            {
                None => Ok(()),
                Some(Err(error)) => Err(Error::FileRead(error.to_string())),
                Some(Ok(_)) => Err(Error::CompassProject(
                    "Local project files already exist. Review or save them before importing."
                        .into(),
                )),
            }
        }
        Ok(_) => Err(Error::CompassProject(
            "The project working directory is not an empty directory.".into(),
        )),
    }
}

struct ImportStage {
    path: PathBuf,
}

impl ImportStage {
    fn new(parent: &Path) -> Result<Self, Error> {
        fs::create_dir_all(parent).map_err(|error| Error::FileWrite(error.to_string()))?;
        let path = parent.join(format!(".import-{}", Uuid::new_v4()));
        fs::create_dir(&path).map_err(|error| Error::FileWrite(error.to_string()))?;
        Ok(Self { path })
    }

    fn publish(self, destination: &Path) -> Result<(), Error> {
        require_empty_directory(destination)?;
        let had_empty_directory = destination.exists();
        if had_empty_directory {
            fs::remove_dir(destination).map_err(|error| Error::FileWrite(error.to_string()))?;
        }
        if let Err(error) = fs::rename(&self.path, destination) {
            if had_empty_directory {
                let _ = fs::create_dir(destination);
            }
            return Err(Error::FileWrite(error.to_string()));
        }
        Ok(())
    }
}

impl Drop for ImportStage {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct InitialImportGuard<'a> {
    state: &'a AppState,
    app_handle: AppHandle,
}

impl<'a> InitialImportGuard<'a> {
    fn new(state: &'a AppState, app_handle: AppHandle) -> Self {
        state.initial_import_running.store(true, Ordering::SeqCst);
        set_sign_out_enabled(&app_handle, false);
        Self { state, app_handle }
    }
}

impl Drop for InitialImportGuard<'_> {
    fn drop(&mut self) {
        self.state
            .initial_import_running
            .store(false, Ordering::SeqCst);
        set_sign_out_enabled(&self.app_handle, true);
    }
}

fn set_sign_out_enabled(app_handle: &AppHandle, enabled: bool) {
    if let Some(menu) = app_handle.menu()
        && let Some(item) = menu.items().ok().and_then(|items| {
            items.into_iter().find_map(|item| {
                item.as_submenu()
                    .and_then(|submenu| submenu.get(SIGN_OUT_MENU_ID))
            })
        })
        && let Some(item) = item.as_menuitem()
        && let Err(error) = item.set_enabled(enabled)
    {
        log::warn!("Could not update Sign Out menu during import: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::api_types::{ActiveMutex, CommitInfo, ProjectType};

    struct ImportFiles {
        project_id: Uuid,
        source: PathBuf,
    }

    impl ImportFiles {
        fn new(project_id: Uuid) -> Self {
            let source =
                std::env::temp_dir().join(format!("compass-import-lifecycle-{project_id}"));
            fs::create_dir(&source).unwrap();
            let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/test_data");
            for (fixture, destination) in [
                ("Fulfords.mak", "Fulfords.mak"),
                ("Fulford.dat", "FULFORD.DAT"),
                ("Fulsurf.dat", "FULSURF.DAT"),
            ] {
                fs::copy(fixtures.join(fixture), source.join(destination)).unwrap();
            }
            Self { project_id, source }
        }

        fn independent_sections(project_id: Uuid, count: usize) -> Self {
            let source =
                std::env::temp_dir().join(format!("compass-import-lifecycle-{project_id}"));
            fs::create_dir(&source).unwrap();
            let mut mak = String::from(
                "@357715.717,4372837.574,3048.000,13,-1.050;\n&North American 1983;\n",
            );
            for index in 0..count {
                let name = format!("S{index:02}");
                mak.push_str(&format!("#{name}.DAT;\n"));
                let dat = format!(
                    "Import Lifecycle Cave\nSURVEY NAME: {name}\nSURVEY DATE: 9 22 2026\nSURVEY TEAM:\nSidecar Tests\nDECLINATION: 0.00 FORMAT: DDDDUDLRLADN CORRECTIONS: 0.00 0.00 0.00\n\nFROM TO LENGTH BEARING INC LEFT UP DOWN RIGHT\n\n{name}A {name}B 1.00 0.00 0.00 1.00 1.00 1.00 1.00\n\x0c\n"
                );
                fs::write(source.join(format!("{name}.DAT")), dat).unwrap();
            }
            fs::write(source.join("Selection.mak"), mak).unwrap();
            Self { project_id, source }
        }

        fn analyze(&self) -> AnalyzedImport {
            import::analyze(&self.source.join("Fulfords.mak")).unwrap()
        }

        fn source_contents(&self) -> Vec<Vec<u8>> {
            ["Fulfords.mak", "FULFORD.DAT", "FULSURF.DAT"]
                .into_iter()
                .map(|name| fs::read(self.source.join(name)).unwrap())
                .collect()
        }
    }

    impl Drop for ImportFiles {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.source);
            let _ = fs::remove_dir_all(compass_project_path(self.project_id));
            let _ = fs::remove_file(
                std::env::temp_dir().join(format!("project_{}.zip", self.project_id)),
            );
        }
    }

    fn fixture() -> (ProjectInfo, ApiInfo) {
        let api = ApiInfo::new(
            "https://example.invalid".parse().unwrap(),
            Some("test@example.invalid".into()),
            Some("test-token".into()),
        );
        let info = ProjectInfo {
            id: Uuid::new_v4(),
            name: "Test".into(),
            description: String::new(),
            is_active: true,
            permission: "ADMIN".into(),
            active_mutex: Some(ActiveMutex {
                user: "test@example.invalid".into(),
                creation_date: String::new(),
                modified_date: String::new(),
            }),
            country: "US".into(),
            created_by: String::new(),
            creation_date: String::new(),
            modified_date: String::new(),
            latitude: None,
            longitude: None,
            fork_from: None,
            visibility: "PRIVATE".into(),
            exclude_geojson: false,
            latest_commit: None,
            project_type: ProjectType::Compass,
        };
        (info, api)
    }

    #[test]
    fn import_requires_empty_editable_project_owned_lock_and_closed_compass() {
        let (mut info, api) = fixture();
        assert!(validate_initial_target(&info, &api, false).is_ok());
        assert!(validate_initial_target(&info, &api, true).is_err());
        info.permission = "READ_ONLY".into();
        assert!(validate_initial_target(&info, &api, false).is_err());
        info.permission = "READ_AND_WRITE".into();
        info.active_mutex.as_mut().unwrap().user = "other@example.invalid".into();
        assert!(matches!(
            validate_initial_target(&info, &api, false),
            Err(Error::ProjectMutexLocked(_))
        ));
        info.active_mutex.as_mut().unwrap().user = "test@example.invalid".into();
        info.latest_commit = Some(CommitInfo {
            id: "commit".into(),
            message: "[Automated] Project Creation".into(),
            author_name: String::new(),
            commit_date: None,
            dt_since: String::new(),
            tree: vec![],
        });
        assert!(validate_initial_target(&info, &api, false).is_ok());
        info.latest_commit.as_mut().unwrap().message = "Imported project".into();
        assert!(validate_initial_target(&info, &api, false).is_err());
    }

    #[test]
    fn failed_staging_and_nonempty_destination_preserve_existing_files() {
        let parent = std::env::temp_dir().join(format!("compass-import-stage-{}", Uuid::new_v4()));
        let destination = parent.join("working_copy");
        fs::create_dir_all(&destination).unwrap();
        fs::write(destination.join("keep.dat"), b"original").unwrap();
        let stage = ImportStage::new(&parent).unwrap();
        let stage_path = stage.path.clone();
        fs::write(stage.path.join("new.dat"), b"new").unwrap();
        assert!(stage.publish(&destination).is_err());
        assert_eq!(fs::read(destination.join("keep.dat")).unwrap(), b"original");
        assert!(!stage_path.exists());
        assert!(!destination.join("new.dat").exists());
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn publishes_complete_stage_to_empty_directory() {
        let parent =
            std::env::temp_dir().join(format!("compass-import-publish-{}", Uuid::new_v4()));
        let destination = parent.join("working_copy");
        fs::create_dir_all(&destination).unwrap();
        let stage = ImportStage::new(&parent).unwrap();
        fs::write(stage.path.join("complete.dat"), b"complete").unwrap();
        stage.publish(&destination).unwrap();
        assert_eq!(
            fs::read(destination.join("complete.dat")).unwrap(),
            b"complete"
        );
        assert_eq!(fs::read_dir(&parent).unwrap().count(), 1);
        fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn gate_prevents_overlapping_mutations_and_releases_after_failure() {
        let state = AppState::new();
        let operation = state.try_project_operation().unwrap();
        assert!(state.try_project_operation().is_err());
        assert!(state.project_operations.try_lock().is_err());
        drop(operation);
        assert!(state.try_project_operation().is_ok());
    }

    #[tokio::test]
    async fn preview_waits_for_background_work_then_revalidates_the_target() {
        let state = AppState::new();
        let operation = state.project_operations.lock().await;
        let request = state.preview_initial_import(Uuid::new_v4(), PathBuf::from("unused.mak"));
        tokio::pin!(request);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(20), &mut request)
                .await
                .is_err(),
            "a refresh must delay preview instead of returning a spurious busy error"
        );
        drop(operation);
        assert!(matches!(
            request.await,
            Err(Error::CompassProject(message)) if message.contains("selected project changed")
        ));
    }

    #[test]
    fn authentication_changes_invalidate_session_identity() {
        let (_, api) = fixture();
        assert!(same_session(&api, &api.clone()));
        assert!(!same_session(&api, &ApiInfo::default()));
        let changed_token = ApiInfo::new(
            api.instance().clone(),
            api.email().map(str::to_owned),
            Some("another-token".into()),
        );
        assert!(!same_session(&api, &changed_token));
    }

    #[test]
    fn post_publication_errors_never_offer_source_import_retry() {
        let error = Error::NetworkRequest("offline".into());
        assert_eq!(
            ProjectSaveFailure::Upload(error.clone()).into_import_outcome(),
            InitialImportOutcome::LocalOnly {
                error: error.clone()
            }
        );
        assert_eq!(
            ProjectSaveFailure::Synchronization(error.clone()).into_import_outcome(),
            InitialImportOutcome::UploadedNeedsRefresh { error }
        );
    }

    #[test]
    fn changed_source_aborts_staging_without_publishing_partial_files() {
        let files = ImportFiles::new(Uuid::new_v4());
        let analysis = files.analyze();
        fs::write(files.source.join("FULFORD.DAT"), b"changed after preview").unwrap();
        let project_root = compass_project_path(files.project_id);
        {
            let stage = ImportStage::new(&project_root).unwrap();
            assert!(matches!(
                analysis.stage(files.project_id, &[0, 1], &stage.path),
                Err(Error::ImportSourceChanged(_))
            ));
        }
        assert!(!compass_project_working_path(files.project_id).exists());
        assert_eq!(fs::read_dir(project_root).unwrap().count(), 0);
        assert_eq!(
            fs::read(files.source.join("FULFORD.DAT")).unwrap(),
            b"changed after preview"
        );
    }

    #[test]
    fn cancelling_stale_preview_preserves_newer_preview_and_matching_cancel_invalidates_it() {
        let (info, api_info) = fixture();
        let files = ImportFiles::new(info.id);
        let analysis = Arc::new(files.analyze());
        let state = AppState::new();
        let old_id = Uuid::new_v4();
        let new_id = Uuid::new_v4();
        {
            let mut store = state.import_previews.lock().unwrap();
            store.preview = Some(StoredPreview {
                id: old_id,
                project_id: info.id,
                api_info: api_info.clone(),
                analysis: Arc::clone(&analysis),
            });
        }
        state.invalidate_import_previews();
        let generation = {
            let mut store = state.import_previews.lock().unwrap();
            assert!(store.preview.is_none());
            store.preview = Some(StoredPreview {
                id: new_id,
                project_id: info.id,
                api_info,
                analysis,
            });
            store.generation
        };
        state.cancel_import_preview(old_id);
        {
            let store = state.import_previews.lock().unwrap();
            assert_eq!(store.preview.as_ref().unwrap().id, new_id);
            assert_eq!(store.generation, generation);
        }
        state.cancel_import_preview(new_id);
        let store = state.import_previews.lock().unwrap();
        assert!(store.preview.is_none());
        assert_ne!(store.generation, generation);
    }

    #[tokio::test]
    async fn refused_upload_keeps_complete_dirty_import_and_saves_bound_project() {
        let (info, _) = fixture();
        let files = ImportFiles::new(info.id);
        let original_sources = files.source_contents();
        let analysis = files.analyze();
        let stage = ImportStage::new(&compass_project_path(info.id)).unwrap();
        analysis.stage(info.id, &[0, 1], &stage.path).unwrap();
        stage
            .publish(&compass_project_working_path(info.id))
            .unwrap();

        let state = AppState::new();
        state.set_project_info(info.clone());
        let mut other = info.clone();
        other.id = Uuid::new_v4();
        state.set_project_info(other.clone());
        let _operation = state.try_project_operation().unwrap();
        // No token in AppState means this selection never sends a lock request.
        // Deliberately select another project to verify the save helper's binding.
        state.set_active_project(Some(other.id)).await.unwrap();
        let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let api_info = ApiInfo::new(
            format!("http://127.0.0.1:{port}").parse().unwrap(),
            Some("test@example.invalid".into()),
            Some("offline-lifecycle-test-token".into()),
        );
        // This makes a real request to an unbound local port; no HTTP mock or
        // remote SpeleoDB project is involved.
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            state.save_project_by_id(info.id, &api_info, "Imported local project".into()),
        )
        .await
        .expect("a refused local connection should fail promptly");
        let failure = match result {
            Err(failure) => failure,
            Ok(_) => panic!("an unbound local port cannot accept an upload"),
        };
        assert!(matches!(
            failure.into_import_outcome(),
            InitialImportOutcome::LocalOnly {
                error: Error::NetworkRequest(_)
            }
        ));
        let working_copy = compass_project_working_path(info.id);
        for (name, original) in ["Fulfords.mak", "FULFORD.DAT", "FULSURF.DAT"]
            .into_iter()
            .zip(&original_sources)
        {
            assert_eq!(&fs::read(working_copy.join(name)).unwrap(), original);
        }
        assert_eq!(files.source_contents(), original_sources);
        assert!(!crate::paths::compass_project_index_path(info.id).exists());
        assert!(!compass_project_path(info.id).join(".revision.txt").exists());
        assert!(!compass_project_path(other.id).exists());
        assert_eq!(
            ProjectManager::initialize_from_info(info)
                .project_status()
                .local_status(),
            common::ui_state::LocalProjectStatus::Dirty
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn selective_initial_import_real_api_round_trip_and_no_changes_retry() {
        use std::collections::BTreeSet;
        use std::io::Read;

        let Ok(instance) = std::env::var("TEST_SPELEODB_INSTANCE") else {
            eprintln!("Skipping real import lifecycle: test instance is not configured");
            return;
        };
        let Ok(token) = std::env::var("TEST_SPELEODB_OAUTH") else {
            eprintln!("Skipping real import lifecycle: test token is not configured");
            return;
        };
        if instance.trim().is_empty() || token.trim().is_empty() {
            eprintln!("Skipping real import lifecycle: test credentials are empty");
            return;
        }
        let api_info = api::auth::authorize_with_token(instance.parse().unwrap(), &token)
            .await
            .expect("test API authentication must succeed");
        let suffix = Uuid::new_v4().simple().to_string()[..8].to_string();
        let created = api::project::create_project(
            &api_info,
            format!("sidecar-ci-selection-{suffix}"),
            "Dedicated selective initial-import lifecycle fixture".into(),
            "US".into(),
            None,
            None,
        )
        .await
        .expect("dedicated test project must be created");
        let project_id = created.id;
        let locked = api::project::acquire_project_mutex(&api_info, project_id)
            .await
            .expect("dedicated test project mutex must be acquired");
        let lifecycle_api = api_info.clone();

        // A spawned task captures assertion panics so the remote mutex is always
        // released before propagating a failed lifecycle assertion.
        let lifecycle = tokio::spawn(async move {
            let files = ImportFiles::independent_sections(project_id, 20);
            let source_mak = fs::read(files.source.join("Selection.mak")).unwrap();
            let analysis = import::analyze(&files.source.join("Selection.mak")).unwrap();
            let preview = analysis.preview(Uuid::new_v4());
            assert_eq!(preview.sections.len(), 20);
            assert!(preview.full_import_reason.is_none());
            let selected = [1_usize, 5, 10, 19];
            let expected_dats = selected.map(|id| format!("S{id:02}.DAT"));
            let stage = ImportStage::new(&compass_project_path(project_id)).unwrap();
            analysis.stage(project_id, &selected, &stage.path).unwrap();
            let working = compass_project_working_path(project_id);
            stage.publish(&working).unwrap();

            let state = AppState::new();
            state.set_project_info(locked);
            let _operation = state.try_project_operation().unwrap();
            let result = state
                .save_project_by_id(
                    project_id,
                    &lifecycle_api,
                    "Selected four of twenty sections".into(),
                )
                .await
                .unwrap_or_else(|error| panic!("initial upload failed: {}", error.into_error()));
            assert_eq!(result, common::api_types::ProjectSaveResult::Saved);

            let downloaded = api::project::download_project_zip(&lifecycle_api, project_id)
                .await
                .expect("uploaded selection must be downloadable");
            let mut archive = zip::ZipArchive::new(std::io::Cursor::new(downloaded)).unwrap();
            let actual_names: BTreeSet<String> = archive.file_names().map(str::to_owned).collect();
            let expected_names: BTreeSet<String> = expected_dats
                .iter()
                .cloned()
                .chain(["Selection.mak".into(), "compass.toml".into()])
                .collect();
            assert_eq!(actual_names, expected_names);
            for name in &actual_names {
                let mut bytes = Vec::new();
                archive
                    .by_name(name)
                    .unwrap()
                    .read_to_end(&mut bytes)
                    .unwrap();
                assert_eq!(
                    bytes,
                    fs::read(working.join(name)).unwrap(),
                    "remote file differs: {name}"
                );
            }
            let mak = fs::read_to_string(working.join("Selection.mak")).unwrap();
            let records: Vec<_> = mak
                .lines()
                .filter_map(|line| line.strip_prefix('#'))
                .map(|line| line.trim_end_matches(';'))
                .collect();
            assert_eq!(
                records,
                expected_dats.iter().map(String::as_str).collect::<Vec<_>>()
            );
            let metadata: toml::Value =
                toml::from_str(&fs::read_to_string(working.join("compass.toml")).unwrap()).unwrap();
            let tracked: Vec<_> = metadata["project"]["dat_files"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap())
                .collect();
            assert_eq!(tracked, records);
            assert_eq!(
                fs::read(files.source.join("Selection.mak")).unwrap(),
                source_mak
            );
            assert_eq!(fs::read_dir(&files.source).unwrap().count(), 21);
            for name in &expected_dats {
                assert_eq!(
                    fs::read(files.source.join(name)).unwrap(),
                    fs::read(working.join(name)).unwrap()
                );
            }

            let retry = state
                .save_project_by_id(
                    project_id,
                    &lifecycle_api,
                    "Retry identical selected import".into(),
                )
                .await
                .unwrap_or_else(|error| panic!("unchanged retry failed: {}", error.into_error()));
            assert_eq!(retry, common::api_types::ProjectSaveResult::NoChanges);
            assert_eq!(
                ProjectManager::initialize_from_info(state.get_project_info(project_id).unwrap())
                    .project_status()
                    .local_status(),
                common::ui_state::LocalProjectStatus::UpToDate
            );
        })
        .await;
        let release = api::project::release_project_mutex(&api_info, project_id).await;
        match lifecycle {
            Ok(()) => {
                release.expect("test project mutex must be released");
            }
            Err(error) if error.is_panic() => {
                if release.is_err() {
                    eprintln!("Releasing the test project mutex also failed");
                }
                std::panic::resume_unwind(error.into_panic());
            }
            Err(error) => panic!("selective import lifecycle task failed: {error}"),
        }
    }
}
