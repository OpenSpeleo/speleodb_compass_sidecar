use crate::{project_management::ProjectManager, user_prefs::UserPrefs};
use chrono::{DateTime, Utc};
use common::{
    ApiInfo, Error,
    api_types::ProjectInfo,
    ui_state::{
        LoadingState, LocalProjectStatus, ProjectSaveResult, ProjectStatus,
        UI_STATE_NOTIFICATION_KEY, UiState,
    },
};
use log::{debug, error, info, trace, warn};
use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{
    AppHandle, Emitter, Manager,
    async_runtime::JoinHandle,
    menu::{MenuBuilder, SubmenuBuilder},
};
use tauri_plugin_updater::{Update, UpdaterExt};
use uuid::Uuid;

const PROJECT_INFO_UPDATE_INTERVAL: Duration = Duration::from_secs(120); //  update the list of projects status every 2 minutes
const LOCAL_STATUS_CHECK_INTERVAL: Duration = Duration::from_secs(1); // check local project status and compass state every second

pub struct AppState {
    app_handle: Mutex<Option<AppHandle>>,
    initializing: Mutex<bool>,
    loading_state: Mutex<LoadingState>,
    api_info: Mutex<ApiInfo>,
    project_info: Mutex<HashMap<uuid::Uuid, ProjectInfo>>,
    active_project: Mutex<Option<uuid::Uuid>>,
    compass_pid: Mutex<Option<u32>>,
    background_task_handle: Mutex<Option<JoinHandle<()>>>,
    last_project_update: Mutex<DateTime<Utc>>,
    last_status_check: Mutex<DateTime<Utc>>,
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
            compass_pid: Mutex::new(None),
            background_task_handle: Mutex::new(None),
            last_project_update: Mutex::new(chrono::Utc::now()),
            last_status_check: Mutex::new(chrono::Utc::now()),
        }
    }

    /// Asynchronously initialize the application state.
    pub async fn init_app_state(&self, app_handle: &AppHandle) {
        info!("Initializing app state");
        if self.app_handle.lock().unwrap().is_none() {
            info!("Storing app handle in app state");
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
                    log::warn!("Previous initialization failed with error: {}.", e);
                    self.set_initializing(false);
                    break;
                }
                LoadingState::Unauthenticated | LoadingState::Ready => {
                    log::info!("App state already initialized",);
                    self.emit_app_state_change();
                    self.set_initializing(false);
                    #[cfg(not(debug_assertions))]
                    if self.background_task_handle.lock().unwrap().is_none() {
                        let app_handle = app_handle.clone();
                        let join_handle = tauri::async_runtime::spawn(async move {
                            AppState::background_update_task(&app_handle).await;
                        });
                        *self.background_task_handle.lock().unwrap() = Some(join_handle);
                    }
                    break;
                }
                _ => loading_state = self.init_internal(&app_handle).await,
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
        let app_handle = self.app_handle()?;
        info!("Updating user preferences");
        prefs.save()?;
        self.set_api_info(prefs.api_info().clone());
        let app_handle = app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let menu = if prefs.api_info().oauth_token().is_none() {
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
        });
        self.emit_app_state_change();

        Ok(())
    }

    pub async fn authenticated(&self) -> () {
        if let Ok(app_handle) = self.app_handle() {
            self.set_loading_state(LoadingState::LoadingProjects);
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
        self.set_loading_state(LoadingState::NotStarted);
        tauri::async_runtime::spawn({
            let app_handle = app_handle.clone();
            async move {
                let app_state = app_handle.state::<AppState>();
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
        project.make_local(&api_info).await?;
        let project_status = project.update_project().await?;
        if let LocalProjectStatus::OutOfDate = project_status.local_status() {
            project.update_local_copies(&api_info).await?;
        }
        Ok(project_status)
    }

    pub async fn set_active_project(
        &self,
        project_id: Option<Uuid>,
        app_handle: &AppHandle,
    ) -> Result<(), Error> {
        if let Some(project_id) = project_id {
            info!("Selecting: {project_id} as active project");
            let project_info =
                api::project::acquire_project_mutex(&self.api_info(), project_id).await?;
            *self.active_project.lock().unwrap() = Some(project_id);
            self.emit_app_state_change();
            self.update_local_project(project_info).await?;
            self.emit_app_state_change();
        } else {
            if let Some(active_project) = self.get_active_project_status() {
                *self.active_project.lock().unwrap() = None;
                self.set_loading_state(LoadingState::LoadingProjects);
                self.emit_app_state_change();
                if let LocalProjectStatus::Dirty = active_project.local_status() {
                    warn!("Refusing to release project mutex for dirty project");
                } else {
                    info!("Releasing mutex for clean active project");
                    let project_info =
                        api::project::release_project_mutex(&self.api_info(), active_project.id())
                            .await?;
                    self.update_local_project(project_info).await?;
                }
                self.init_internal(app_handle).await;
            }
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

    pub fn compass_is_open(&self) -> bool {
        self.compass_pid.lock().unwrap().is_some()
    }

    #[cfg(target_os = "windows")]
    pub fn set_compass_pid(&self, pid: Option<u32>) {
        *self.compass_pid.lock().unwrap() = pid;
        self.emit_app_state_change();
    }

    pub fn emit_app_state_change(&self) {
        let loading_state = self.loading_state();
        let project_info = self
            .project_info
            .lock()
            .unwrap()
            .values()
            .map(|p| ProjectManager::initialize_from_info(p.clone()).project_status())
            .collect();
        let user_email = self.api_info().email().map(|s| s.to_string());
        let active_project_id = self.get_active_project_id();
        let compass_is_open = self.compass_is_open();
        let ui_state = UiState::new(
            loading_state,
            user_email,
            project_info,
            active_project_id,
            compass_is_open,
        );
        if let Ok(app_handle) = self.app_handle() {
            app_handle
                .emit(UI_STATE_NOTIFICATION_KEY, &ui_state)
                .unwrap();
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

    /// Internal function used to update loading state and emit state change event.
    fn set_loading_state(&self, state: LoadingState) -> LoadingState {
        *self.loading_state.lock().unwrap() = state.clone();
        self.emit_app_state_change();
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
        match api::auth::authorize_with_token(api_info.instance().clone(), &token).await {
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

    async fn init_internal(&self, app_handle: &AppHandle) -> LoadingState {
        let loading_state = self.loading_state();
        let sec_delay = 0;
        match loading_state {
            LoadingState::NotStarted => self.set_loading_state(LoadingState::CheckingForUpdates),
            LoadingState::CheckingForUpdates => match self.check_for_update().await {
                Ok(update) => {
                    if let Some(update) = update {
                        log::info!("Update available, upating...");
                        tokio::time::sleep(std::time::Duration::from_secs(sec_delay)).await;
                        let loading_state = self.set_loading_state(LoadingState::Updating);
                        self.update_app(update).await.unwrap();
                        loading_state
                    } else {
                        log::info!("No updates available, attempting to load user preferences");
                        tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                        self.set_loading_state(LoadingState::LoadingPrefs)
                    }
                }
                Err(e) => {
                    log::warn!("Failed to check for updates: {}", e);
                    tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::Failed(e))
                }
            },
            LoadingState::LoadingPrefs => {
                let prefs = self.load_user_preferences();
                if let Some(_token) = prefs.api_info().oauth_token() {
                    info!("User prefs found, attempting to authenticate user");
                    tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::Authenticating)
                } else {
                    info!("No user prefs found, starting unauthenticated");
                    tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                    self.set_loading_state(LoadingState::Unauthenticated)
                }
            }
            LoadingState::Authenticating => {
                tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                match self.authenticate_user().await {
                    Ok(_) => self.set_loading_state(LoadingState::LoadingProjects),
                    Err(e) => {
                        // TODO:: Handle different authentication errors appropriately
                        log::warn!("Authentication failed: {}", e);
                        self.set_loading_state(LoadingState::Unauthenticated)
                    }
                }
            }
            LoadingState::LoadingProjects => {
                tokio::time::sleep(Duration::from_secs(sec_delay)).await;
                match self.load_user_projects().await {
                    Ok(_) => {
                        log::info!("User projects loaded successfully, app is ready");
                        self.set_loading_state(LoadingState::Ready)
                    }
                    Err(e) => {
                        log::warn!("Failed to load user projects: {}", e);
                        self.set_loading_state(LoadingState::Failed(e))
                    }
                }
            }
            _ => {
                log::info!("App already initialized or in progress");
                loading_state
            }
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
                        app_state.emit_app_state_change();
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
                app_state.emit_app_state_change();
                *app_state.last_status_check.lock().unwrap() = chrono::Utc::now();
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}
