use crate::state::AppState;
use common::ui_state::{UpdateNotification, UpdateNotificationPhase};
use log::{debug, error, info};
use std::{
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

pub const APP_NAME: &str = "SpeleoDB Compass Sidecar";
/// The single source of truth for the repository URL that ships with the
/// app. `LATEST_RELEASE_URL` and the `about_info` command both derive from
/// this constant — keep them in lockstep to avoid showing the user two
/// inconsistent links.
pub const REPO_URL: &str = "https://github.com/OpenSpeleo/speleodb_compass_sidecar";
/// Public releases page used by the toast's "Download Latest" button.
/// Kept colocated with [`REPO_URL`] for visual review.
pub const LATEST_RELEASE_URL: &str =
    "https://github.com/OpenSpeleo/speleodb_compass_sidecar/releases/latest";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UpdateCheckOrigin {
    Startup,
    Manual,
}

#[derive(Default, Debug)]
pub(crate) struct DownloadProgress {
    downloaded: u64,
    last_percent: Option<u8>,
}

impl DownloadProgress {
    pub(crate) fn record_chunk(
        &mut self,
        chunk_len: usize,
        content_len: Option<u64>,
    ) -> Option<u8> {
        self.downloaded = self.downloaded.saturating_add(chunk_len as u64);
        let content_len = content_len?;
        if content_len == 0 {
            return None;
        }

        let percent = self
            .downloaded
            .saturating_mul(100)
            .checked_div(content_len)
            .unwrap_or(100)
            .min(100) as u8;

        if self.last_percent == Some(percent) {
            None
        } else {
            self.last_percent = Some(percent);
            Some(percent)
        }
    }
}

/// Decide whether a new download progress percent should overwrite the
/// currently-published one. Returns `true` only when the new percent strictly
/// advances the displayed progress, so out-of-order callbacks from the
/// updater download stream cannot regress the UI from e.g. 50% back to 30%.
pub(crate) fn should_publish_progress(current: Option<u8>, new_percent: u8) -> bool {
    match current {
        None => true,
        Some(existing) => new_percent > existing,
    }
}

pub(crate) fn no_update_notification(
    origin: UpdateCheckOrigin,
    id: u64,
) -> Option<UpdateNotification> {
    match origin {
        UpdateCheckOrigin::Startup => None,
        UpdateCheckOrigin::Manual => Some(UpdateNotification::new(
            id,
            UpdateNotificationPhase::UpToDate {
                app_name: APP_NAME.to_string(),
            },
        )),
    }
}

pub(crate) fn humanize_update_error(error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        "unknown updater error".to_string()
    } else {
        trimmed.to_string()
    }
}

/// RAII guard that resets the workflow-running flag whether the workflow
/// finishes normally, returns early via `?`, or unwinds. Without this guard a
/// panic in any awaited call would leave the flag stuck and silently drop
/// every future check (including manual retries).
struct WorkflowGuard<'a> {
    flag: &'a AtomicBool,
}

impl<'a> WorkflowGuard<'a> {
    fn new(flag: &'a AtomicBool) -> Self {
        Self { flag }
    }
}

impl<'a> Drop for WorkflowGuard<'a> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

impl AppState {
    pub fn start_startup_update_check(app_handle: &AppHandle) {
        let app_state = app_handle.state::<AppState>();
        // Read first: if a startup check has already been *spawned*, bail.
        if app_state
            .startup_update_check_started
            .load(Ordering::SeqCst)
        {
            return;
        }
        // Try to claim the workflow lock. If a different workflow (e.g. a
        // manual check spawned before the startup check could run) is in
        // flight, we leave `startup_update_check_started` as-is so a later
        // call (a follow-up `ensure_initialized` or a one-off retry) can
        // still arm the startup check once the workflow lock is free.
        if app_state
            .update_workflow_running
            .swap(true, Ordering::SeqCst)
        {
            return;
        }
        // Now that we own the workflow slot, mark the startup check as
        // started. If this races with another caller, the second caller
        // observes `true` and bails on the load above before reaching the
        // workflow lock.
        if app_state
            .startup_update_check_started
            .swap(true, Ordering::SeqCst)
        {
            // Another caller raced us to the workflow lock and the startup
            // flag flipped behind us. Release the workflow lock we just
            // claimed so that other caller's spawned workflow keeps owning
            // the slot.
            app_state
                .update_workflow_running
                .store(false, Ordering::SeqCst);
            return;
        }
        Self::spawn_workflow(app_handle.clone(), UpdateCheckOrigin::Startup);
    }

    pub fn start_manual_update_check(app_handle: &AppHandle) {
        let app_state = app_handle.state::<AppState>();
        if app_state
            .update_workflow_running
            .swap(true, Ordering::SeqCst)
        {
            // Another workflow is in progress. Record the manual request so
            // that workflow can apply manual semantics (e.g. UpToDate after
            // a no-update completion) and so a follow-up manual check fires
            // if the running workflow already passed that decision point.
            app_state
                .pending_manual_update_check
                .store(true, Ordering::SeqCst);
            return;
        }
        Self::spawn_workflow(app_handle.clone(), UpdateCheckOrigin::Manual);
    }

    fn spawn_workflow(app_handle: AppHandle, origin: UpdateCheckOrigin) {
        tauri::async_runtime::spawn(async move {
            let app_state = app_handle.state::<AppState>();
            app_state
                .run_update_workflow(app_handle.clone(), origin)
                .await;
        });
    }

    pub async fn dismiss_update_notification(&self, dismissal_key: String) {
        self.dismissed_update_notification_keys
            .lock()
            .unwrap()
            .insert(dismissal_key.clone());

        let should_clear = self
            .update_notification
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|notification| notification.dismissal_key() == dismissal_key);

        if should_clear {
            *self.update_notification.lock().unwrap() = None;
            self.emit_app_state_change().await;
        }
    }

    pub(crate) async fn publish_update_notification(
        &self,
        notification: Option<UpdateNotification>,
    ) {
        let visible_notification = notification.and_then(|notification| {
            let dismissal_key = notification.dismissal_key();
            let is_dismissed = self
                .dismissed_update_notification_keys
                .lock()
                .unwrap()
                .contains(&dismissal_key);
            if is_dismissed {
                None
            } else {
                Some(notification)
            }
        });

        *self.update_notification.lock().unwrap() = visible_notification;
        self.emit_app_state_change().await;
    }

    pub(crate) async fn clear_update_notification_if_key(&self, dismissal_key: &str) {
        let should_clear = self
            .update_notification
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|notification| notification.dismissal_key() == dismissal_key);

        if should_clear {
            *self.update_notification.lock().unwrap() = None;
            self.emit_app_state_change().await;
        }
    }

    pub(crate) async fn publish_download_progress_notification(
        &self,
        id: u64,
        version: String,
        progress_percent: u8,
    ) {
        let notification = UpdateNotification::new(
            id,
            UpdateNotificationPhase::Downloading {
                version,
                progress_percent: Some(progress_percent),
            },
        );
        let dismissal_key = notification.dismissal_key();
        let did_publish = {
            let is_dismissed = self
                .dismissed_update_notification_keys
                .lock()
                .unwrap()
                .contains(&dismissal_key);
            let mut current_notification = self.update_notification.lock().unwrap();

            let should_publish = !is_dismissed
                && current_notification.as_ref().is_some_and(|current| {
                    if current.id != id {
                        return false;
                    }
                    let UpdateNotificationPhase::Downloading {
                        progress_percent: existing,
                        ..
                    } = &current.phase
                    else {
                        return false;
                    };
                    should_publish_progress(*existing, progress_percent)
                });

            if should_publish {
                *current_notification = Some(notification);
            }

            should_publish
        };

        if did_publish {
            self.emit_app_state_change().await;
        }
    }

    /// Entry point for a workflow whose running flag has already been
    /// claimed by `start_*_update_check`. Releases the flag via RAII,
    /// publishes a Failed notification on error, and triggers a follow-up
    /// manual check if one was requested while the workflow ran past the
    /// no-update decision point.
    async fn run_update_workflow(&self, app_handle: AppHandle, origin: UpdateCheckOrigin) {
        let id = self
            .next_update_notification_id
            .fetch_add(1, Ordering::SeqCst);

        // Release the workflow flag no matter how we leave this scope —
        // happy path, early `?` return, or panic.
        let _running_guard = WorkflowGuard::new(&self.update_workflow_running);

        let result = self
            .run_update_check_inner(app_handle.clone(), origin, id)
            .await;

        if let Err(error_message) = result {
            error!("Application update failed: {}", error_message);
            self.publish_update_notification(Some(UpdateNotification::new(
                id,
                UpdateNotificationPhase::Failed {
                    message: humanize_update_error(&error_message),
                },
            )))
            .await;
        }

        // Drop the running guard so a follow-up check can claim the slot.
        drop(_running_guard);

        // If a manual click landed after the no-update decision (or never
        // got consumed because the workflow took a different branch), honor
        // it now by running a fresh manual check.
        if self
            .pending_manual_update_check
            .swap(false, Ordering::SeqCst)
        {
            Self::start_manual_update_check(&app_handle);
        }
    }

    async fn run_update_check_inner(
        &self,
        app_handle: AppHandle,
        origin: UpdateCheckOrigin,
        id: u64,
    ) -> Result<(), String> {
        self.publish_update_notification(Some(UpdateNotification::new(
            id,
            UpdateNotificationPhase::Checking,
        )))
        .await;

        info!("Checking for application updates");
        // The Windows updater calls `std::process::exit(0)` from inside
        // `Update::install`, which bypasses our `RunEvent::ExitRequested`
        // handler in `lib.rs` that releases an active project mutex. Register
        // an `on_before_exit` hook so the mutex is released before the
        // process disappears. The hook also preserves the plugin's default
        // `cleanup_before_exit` call so tray icons / resource tables drop.
        // The hook is only invoked on Windows (`install_inner` on macOS /
        // Linux does not call it), but registering it always is cheap and
        // resilient against future plugin changes.
        let app_handle_for_hook = app_handle.clone();
        let updater = app_handle
            .updater_builder()
            .on_before_exit(move || {
                if let Some(project_id) = app_handle_for_hook
                    .state::<AppState>()
                    .get_active_project_id()
                {
                    info!(
                        "Updater on_before_exit: releasing mutex for project {}",
                        project_id
                    );
                    tauri::async_runtime::block_on(async {
                        let app_state = app_handle_for_hook.state::<AppState>();
                        api::project::release_project_mutex(&app_state.api_info(), project_id)
                            .await
                            .ok();
                    });
                }
                app_handle_for_hook.cleanup_before_exit();
            })
            .build()
            .map_err(|e| e.to_string())?;
        let Some(update) = updater.check().await.map_err(|e| e.to_string())? else {
            // If a manual click arrived during this workflow, treat the
            // no-update completion as manual so the user gets feedback.
            let effective_origin = if origin == UpdateCheckOrigin::Manual
                || self
                    .pending_manual_update_check
                    .swap(false, Ordering::SeqCst)
            {
                UpdateCheckOrigin::Manual
            } else {
                UpdateCheckOrigin::Startup
            };

            if let Some(notification) = no_update_notification(effective_origin, id) {
                let dismissal_key = notification.dismissal_key();
                self.publish_update_notification(Some(notification)).await;
                let app_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(Duration::from_secs(4)).await;
                    let app_state = app_handle.state::<AppState>();
                    app_state
                        .clear_update_notification_if_key(&dismissal_key)
                        .await;
                });
            } else {
                self.publish_update_notification(None).await;
            }
            return Ok(());
        };

        let version = update.version.clone();
        self.publish_update_notification(Some(UpdateNotification::new(
            id,
            UpdateNotificationPhase::Downloading {
                version: version.clone(),
                progress_percent: None,
            },
        )))
        .await;

        let progress = Arc::new(Mutex::new(DownloadProgress::default()));
        let progress_for_download = progress.clone();
        let app_handle_for_progress = app_handle.clone();
        let version_for_progress = version.clone();
        let bytes = update
            .download(
                move |chunk_len, content_len| {
                    let percent = progress_for_download
                        .lock()
                        .unwrap()
                        .record_chunk(chunk_len, content_len);

                    if let Some(percent) = percent {
                        let app_handle = app_handle_for_progress.clone();
                        let version = version_for_progress.clone();
                        tauri::async_runtime::spawn(async move {
                            let app_state = app_handle.state::<AppState>();
                            app_state
                                .publish_download_progress_notification(id, version, percent)
                                .await;
                        });
                    }
                },
                || debug!("Application update download finished"),
            )
            .await
            .map_err(|e| e.to_string())?;

        self.publish_update_notification(Some(UpdateNotification::new(
            id,
            UpdateNotificationPhase::Installing {
                version: version.clone(),
            },
        )))
        .await;

        // On Windows, `update.install` extracts and launches the bundled
        // installer, then calls `std::process::exit(0)` from inside the
        // updater plugin. The post-install code (Relaunching publish,
        // restart) is therefore unreachable on Windows. Publish Relaunching
        // *before* the install call so the user actually sees that phase
        // before the process disappears.
        #[cfg(target_os = "windows")]
        self.publish_update_notification(Some(UpdateNotification::new(
            id,
            UpdateNotificationPhase::Relaunching {
                version: version.clone(),
            },
        )))
        .await;

        // `update.install` is synchronous and on macOS triggers an
        // AppleScript dialog with admin privileges; running it on the async
        // runtime would stall every other task. Move it to the blocking
        // pool. The closure also captures `bytes` by move so we don't keep
        // the buffer alive longer than necessary.
        let update_for_install = update.clone();
        tauri::async_runtime::spawn_blocking(move || update_for_install.install(bytes))
            .await
            .map_err(|join_error| format!("Update install task failed: {join_error}"))?
            .map_err(|e| e.to_string())?;

        // On macOS / Linux, install returned successfully (the .app bundle
        // or AppImage has been replaced on disk) and we still need to
        // explicitly trigger a relaunch. `restart()` returns `!`, which
        // coerces to `Result<(), String>` so the function type-checks.
        #[cfg(not(target_os = "windows"))]
        {
            self.publish_update_notification(Some(UpdateNotification::new(
                id,
                UpdateNotificationPhase::Relaunching { version },
            )))
            .await;

            // Give the WebView one render frame to display the Relaunching
            // toast before we tear it down. `restart()` parks this worker
            // thread forever and asks the main thread to fire
            // `RunEvent::ExitRequested` → `Exit`; without this short delay
            // the exit request can preempt the IPC emit and the user never
            // sees the final phase change.
            tokio::time::sleep(Duration::from_millis(150)).await;

            app_handle.restart()
        }

        // On Windows the install call has already exited the process, so
        // this line is unreachable. Keep the explicit Ok(()) so the
        // function type-checks on platforms where install_inner returns.
        #[cfg(target_os = "windows")]
        Ok(())
    }
}

pub fn open_latest_release_url() -> Result<(), String> {
    // Detach stdio on every platform so a GUI-spawned helper does not
    // inherit the parent's pipes (which can keep `xdg-open`/`open` alive
    // longer than expected and which on Windows leaks a flashing `cmd`
    // console without `CREATE_NO_WINDOW`).
    #[cfg(target_os = "macos")]
    let command = Command::new("open")
        .arg(LATEST_RELEASE_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    #[cfg(target_os = "linux")]
    let command = Command::new("xdg-open")
        .arg(LATEST_RELEASE_URL)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    #[cfg(target_os = "windows")]
    let command = {
        // CREATE_NO_WINDOW prevents a console window from briefly flashing
        // behind the GUI when we shell out to `cmd /C start`.
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        use std::os::windows::process::CommandExt;

        Command::new("cmd")
            .args(["/C", "start", "", LATEST_RELEASE_URL])
            .creation_flags(CREATE_NO_WINDOW)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
    };

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let command = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "opening URLs is not supported on this platform",
    ));

    command
        .map(|_| ())
        .map_err(|e| format!("Failed to open latest release page: {e}"))
}

#[cfg(test)]
mod tests {
    use super::{
        APP_NAME, DownloadProgress, UpdateCheckOrigin, WorkflowGuard, humanize_update_error,
        no_update_notification, should_publish_progress,
    };
    use crate::state::AppState;
    use common::ui_state::{UpdateNotification, UpdateNotificationPhase};
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Builds a Downloading-phase notification for the given workflow id and
    /// progress percent. Used by the dismiss/publish behavior tests below.
    fn downloading_notification(id: u64, progress_percent: Option<u8>) -> UpdateNotification {
        UpdateNotification::new(
            id,
            UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent,
            },
        )
    }

    fn installing_notification(id: u64) -> UpdateNotification {
        UpdateNotification::new(
            id,
            UpdateNotificationPhase::Installing {
                version: "0.2.0".to_string(),
            },
        )
    }

    #[test]
    fn download_progress_accumulates_chunks() {
        let mut progress = DownloadProgress::default();

        assert_eq!(progress.record_chunk(25, Some(100)), Some(25));
        assert_eq!(progress.record_chunk(25, Some(100)), Some(50));
    }

    #[test]
    fn download_progress_is_clamped_to_one_hundred() {
        let mut progress = DownloadProgress::default();

        assert_eq!(progress.record_chunk(150, Some(100)), Some(100));
    }

    #[test]
    fn download_progress_is_unknown_without_content_length() {
        let mut progress = DownloadProgress::default();

        assert_eq!(progress.record_chunk(25, None), None);
    }

    /// `Some(0)` is a valid-but-unusable Content-Length: dividing by zero
    /// would either panic or produce garbage. The helper must report
    /// progress as unknown (returning `None`), exercising the explicit
    /// guard inside `record_chunk`.
    #[test]
    fn download_progress_is_unknown_for_zero_content_length() {
        let mut progress = DownloadProgress::default();

        assert_eq!(progress.record_chunk(25, Some(0)), None);
    }

    #[test]
    fn startup_no_update_stays_quiet() {
        assert!(no_update_notification(UpdateCheckOrigin::Startup, 1).is_none());
    }

    #[test]
    fn manual_no_update_reports_up_to_date() {
        let notification = no_update_notification(UpdateCheckOrigin::Manual, 1)
            .expect("manual checks should show an up-to-date notification");

        assert_eq!(
            notification.phase,
            UpdateNotificationPhase::UpToDate {
                app_name: APP_NAME.to_string()
            }
        );
    }

    #[test]
    fn humanize_update_error_falls_back_for_empty_messages() {
        assert_eq!(humanize_update_error("  "), "unknown updater error");
    }

    #[test]
    fn humanize_update_error_preserves_actionable_detail() {
        assert_eq!(
            humanize_update_error("signature mismatch"),
            "signature mismatch"
        );
    }

    #[test]
    fn humanize_update_error_trims_surrounding_whitespace() {
        assert_eq!(
            humanize_update_error("  network unreachable  "),
            "network unreachable"
        );
    }

    #[test]
    fn progress_publish_advances_from_none() {
        assert!(should_publish_progress(None, 0));
        assert!(should_publish_progress(None, 42));
    }

    #[test]
    fn progress_publish_advances_when_strictly_greater() {
        assert!(should_publish_progress(Some(20), 30));
        assert!(should_publish_progress(Some(99), 100));
    }

    #[test]
    fn progress_publish_skips_equal_or_regressing_percent() {
        // Equal percent is a no-op (already shown).
        assert!(!should_publish_progress(Some(50), 50));
        // Regressing percent is the stale-callback case we must reject.
        assert!(!should_publish_progress(Some(50), 30));
        assert!(!should_publish_progress(Some(100), 99));
    }

    /// `WorkflowGuard` is the only thing standing between a panicking
    /// workflow and a permanently-stuck `update_workflow_running` flag.
    /// Verify the Drop reset works in isolation.
    #[test]
    fn workflow_guard_resets_flag_on_drop() {
        let flag = AtomicBool::new(true);
        {
            let _guard = WorkflowGuard::new(&flag);
            assert!(
                flag.load(Ordering::SeqCst),
                "guard construction must not change the flag"
            );
        }
        assert!(
            !flag.load(Ordering::SeqCst),
            "Drop should have cleared the flag"
        );
    }

    /// Dismissing the currently-displayed notification clears it and
    /// records the dismissal so future republishes of the same key are
    /// suppressed. The published notification was Downloading 30%; the
    /// dismissal arrives with the matching key.
    #[tokio::test]
    async fn dismiss_clears_current_when_key_matches() {
        let app_state = AppState::new();
        let notification = downloading_notification(7, Some(30));
        let dismissal_key = notification.dismissal_key();
        *app_state.update_notification.lock().unwrap() = Some(notification);

        app_state
            .dismiss_update_notification(dismissal_key.clone())
            .await;

        assert!(
            app_state.update_notification.lock().unwrap().is_none(),
            "matching dismissal must clear the current notification"
        );
        assert!(
            app_state
                .dismissed_update_notification_keys
                .lock()
                .unwrap()
                .contains(&dismissal_key),
            "dismissal must be recorded for future suppression"
        );
    }

    /// Dismissing a key that does not match the current phase records the
    /// dismissal but must not disturb the visible notification — otherwise
    /// a stale "dismiss" event from a previous phase would silently wipe
    /// the new phase's UI.
    #[tokio::test]
    async fn dismiss_keeps_current_when_key_differs() {
        let app_state = AppState::new();
        let displayed = installing_notification(7);
        let displayed_key = displayed.dismissal_key();
        *app_state.update_notification.lock().unwrap() = Some(displayed);

        let stale_downloading_key = downloading_notification(7, Some(30)).dismissal_key();
        app_state
            .dismiss_update_notification(stale_downloading_key.clone())
            .await;

        let current = app_state.update_notification.lock().unwrap().clone();
        assert_eq!(
            current.as_ref().map(UpdateNotification::dismissal_key),
            Some(displayed_key),
            "non-matching dismissal must not clear the current notification"
        );
        assert!(
            app_state
                .dismissed_update_notification_keys
                .lock()
                .unwrap()
                .contains(&stale_downloading_key),
            "key should still be recorded so a future Downloading republish stays suppressed"
        );
    }

    /// A pre-recorded dismissal key must suppress every subsequent publish
    /// that carries the same key, even if the previous notification has
    /// since been cleared. This is the contract that makes "dismiss
    /// Downloading once and you don't see Downloading again" actually hold.
    #[tokio::test]
    async fn publish_skips_dismissed_key() {
        let app_state = AppState::new();
        let notification = downloading_notification(7, None);
        let dismissal_key = notification.dismissal_key();
        app_state
            .dismissed_update_notification_keys
            .lock()
            .unwrap()
            .insert(dismissal_key);

        app_state
            .publish_update_notification(Some(notification))
            .await;

        assert!(
            app_state.update_notification.lock().unwrap().is_none(),
            "publish must drop notifications whose key has been dismissed"
        );
    }

    /// Verify the publish logic enforces strict monotonicity *given a
    /// well-defined call sequence*. The chunk callback may spawn publishes
    /// that land out of order on the async runtime; this test exercises
    /// the per-call decision (a stale 30% emit must not regress a freshly
    /// published 50%) without needing to interleave concurrent publishes.
    /// The pure-function counterpart `should_publish_progress` is unit
    /// tested above for None / equal / regressing inputs.
    #[tokio::test]
    async fn download_progress_advances_only_when_strictly_greater() {
        let app_state = AppState::new();
        *app_state.update_notification.lock().unwrap() =
            Some(downloading_notification(7, Some(50)));

        // Stale 30% callback arriving after 50% must be rejected.
        app_state
            .publish_download_progress_notification(7, "0.2.0".to_string(), 30)
            .await;
        let after_stale = app_state.update_notification.lock().unwrap().clone();
        assert_eq!(
            after_stale,
            Some(downloading_notification(7, Some(50))),
            "regressing percent must not overwrite the published progress"
        );

        // Fresh 60% advances the displayed progress.
        app_state
            .publish_download_progress_notification(7, "0.2.0".to_string(), 60)
            .await;
        let after_advance = app_state.update_notification.lock().unwrap().clone();
        assert_eq!(
            after_advance,
            Some(downloading_notification(7, Some(60))),
            "strictly-greater percent must advance the published progress"
        );
    }

    /// Once the workflow has progressed to Installing, any in-flight
    /// download progress callbacks must not retroactively flip the toast
    /// back to Downloading. This guard exists separately from the
    /// monotonicity check because phase transitions allocate a different
    /// dismissal key.
    #[tokio::test]
    async fn download_progress_ignored_after_phase_advances_to_installing() {
        let app_state = AppState::new();
        let installing = installing_notification(7);
        *app_state.update_notification.lock().unwrap() = Some(installing.clone());

        app_state
            .publish_download_progress_notification(7, "0.2.0".to_string(), 99)
            .await;

        let current = app_state.update_notification.lock().unwrap().clone();
        assert_eq!(
            current,
            Some(installing),
            "stale Downloading progress must not regress the UI from Installing"
        );
    }

    /// A different workflow id (e.g. a manual retry) must not have its
    /// progress callbacks land on a previous workflow's notification.
    #[tokio::test]
    async fn download_progress_ignored_when_workflow_id_differs() {
        let app_state = AppState::new();
        *app_state.update_notification.lock().unwrap() =
            Some(downloading_notification(7, Some(50)));

        app_state
            .publish_download_progress_notification(8, "0.2.0".to_string(), 90)
            .await;

        let current = app_state.update_notification.lock().unwrap().clone();
        assert_eq!(
            current,
            Some(downloading_notification(7, Some(50))),
            "progress for a different workflow id must not overwrite the active notification"
        );
    }

    /// Auto-clear (used to dismiss the manual `up to date` toast after a
    /// short sleep) must only clear when the current notification's key
    /// still matches. If the user has since dismissed the toast or a new
    /// workflow has started, the auto-clear is a no-op.
    #[tokio::test]
    async fn clear_update_notification_if_key_only_clears_matching_key() {
        let app_state = AppState::new();
        let up_to_date = UpdateNotification::new(
            3,
            UpdateNotificationPhase::UpToDate {
                app_name: APP_NAME.to_string(),
            },
        );
        let up_to_date_key = up_to_date.dismissal_key();
        *app_state.update_notification.lock().unwrap() = Some(up_to_date);

        // Mismatching key — must not clear.
        app_state
            .clear_update_notification_if_key("99:checking")
            .await;
        assert!(
            app_state.update_notification.lock().unwrap().is_some(),
            "non-matching key must not clear the current notification"
        );

        // Matching key — clears.
        app_state
            .clear_update_notification_if_key(&up_to_date_key)
            .await;
        assert!(
            app_state.update_notification.lock().unwrap().is_none(),
            "matching key must clear the current notification"
        );
    }
}
