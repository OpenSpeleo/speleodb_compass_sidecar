use common::ui_state::{UpdateNotification, UpdateNotificationPhase};
use wasm_bindgen_futures::spawn_local;
use yew::{Callback, Html, Properties, classes, function_component, html};

use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;

#[derive(Properties, PartialEq)]
pub struct UpdateNotificationToastProps {
    pub notification: Option<UpdateNotification>,
}

pub(crate) fn update_notification_message(phase: &UpdateNotificationPhase) -> String {
    match phase {
        UpdateNotificationPhase::Checking => "Checking for updates...".to_string(),
        UpdateNotificationPhase::Downloading {
            version,
            progress_percent,
        } => match progress_percent {
            Some(progress_percent) => {
                format!("Downloading update {version} ({progress_percent}%)")
            }
            None => format!("Downloading update {version}..."),
        },
        UpdateNotificationPhase::Installing { version } => {
            format!("Installing update {version}...")
        }
        UpdateNotificationPhase::Relaunching { .. } => {
            "Update installed. Relaunching...".to_string()
        }
        UpdateNotificationPhase::UpToDate { app_name } => {
            format!("{app_name} is up to date.")
        }
        UpdateNotificationPhase::Failed { message } => {
            format!("Update failed: {message}")
        }
    }
}

pub(crate) fn update_notification_is_working(phase: &UpdateNotificationPhase) -> bool {
    matches!(
        phase,
        UpdateNotificationPhase::Checking
            | UpdateNotificationPhase::Downloading { .. }
            | UpdateNotificationPhase::Installing { .. }
            | UpdateNotificationPhase::Relaunching { .. }
    )
}

pub(crate) fn update_notification_is_error(phase: &UpdateNotificationPhase) -> bool {
    matches!(phase, UpdateNotificationPhase::Failed { .. })
}

#[function_component(UpdateNotificationToast)]
pub fn update_notification_toast(props: &UpdateNotificationToastProps) -> Html {
    let Some(notification) = props.notification.clone() else {
        return html! {};
    };

    let dismissal_key = notification.dismissal_key();
    let phase = notification.phase;
    let message = update_notification_message(&phase);
    let is_working = update_notification_is_working(&phase);
    let is_error = update_notification_is_error(&phase);

    let on_dismiss = {
        let dismissal_key = dismissal_key.clone();
        Callback::from(move |_| {
            let dismissal_key = dismissal_key.clone();
            spawn_local(async move {
                if let Err(e) = SPELEO_DB_CONTROLLER
                    .dismiss_update_notification(&dismissal_key)
                    .await
                {
                    log::error!("Failed to dismiss update notification: {}", e);
                }
            });
        })
    };

    let on_retry = Callback::from(move |_| {
        spawn_local(async move {
            if let Err(e) = SPELEO_DB_CONTROLLER.check_for_updates_now().await {
                log::error!("Failed to retry update check: {}", e);
            }
        });
    });

    let on_download_latest = Callback::from(move |_| {
        spawn_local(async move {
            if let Err(e) = SPELEO_DB_CONTROLLER.open_latest_release().await {
                log::error!("Failed to open latest release page: {}", e);
            }
        });
    });

    // Accessibility: The live region is scoped to the label + message
    // text. With `aria-atomic="false"` a screen reader announces only the
    // text node that changed — phase transitions read the new message,
    // download progress updates do *not* re-announce the static "Updates"
    // label every percent. The action buttons and the dismiss control sit
    // outside the live region so their appearance is not announced as
    // text; users navigate to them via Tab as with any other button.
    html! {
        <aside
            class={classes!(
                "update-notification",
                is_error.then_some("update-notification--error")
            )}
            aria-label="Update status"
        >
            <div class="update-notification__content">
                <div class="update-notification__status" aria-hidden="true">
                    {
                        if is_working {
                            html! { <span class="update-notification__spinner" /> }
                        } else {
                            html! { <span class="update-notification__dot" /> }
                        }
                    }
                </div>
                <div
                    class="update-notification__text"
                    role="status"
                    aria-live="polite"
                    aria-atomic="false"
                >
                    <div class="update-notification__label">{"Updates"}</div>
                    <div class="update-notification__message">{message}</div>
                </div>
                <button
                    type="button"
                    class="update-notification__dismiss"
                    aria-label="Dismiss update notification"
                    onclick={on_dismiss}
                >
                    {"\u{00D7}"}
                </button>
            </div>
            {
                if is_error {
                    html! {
                        <div class="update-notification__actions">
                            <button type="button" onclick={on_retry}>{"Retry"}</button>
                            <button type="button" onclick={on_download_latest}>{"Download Latest"}</button>
                        </div>
                    }
                } else {
                    html! {}
                }
            }
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::{
        update_notification_is_error, update_notification_is_working, update_notification_message,
    };
    use common::ui_state::UpdateNotificationPhase;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test_configure!(run_in_browser);

    /// Constructs every `UpdateNotificationPhase` variant for exhaustive
    /// matcher tests. If a new phase is added the compiler forces this
    /// helper to grow, which in turn forces the matchers below to declare
    /// where the new phase belongs.
    fn all_phases() -> Vec<UpdateNotificationPhase> {
        vec![
            UpdateNotificationPhase::Checking,
            UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: None,
            },
            UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: Some(42),
            },
            UpdateNotificationPhase::Installing {
                version: "0.2.0".to_string(),
            },
            UpdateNotificationPhase::Relaunching {
                version: "0.2.0".to_string(),
            },
            UpdateNotificationPhase::UpToDate {
                app_name: "SpeleoDB Compass Sidecar".to_string(),
            },
            UpdateNotificationPhase::Failed {
                message: "signature mismatch".to_string(),
            },
        ]
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn checking_message_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::Checking),
            "Checking for updates..."
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn downloading_message_without_progress_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: None,
            }),
            "Downloading update 0.2.0..."
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn downloading_message_with_progress_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: Some(42),
            }),
            "Downloading update 0.2.0 (42%)"
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn installing_message_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::Installing {
                version: "0.2.0".to_string(),
            }),
            "Installing update 0.2.0..."
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn relaunching_message_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::Relaunching {
                version: "0.2.0".to_string(),
            }),
            "Update installed. Relaunching..."
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn up_to_date_message_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::UpToDate {
                app_name: "SpeleoDB Compass Sidecar".to_string(),
            }),
            "SpeleoDB Compass Sidecar is up to date."
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn failed_message_matches_ux() {
        assert_eq!(
            update_notification_message(&UpdateNotificationPhase::Failed {
                message: "signature mismatch".to_string(),
            }),
            "Update failed: signature mismatch"
        );
    }

    /// `is_error` is `true` only for `Failed` and `false` for every other
    /// phase. Iterating over `all_phases()` ensures a future contributor
    /// who adds a new variant is forced to think about which bucket it
    /// belongs to.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn failed_is_the_only_error_phase() {
        for phase in all_phases() {
            let expected = matches!(phase, UpdateNotificationPhase::Failed { .. });
            assert_eq!(
                update_notification_is_error(&phase),
                expected,
                "is_error mismatch for phase {phase:?}"
            );
        }
    }

    /// Spinner is shown for every "in flight" phase (Checking, Downloading,
    /// Installing, Relaunching) and hidden for terminal phases (UpToDate,
    /// Failed). Same exhaustive iteration pattern as above.
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    fn working_phases_match_in_flight_phases_only() {
        for phase in all_phases() {
            let expected = matches!(
                phase,
                UpdateNotificationPhase::Checking
                    | UpdateNotificationPhase::Downloading { .. }
                    | UpdateNotificationPhase::Installing { .. }
                    | UpdateNotificationPhase::Relaunching { .. }
            );
            assert_eq!(
                update_notification_is_working(&phase),
                expected,
                "is_working mismatch for phase {phase:?}"
            );
        }
    }

    /// Behavioral tests: render the component via yew's
    /// [`LocalServerRenderer`] (gated behind the `ssr` dev-dependency
    /// feature flag) and assert against the produced HTML. These are
    /// deliberately substring-level so a small markup tweak (e.g. adding a
    /// wrapper class) does not flap them, but they do enforce the
    /// user-facing contract: which controls appear in which phase, the
    /// dismiss button's accessible name, the empty-render path, and the
    /// non-atomic live-region we added in this review pass.
    #[cfg(target_arch = "wasm32")]
    mod render {
        use super::super::{UpdateNotificationToast, UpdateNotificationToastProps};
        use common::ui_state::{UpdateNotification, UpdateNotificationPhase};
        use wasm_bindgen_test::wasm_bindgen_test;
        use yew::LocalServerRenderer;

        fn props_with(phase: UpdateNotificationPhase) -> UpdateNotificationToastProps {
            UpdateNotificationToastProps {
                notification: Some(UpdateNotification::new(7, phase)),
            }
        }

        async fn render_with(props: UpdateNotificationToastProps) -> String {
            LocalServerRenderer::<UpdateNotificationToast>::with_props(props)
                .render()
                .await
        }

        #[wasm_bindgen_test]
        async fn renders_nothing_when_notification_is_none() {
            let html = render_with(UpdateNotificationToastProps { notification: None }).await;
            // Yew SSR may emit invisible boundary markers (e.g. comments)
            // around an empty `html! {}`; the user-visible payload — any
            // <aside>, <button>, or "Updates" label — must be absent.
            assert!(
                !html.contains("<aside"),
                "no toast aside should render when notification is None: {html}"
            );
            assert!(
                !html.contains("Updates"),
                "no Updates label should render when notification is None: {html}"
            );
            assert!(
                !html.contains("<button"),
                "no buttons should render when notification is None: {html}"
            );
        }

        #[wasm_bindgen_test]
        async fn failed_phase_renders_retry_and_download_latest() {
            let html = render_with(props_with(UpdateNotificationPhase::Failed {
                message: "signature mismatch".to_string(),
            }))
            .await;
            assert!(
                html.contains(">Retry<"),
                "Failed phase must render Retry button: {html}"
            );
            assert!(
                html.contains(">Download Latest<"),
                "Failed phase must render Download Latest button: {html}"
            );
            // The error class on the wrapping aside drives the red accent.
            assert!(
                html.contains("update-notification--error"),
                "Failed phase must apply the error modifier class: {html}"
            );
        }

        #[wasm_bindgen_test]
        async fn checking_phase_omits_action_buttons() {
            let html = render_with(props_with(UpdateNotificationPhase::Checking)).await;
            assert!(
                !html.contains(">Retry<"),
                "non-Failed phases must not render Retry: {html}"
            );
            assert!(
                !html.contains(">Download Latest<"),
                "non-Failed phases must not render Download Latest: {html}"
            );
        }

        #[wasm_bindgen_test]
        async fn dismiss_button_has_accessible_name() {
            let html = render_with(props_with(UpdateNotificationPhase::Checking)).await;
            assert!(
                html.contains("aria-label=\"Dismiss update notification\""),
                "dismiss control must expose an accessible name: {html}"
            );
        }

        /// Progress percent updates must not re-announce the static
        /// "Updates" label every tick. The text region must carry
        /// `aria-live="polite"` and `aria-atomic="false"` so screen readers
        /// announce only the changed message text.
        #[wasm_bindgen_test]
        async fn message_lives_in_polite_non_atomic_region() {
            let html = render_with(props_with(UpdateNotificationPhase::Downloading {
                version: "0.2.0".to_string(),
                progress_percent: Some(42),
            }))
            .await;
            assert!(
                html.contains("aria-live=\"polite\""),
                "expected polite live region for screen readers: {html}"
            );
            assert!(
                html.contains("aria-atomic=\"false\""),
                "expected non-atomic announcement so progress ticks don't re-read label: {html}"
            );
        }
    }
}
