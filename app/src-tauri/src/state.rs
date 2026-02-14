use crate::{project_management::ProjectManager, user_prefs::UserPrefs};
use chrono::{DateTime, Utc};
use common::{
    ApiInfo, Error,
    api_types::ProjectInfo,
    ui_state::{LoadingState, LocalProjectStatus, ProjectSaveResult, ProjectStatus, UiState},
};
use log::{debug, error, info, trace, warn};
use std::{
    collections::HashMap,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use tauri::{
    AppHandle, Emitter, Manager,
    async_runtime::JoinHandle,
    menu::{MenuBuilder, SubmenuBuilder},
};
use tauri_plugin_updater::{Update, UpdaterExt};
use uuid::Uuid;

const PROJECT_INFO_UPDATE_INTERVAL: Duration = Duration::from_secs(120); //  update the list of projects status every 2 minutes
const LOCAL_STATUS_CHECK_INTERVAL: Duration = Duration::from_secs(1); // check local project status and compass state every second

/// Event key for UI state notifications
pub const UI_STATE_EVENT: &str = "ui-state-update";

pub struct AppState {
    app_handle: Mutex<Option<AppHandle>>,
    initializing: Mutex<bool>,
    loading_state: Mutex<LoadingState>,
    api_info: Mutex<ApiInfo>,
    project_info: Mutex<HashMap<uuid::Uuid, ProjectInfo>>,
    active_project: Mutex<Option<uuid::Uuid>>,
    project_downloading: Mutex<bool>,
    compass_pid: Mutex<Option<u32>>,
    background_task_handle: Mutex<Option<JoinHandle<()>>>,
    last_project_update: Mutex<DateTime<Utc>>,
    last_status_check: Mutex<DateTime<Utc>>,
    last_emitted_ui_state: Mutex<UiState>,
    emit_mutex: tokio::sync::Mutex<()>,
    /// Flag indicating the WebView frontend is ready to receive events.
    /// Set to `true` when the first `ensure_initialized` IPC call arrives from the frontend.
    /// Before this flag is set, `emit_str` calls are skipped to avoid crashing WebView2.
    webview_ready: AtomicBool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            app_handle: Mutex::new(None),
            initializing: Mutex::new(false),
            loading_state: Mutex::new(LoadingState::NotStarted),
            api_info: Mutex::new(ApiInfo::default()),
            project_info: Mutex::new(HashMap::new()),
            active_project: Mutex::new(None),
            project_downloading: Mutex::new(false),
            compass_pid: Mutex::new(None),
            background_task_handle: Mutex::new(None),
            last_project_update: Mutex::new(chrono::Utc::now()),
            last_status_check: Mutex::new(chrono::Utc::now()),
            last_emitted_ui_state: Mutex::new(UiState::default()),
            emit_mutex: tokio::sync::Mutex::new(()),
            webview_ready: AtomicBool::new(false),
        }
    }

    pub fn reset_ui_state(&self) {
        *self.last_emitted_ui_state.lock().unwrap() = UiState::default();
    }

    /// Mark the WebView frontend as ready to receive events.
    /// Called when the first `ensure_initialized` IPC arrives from the WASM frontend.
    pub fn mark_webview_ready(&self) {
        self.webview_ready.swap(true, Ordering::SeqCst);
    }

    /// Check whether the WebView frontend is ready to receive events.
    pub fn is_webview_ready(&self) -> bool {
        self.webview_ready.load(Ordering::SeqCst)
    }

    /// Asynchronously initialize the application state.
    pub async fn init_app_state(&self, app_handle: &AppHandle) {
        if self.app_handle.lock().unwrap().is_none() {
            *self.app_handle.lock().unwrap() = Some(app_handle.clone());
        }
        if self.initializing() {
            return;
        }
        self.set_initializing(true);
        let mut loading_state = self.loading_state();
        loop {
            match &loading_state {
                LoadingState::Failed(e) => {
                    warn!("Previous initialization failed with error: {}", e);
                    self.set_initializing(false);
                    break;
                }
                LoadingState::Unauthenticated | LoadingState::Ready => {
                    self.apply_menu_for_auth_state();
                    self.emit_app_state_change().await;
                    self.set_initializing(false);
                    if self.background_task_handle.lock().unwrap().is_none() {
                        let app_handle = app_handle.clone();
                        let join_handle = tauri::async_runtime::spawn(async move {
                            AppState::background_update_task(&app_handle).await;
                        });
                        *self.background_task_handle.lock().unwrap() = Some(join_handle);
                    }
                    break;
                }
                _ => {
                    loading_state = self.init_internal().await;
                }
            }
        }
    }

    fn app_handle(&self) -> Result<AppHandle, Error> {
        self.app_handle
            .lock()
            .unwrap()
            .clone()
            .ok_or(Error::NoAppHandle)
    }

    pub fn api_info(&self) -> ApiInfo {
        self.api_info.lock().unwrap().clone()
    }

    fn set_api_info(&self, api_info: ApiInfo) {
        *self.api_info.lock().unwrap() = api_info;
    }

    pub fn update_user_prefs(&self, prefs: UserPrefs) -> Result<(), Error> {
        let _ = self.app_handle()?;
        prefs.save()?;
        self.set_api_info(prefs.api_info().clone());

        // Menu update is deferred to `apply_menu_for_auth_state()`.
        // Do NOT spawn set_menu or emit_app_state_change here.

        Ok(())
    }

    /// Apply the correct menu bar based on the current authentication state.
    /// Called once after initialization reaches a terminal state (Ready / Unauthenticated)
    /// to avoid racing with WebView2 event callbacks during the rapid init sequence.
    fn apply_menu_for_auth_state(&self) {
        let Ok(app_handle) = self.app_handle() else {
            return;
        };
        let has_token = self.api_info().oauth_token().is_some();
        let menu = if !has_token {
            MenuBuilder::new(&app_handle).build().unwrap()
        } else {
            let submenu = SubmenuBuilder::new(&app_handle, "Account")
                .submenu_native_icon(tauri::menu::NativeIcon::UserAccounts)
                .text("sign_out", "Sign Out")
                .build()
                .unwrap();
            MenuBuilder::new(&app_handle)
                .item(&submenu)
                .build()
                .unwrap()
        };
        app_handle.set_menu(menu).unwrap();
    }

    pub async fn authenticated(&self) -> () {
        if let Ok(app_handle) = self.app_handle() {
            self.set_loading_state(LoadingState::LoadingProjects).await;
            self.init_app_state(&app_handle).await;
        }
    }

    pub fn sign_out(&self, app_handle: &AppHandle) -> Result<(), Error> {
        UserPrefs::forget()?;
        {
            let mut project_lock = self.project_info.lock().unwrap();
            project_lock.clear();
        }
        {
            let user_prefs = ApiInfo::default();
            *self.api_info.lock().unwrap() = user_prefs;
        }
        self.set_loading_state_sync(LoadingState::NotStarted);
        tauri::async_runtime::spawn({
            let app_handle = app_handle.clone();
            async move {
                let app_state = app_handle.state::<AppState>();
                app_state.emit_app_state_change().await;
                app_state.init_app_state(&app_handle).await;
            }
        });
        Ok(())
    }

    pub async fn update_local_project(
        &self,
        project_info: ProjectInfo,
    ) -> Result<ProjectStatus, Error> {
        let api_info = self.api_info();
        self.set_project_info(project_info.clone());
        let mut project = ProjectManager::initialize_from_info(project_info);
        let project_status = project.update_project(&self.api_info()).await?;
        Ok(project_status)
    }

    pub async fn set_active_project(&self, project_id: Option<Uuid>) -> Result<(), Error> {
        if let Some(project_id) = project_id {
            info!("Selecting: {project_id} as active project");

            // Immediately switch to the project detail view and show downloading spinner
            *self.active_project.lock().unwrap() = Some(project_id);
            *self.project_downloading.lock().unwrap() = true;
            self.emit_app_state_change().await;

            // Now do the heavy work (mutex acquisition + download)
            let result = async {
                match api::project::acquire_project_mutex(&self.api_info(), project_id).await {
                    Ok(info) => {
                        info!("Project lock grabbed successfully");
                        let project = ProjectManager::initialize_from_info(info.clone());
                        project.make_local(&self.api_info()).await?;
                        self.update_local_project(info).await?;
                    }
                    Err(_e) => {
                        warn!(
                            "Failed to grab lock for project: {project_id}, opening as read-only"
                        );
                    }
                };
                Ok::<(), Error>(())
            }
            .await;

            // Clear downloading state regardless of success/failure
            *self.project_downloading.lock().unwrap() = false;
            self.emit_app_state_change().await;

            // Propagate any error from the download
            result?;
        } else if let Some(active_project) = self.get_active_project_status() {
            *self.active_project.lock().unwrap() = None;
            self.set_loading_state_sync(LoadingState::LoadingProjects);
            self.emit_app_state_change().await;
            if let LocalProjectStatus::Dirty = active_project.local_status() {
                warn!("Refusing to release project mutex for dirty project");
            } else {
                info!("Releasing mutex for clean active project");
                if let Some(active_mutex) = active_project.active_mutex()
                    && let Some(email) = self.api_info().email()
                    && active_mutex.user == email
                {
                    info!("Active mutex owned by current user, releasing");
                    let project_info =
                        api::project::release_project_mutex(&self.api_info(), active_project.id())
                            .await?;
                    self.update_local_project(project_info).await?;
                } else {
                    warn!("Active mutex not owned by current user, skipping release");
                }
            }
            self.init_internal().await;
        };
        Ok(())
    }

    pub fn get_active_project_id(&self) -> Option<uuid::Uuid> {
        *self.active_project.lock().unwrap()
    }

    pub fn get_active_project_status(&self) -> Option<ProjectStatus> {
        let active_project_id = self.get_active_project_id()?;
        let project_lock = self.project_info.lock().unwrap();
        let project_info = project_lock.get(&active_project_id)?;
        let project_manager = ProjectManager::initialize_from_info(project_info.clone());
        Some(project_manager.project_status())
    }

    pub async fn save_active_project(
        &self,
        commit_message: String,
    ) -> Result<ProjectSaveResult, Error> {
        let Some(project_id) = self.get_active_project_id() else {
            error!("No active project to save");
            return Err(Error::NoProjectSelected);
        };
        let project_info = self
            .get_project_info(project_id)
            .ok_or(Error::NoProjectSelected)?;
        let mut project_manager = ProjectManager::initialize_from_info(project_info);
        let api_info = self.api_info();
        let result = project_manager
            .save_local_changes(&api_info, commit_message)
            .await?;

        let updated_project_info = api::project::fetch_project_info(&api_info, project_id).await?;
        let project_manager = ProjectManager::initialize_from_info(updated_project_info);
        project_manager.update_local_copies(&api_info).await?;
        Ok(result)
    }

    pub async fn discard_active_project_changes(&self) -> Result<(), Error> {
        let Some(project_id) = self.get_active_project_id() else {
            error!("No active project to discard changes for");
            return Err(Error::NoProjectSelected);
        };
        let project_info = self
            .get_project_info(project_id)
            .ok_or(Error::NoProjectSelected)?;
        let project_manager = ProjectManager::initialize_from_info(project_info);
        let api_info = self.api_info();
        project_manager.update_local_copies(&api_info).await?;
        self.emit_app_state_change().await;
        Ok(())
    }

    pub fn compass_is_open(&self) -> bool {
        self.compass_pid.lock().unwrap().is_some()
    }

    #[cfg(target_os = "windows")]
    pub fn set_compass_pid(&self, pid: Option<u32>) {
        *self.compass_pid.lock().unwrap() = pid;
        // Spawn emit in a separate task since this function is sync
        if let Ok(app_handle) = self.app_handle() {
            tauri::async_runtime::spawn(async move {
                let app_state = app_handle.state::<AppState>();
                app_state.emit_app_state_change().await;
            });
        }
    }

    #[cfg(target_os = "windows")]
    fn get_compass_pid(&self) -> Option<u32> {
        *self.compass_pid.lock().unwrap()
    }

    /// Check if the Compass process is still running and update state if it has exited.
    #[cfg(target_os = "windows")]
    fn check_compass_process(&self) {
        use sysinfo::System;

        if let Some(pid) = self.get_compass_pid() {
            let s = System::new_all();
            let pid = sysinfo::Pid::from_u32(pid);
            if s.process(pid).is_none() {
                info!("Compass process (PID {}) has exited", pid);
                self.set_compass_pid(None);
            }
        }
    }

    pub async fn emit_app_state_change(&self) {
        let _emit_lock = self.emit_mutex.lock().await;
        let loading_state = self.loading_state();
        // Clone project info while holding lock briefly, then release before doing I/O
        let mut projects: Vec<ProjectInfo> = match self.project_info.lock() {
            Ok(guard) => guard.values().cloned().collect(),
            Err(e) => {
                error!("Failed to lock project_info: {}", e);
                return;
            }
        };
        // Sort by modified_date descending for consistent ordering
        projects.sort_by(|a, b| b.modified_date.cmp(&a.modified_date));
        // Compute project statuses without holding the lock (this does file I/O)
        let project_statuses: Vec<ProjectStatus> = projects
            .into_iter()
            .map(|p| ProjectManager::initialize_from_info(p).project_status())
            .collect();
        let user_email = self.api_info().email().map(|s| s.to_string());
        let active_project_id = self.get_active_project_id();
        let compass_is_open = self.compass_is_open();
        let project_downloading = *self.project_downloading.lock().unwrap();
        let ui_state = UiState::new(
            loading_state.clone(),
            user_email,
            project_statuses,
            active_project_id,
            compass_is_open,
            project_downloading,
        );
        // Only send if the state has actually changed
        {
            let mut last_state = match self.last_emitted_ui_state.lock() {
                Ok(guard) => guard,
                Err(e) => {
                    error!("Failed to lock last_emitted_ui_state: {}", e);
                    return;
                }
            };
            if *last_state == ui_state {
                return;
            }
            *last_state = ui_state.clone();
        }

        // ── WebView readiness gate ──
        // If the WebView hasn't signaled readiness (via ensure_initialized IPC),
        // we update internal state above but skip the actual emit_str call.
        // This prevents STATUS_FATAL_USER_CALLBACK_EXCEPTION (0xc000041d) on
        // Windows when WebView2 hasn't finished initializing its JS runtime.
        if !self.is_webview_ready() {
            return;
        }

        // Serialize and send to frontend
        let serialized = match serde_json::to_string(&ui_state) {
            Ok(s) => s,
            Err(e) => {
                error!("Failed to serialize UI state: {}", e);
                return;
            }
        };

        // Get app handle for emitting
        let app_handle = match self.app_handle() {
            Ok(handle) => handle,
            Err(e) => {
                error!("No app handle available for emit: {}", e);
                return;
            }
        };
        // Emit event to frontend
        if let Err(e) = app_handle.emit_str(UI_STATE_EVENT, serialized) {
            error!("Failed to emit UI state event: {}", e);
        }
    }

    fn initializing(&self) -> bool {
        *self.initializing.lock().unwrap()
    }

    fn set_initializing(&self, initializing: bool) {
        *self.initializing.lock().unwrap() = initializing;
    }

    fn loading_state(&self) -> LoadingState {
        self.loading_state.lock().unwrap().clone()
    }

    /// Internal sync function to update loading state without emitting (for use in async contexts).
    fn set_loading_state_sync(&self, state: LoadingState) -> LoadingState {
        *self.loading_state.lock().unwrap() = state.clone();
        state
    }

    /// Internal async function to update loading state and emit state change event.
    async fn set_loading_state(&self, state: LoadingState) -> LoadingState {
        *self.loading_state.lock().unwrap() = state.clone();
        self.emit_app_state_change().await;
        state
    }

    async fn check_for_update(&self) -> Result<Option<Update>, Error> {
        info!("Checking for app updates");
        let app_handle = self.app_handle()?;
        Ok(match app_handle.updater().unwrap().check().await {
            Ok(update) => update,
            Err(e) => {
                warn!("Failed to check for updates: {}", e);
                None
            }
        })
    }

    async fn update_app(&self, update: Update) -> Result<(), Error> {
        info!("Updating application");
        update
            .download_and_install(
                |chunk_len, content_len| {
                    if let Some(content_len) = content_len {
                        let progress = chunk_len as f64 / content_len as f64;
                        debug!("Download progress: {:.2}%", progress * 100.0);
                    }
                },
                || info!("Download finished"),
            )
            .await
            .unwrap();
        let app_handle = self.app_handle().unwrap();
        app_handle.restart()
    }

    fn load_user_preferences(&self) -> UserPrefs {
        info!("Loading user preferences");
        let user_prefs = UserPrefs::load().unwrap_or_default();
        self.update_user_prefs(user_prefs.clone())
            .unwrap_or_else(|e| {
                log::warn!("Failed to save loaded user preferences: {}", e);
            });
        user_prefs
    }

    async fn authenticate_user(&self) -> Result<(), String> {
        let api_info = self.api_info();
        info!("Authenticating user");
        let Some(token) = api_info.oauth_token() else {
            log::warn!("No OAuth token found in user preferences");
            return Err("No OAuth token found".to_string());
        };
        match api::auth::authorize_with_token(api_info.instance().clone(), token).await {
            Ok(api_info) => {
                log::info!("User authenticated successfully");
                let prefs = UserPrefs::new(api_info);
                if self.update_user_prefs(prefs).is_err() {
                    log::warn!("Failed to save user preferences after authentication");
                }
                Ok(())
            }
            Err(e) => {
                log::warn!("Failed to authenticate user with saved token: {}", e);
                Err(e)
            }
        }
    }

    async fn load_user_projects(&self) -> Result<Vec<ProjectInfo>, Error> {
        info!("Loading user projects");
        let api_info = self.api_info();
        match api::project::fetch_projects(&api_info).await {
            Ok(projects) => {
                self.clear_local_projects();
                for project in projects.clone() {
                    self.update_local_project(project).await?;
                }
                *self.last_project_update.lock().unwrap() = chrono::Utc::now();
                Ok(projects)
            }
            Err(e) => {
                warn!("Failed to load user projects: {}", e);
                Err(e)
            }
        }
    }

    /// Advance the loading state machine by one step.
    async fn init_internal(&self) -> LoadingState {
        let loading_state = self.loading_state();
        match loading_state {
            LoadingState::NotStarted => {
                self.set_loading_state(LoadingState::CheckingForUpdates)
                    .await
            }
            LoadingState::CheckingForUpdates => match self.check_for_update().await {
                Ok(update) => {
                    if let Some(update) = update {
                        let loading_state = self.set_loading_state(LoadingState::Updating).await;
                        self.update_app(update).await.unwrap();
                        loading_state
                    } else {
                        self.set_loading_state(LoadingState::LoadingPrefs).await
                    }
                }
                Err(e) => {
                    warn!("Failed to check for updates: {}", e);
                    self.set_loading_state(LoadingState::Failed(e)).await
                }
            },
            LoadingState::LoadingPrefs => {
                let prefs = self.load_user_preferences();
                if let Some(_token) = prefs.api_info().oauth_token() {
                    self.set_loading_state(LoadingState::Authenticating).await
                } else {
                    self.set_loading_state(LoadingState::Unauthenticated).await
                }
            }
            LoadingState::Authenticating => match self.authenticate_user().await {
                Ok(_) => self.set_loading_state(LoadingState::LoadingProjects).await,
                Err(_) => self.set_loading_state(LoadingState::Unauthenticated).await,
            },
            LoadingState::LoadingProjects => match self.load_user_projects().await {
                Ok(_) => self.set_loading_state(LoadingState::Ready).await,
                Err(e) => {
                    warn!("Failed to load user projects: {}", e);
                    self.set_loading_state(LoadingState::Failed(e)).await
                }
            },
            _ => loading_state,
        }
    }

    fn set_project_info(&self, project_info: ProjectInfo) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.insert(project_info.id, project_info);
    }

    fn get_project_info(&self, project_id: Uuid) -> Option<ProjectInfo> {
        let project_lock = self.project_info.lock().unwrap();
        project_lock.get(&project_id).cloned()
    }

    fn clear_local_projects(&self) {
        let mut project_lock = self.project_info.lock().unwrap();
        project_lock.clear();
    }

    async fn background_update_task(app_handle: &AppHandle) {
        let app_state = app_handle.state::<AppState>();
        loop {
            let last_projectupdate = *app_state.last_project_update.lock().unwrap();
            if chrono::Utc::now()
                .signed_duration_since(last_projectupdate)
                .to_std()
                .unwrap()
                >= PROJECT_INFO_UPDATE_INTERVAL
            {
                trace!("Background task: updating project info from API");
                match app_state.load_user_projects().await {
                    Ok(_) => {
                        app_state.emit_app_state_change().await;
                    }
                    Err(e) => {
                        error!("Background task: failed to update project info: {}", e);
                    }
                }
            }
            let last_local_status_check = *app_state.last_status_check.lock().unwrap();
            if chrono::Utc::now()
                .signed_duration_since(last_local_status_check)
                .to_std()
                .unwrap()
                >= LOCAL_STATUS_CHECK_INTERVAL
            {
                trace!("Background task: checking local project statuses");
                let project_info: Vec<ProjectInfo> = app_state
                    .project_info
                    .lock()
                    .unwrap()
                    .values()
                    .cloned()
                    .collect();
                for project in project_info {
                    let project_id = project.id;
                    if let Err(e) = app_state.update_local_project(project).await {
                        error!(
                            "Background task: failed to update local project {}: {}",
                            project_id, e
                        );
                    }
                }
                #[cfg(target_os = "windows")]
                app_state.check_compass_process();
                app_state.emit_app_state_change().await;
                *app_state.last_status_check.lock().unwrap() = chrono::Utc::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
