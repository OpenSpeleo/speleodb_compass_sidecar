use common::api_types::ProjectInfo;
use log::{error, info};
use serde::Serialize;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

use crate::{
    components::{project_details::ProjectDetails, project_listing::ProjectListing},
    invoke,
    speleo_db_controller::SPELEO_DB_CONTROLLER,
};

#[derive(Clone, Copy, PartialEq)]
enum ActiveTab {
    Listing,
    Details,
}

#[function_component(App)]
pub fn app() -> Html {
    // Fields
    let instance = use_state(|| "https://speleodb.org".to_string());
    // Kinda dorky to force type inference
    let email = use_state(|| {
        let initial_state: Option<String> = None;
        initial_state
    });
    let password = use_state(|| {
        let initial_state: Option<String> = None;
        initial_state
    });
    let oauth = use_state(|| {
        let initial_state: Option<String> = None;
        initial_state
    });

    // UI state
    let loading = use_state(|| false);
    let logged_in = use_state(|| false);
    let show_error = use_state(|| false);
    let error_msg = use_state(String::new);
    let error_is_403 = use_state(|| false);
    let active_tab = use_state(|| ActiveTab::Listing);
    let selected_project: UseStateHandle<Option<ProjectInfo>> = use_state(|| None);
    let refresh_trigger = use_state(|| 0u32);
    // Silent mode for validation errors (true on startup/auto-login, false on interaction)
    let validation_silent = use_state(|| true);

    let validate_email = |val: &str| -> bool {
        if val.is_empty() {
            return true;
        } // empty is ok
        let parts: Vec<&str> = val.split('@').collect();
        parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') && parts[1].len() > 2
    };

    let validate_oauth = |val: &str| -> bool {
        if val.is_empty() {
            return true;
        } // empty is ok
        val.len() == 40 && val.chars().all(|c| c.is_ascii_hexdigit())
    };

    // Handlers
    let on_reset = {
        let instance = instance.clone();
        let email = email.clone();
        let password = password.clone();
        let oauth = oauth.clone();
        let show_error = show_error.clone();
        let error_msg = error_msg.clone();
        Callback::from(move |_| {
            instance.set("https://www.speleoDB.org".to_string());
            email.set(None);
            password.set(None);
            oauth.set(None);
            show_error.set(false);
            error_msg.set(String::new());
        })
    };

    let on_forget = {
        let instance = instance.clone();
        let email = email.clone();
        let password = password.clone();
        let oauth = oauth.clone();
        Callback::from(move |_| {
            let instance = instance.clone();
            let email = email.clone();
            let password = password.clone();
            let oauth = oauth.clone();
            spawn_local(async move {
                let _: () = invoke("forget_user_prefs", &()).await.unwrap();
                instance.set("https://www.speleoDB.org".to_string());
                email.set(None);
                password.set(None);
                oauth.set(None);
            });
        })
    };

    let on_connect = {
        let instance = instance.clone();
        let email = email.clone();
        let password = password.clone();
        let oauth = oauth.clone();
        let loading = loading.clone();
        let logged_in = logged_in.clone();
        let show_error = show_error.clone();
        let error_msg = error_msg.clone();
        let error_is_403 = error_is_403.clone();
        let validation_silent = validation_silent.clone();
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            validation_silent.set(false);
            // quick validation
            let oauth_ok = oauth.as_deref().is_some_and(|oauth| {
                oauth.len() == 40 && oauth.chars().all(|c| c.is_ascii_hexdigit())
            });

            let pass_ok = email.as_deref().is_some_and(|email| {
                password.as_deref().is_some_and(|_password| {
                    let parts: Vec<&str> = email.split('@').collect();
                    parts.len() == 2 && parts[1].contains('.')
                })
            });

            if !(oauth_ok ^ pass_ok) {
                error_msg.set("Must provide exactly one auth method: either email+password or a 40-char OAUTH token".to_string());
                error_is_403.set(false);
                show_error.set(true);
                return;
            }

            loading.set(true);
            let loading = loading.clone();
            let logged_in = logged_in.clone();
            let show_error = show_error.clone();
            let error_msg = error_msg.clone();
            let error_is_403 = error_is_403.clone();

            // snapshot current field values so we don't move the state handles into the async block
            let instance_val = (*instance).clone();
            let email_val = (*email).clone();
            let password_val = (*password).clone();
            let oauth_val = (*oauth).clone();

            spawn_local(async move {
                let url = match instance_val.parse() {
                    Ok(url) => url,
                    Err(_) => {
                        error_msg.set("Instance URL is invalid".to_string());
                        error_is_403.set(false);
                        show_error.set(true);
                        loading.set(false);
                        return;
                    }
                };
                // use the singleton controller to authenticate; it will save prefs on success
                match SPELEO_DB_CONTROLLER
                    .authenticate(
                        email_val.as_deref(),
                        password_val.as_deref(),
                        oauth_val.as_deref(),
                        &url,
                    )
                    .await
                {
                    Ok(()) => {
                        logged_in.set(true);
                    }
                    Err(e) => {
                        // Check if it's a 403 error (invalid credentials)
                        let is_403 = e.contains("403") || e.to_lowercase().contains("forbidden");
                        error_is_403.set(is_403);
                        error_msg.set(e);
                        show_error.set(true);
                    }
                }
                loading.set(false);
            });
        })
    };

    // UI pieces
    let on_instance_input = {
        let instance = instance.clone();
        let validation_silent = validation_silent.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_dyn_into().unwrap();
            instance.set(input.value().parse().unwrap());
            validation_silent.set(false);
        })
    };

    let on_instance_blur = {
        let instance = instance.clone();
        Callback::from(move |e: FocusEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let mut value = input.value();
            // Auto-trim trailing slash on blur
            if value.ends_with('/') {
                value = value.trim_end_matches('/').to_string();

                instance.set(value.parse().unwrap());
            }
        })
    };

    let on_email_input = {
        let email = email.clone();
        let validation_silent = validation_silent.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            email.set(Some(input.value()));
            validation_silent.set(false);
        })
    };

    let on_password_input = {
        let password = password.clone();
        let validation_silent = validation_silent.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            password.set(Some(input.value()));
            validation_silent.set(false);
        })
    };

    let on_oauth_input = {
        let oauth = oauth.clone();
        let validation_silent = validation_silent.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            oauth.set(Some(input.value()));
            validation_silent.set(false);
        })
    };

    let close_error = {
        let show_error = show_error.clone();
        let error_is_403 = error_is_403.clone();
        Callback::from(move |_| {
            show_error.set(false);
            error_is_403.set(false);
        })
    };

    // Disconnect handler: clear OAuth token in prefs and form, reset UI to login
    let on_disconnect = {
        let instance = instance.clone();
        let email = email.clone();
        let password = password.clone();
        let oauth = oauth.clone();
        let logged_in = logged_in.clone();
        let active_tab = active_tab.clone();
        let selected_project = selected_project.clone();
        Callback::from(move |_| {
            let instance_val = (*instance).clone();
            let email = email.clone();
            let password = password.clone();
            let oauth = oauth.clone();
            let logged_in = logged_in.clone();
            let active_tab = active_tab.clone();
            let selected_project = selected_project.clone();
            spawn_local(async move {
                /// Empty struct for no-argument invocations.
                #[derive(Serialize)]
                struct UnitArgs {}
                let args = UnitArgs {};
                let _: () = invoke("forget_user_prefs", &args).await.unwrap();
                // Clear form fields
                email.set(None);
                password.set(None);
                oauth.set(None);
                // Reset in-app state
                selected_project.set(None);
                active_tab.set(ActiveTab::Listing);
                logged_in.set(false);
            });
        })
    };

    // Project selection from listing
    let on_project_selected = {
        let selected_project = selected_project.clone();
        let active_tab = active_tab.clone();
        Callback::from(move |project: ProjectInfo| {
            selected_project.set(Some(project));
            active_tab.set(ActiveTab::Details);
        })
    };

    // Back to project listing
    let on_back_to_listing = {
        let active_tab = active_tab.clone();
        let refresh_trigger = refresh_trigger.clone();
        Callback::from(move |_| {
            active_tab.set(ActiveTab::Listing);
            // Increment refresh trigger to force project list refresh
            refresh_trigger.set(*refresh_trigger + 1);
        })
    };

    // Derived UI state
    // Enable connect button even if invalid, so user can click and see validation errors
    let is_connect_disabled = *loading;

    // Check if both auth methods are being used (mutually exclusive)
    let has_oauth = oauth.is_some();
    let has_email = email.is_some();
    let has_password = password.is_some();
    let both_auth_methods_used = has_oauth && (has_email || has_password);

    // Field validation state for visual feedback
    // Only show errors if not in silent mode
    let show_errors = !*validation_silent;
    let email_invalid = show_errors && email.as_deref().is_some_and(|email| !validate_email(email));
    let oauth_invalid = show_errors && oauth.as_deref().is_some_and(|oauth| !validate_oauth(oauth));

    // Show error on auth fields if both methods are used
    let email_conflict = both_auth_methods_used && has_email;
    let password_conflict = both_auth_methods_used && has_password;
    let oauth_conflict = both_auth_methods_used && has_oauth;

    // Show error if email/password incomplete
    let email_missing_password = has_email && !has_password && !has_oauth;
    let password_missing_email = has_password && !has_email && !has_oauth;

    if *logged_in {
        html! {
            <main class="container">
                <header style="display:flex; justify-content:space-between; align-items:center; margin-bottom:24px; width:100%;">
                    <h1>{"SpeleoDB - Compass Sidecar"}</h1>
                    <div style="gap:8px;">
                        <button style="background-color:red; color:white;" onclick={on_disconnect.clone()}>{ "Disconnect" }</button>
                    </div>
                </header>
                <section style="width:100%;">
                    {
                        if *active_tab == ActiveTab::Listing {
                            html!{ <ProjectListing on_select={on_project_selected.clone()} refresh_trigger={*refresh_trigger} /> }
                        } else if let Some(project) = &*selected_project {
                            html!{ <ProjectDetails project={project.clone()} on_back={on_back_to_listing.clone()} /> }
                        } else {
                            html!{ <p>{"Select a project from the listing first."}</p> }
                        }
                    }
                </section>
            </main>
        }
    } else {
        html! {
            <main class="container">
                <h1>{"SpeleoDB - Compass Sidecar"}</h1>

                <div class="row">
                    <img src="public/speleodb_long.png" class="logo speleodb" alt="SpeleoDB logo"/>
                </div>

                <form onsubmit={on_connect} class="auth-form">
                    <div class="auth-group">
                        <label for="instance">{"SpeleoDB instance"}</label>
                        <input
                            id="instance"
                            type="text"
                            class={if false { "full invalid" } else { "full" }}
                            oninput={on_instance_input}
                            onblur={on_instance_blur}
                            placeholder="https://www.speleodb.org"
                        />
                        { if false {
                            html!{ <span class="field-error">{"Must start with http:// or https://"}</span> }
                        } else { html!{} } }
                    </div>

                    <div class="accent-bar" aria-hidden="true" />

                    <p class="hint">{"Authenticate with either Email & Password or OAuth token."}</p>

                    <div class="auth-group">
                        <label for="email">{"Email"}</label>
                        <input
                            id="email"
                            type="email"
                            class={if email_invalid || email_conflict || email_missing_password { "full invalid" } else { "full" }}
                            value={(*email).clone()}
                            oninput={on_email_input}
                            placeholder="your@email.com"
                        />
                        { if email_invalid {
                            html!{ <span class="field-error">{"Invalid email format"}</span> }
                        } else if email_conflict {
                            html!{ <span class="field-error">{"Cannot use both email/password AND OAuth token"}</span> }
                        } else if email_missing_password {
                            html!{ <span class="field-error">{"Password is required when using email"}</span> }
                        } else { html!{} } }
                    </div>

                    <div class="auth-group">
                        <label for="password">{"Password"}</label>
                        <input
                            id="password"
                            type="password"
                            placeholder="Your SpeleoDB Password"
                            class={if password_conflict || password_missing_email { "full invalid" } else { "full" }}
                            value={(*password).clone()}
                            oninput={on_password_input}
                        />
                        { if password_conflict {
                            html!{ <span class="field-error">{"Cannot use both email/password AND OAuth token"}</span> }
                        } else if password_missing_email {
                            html!{ <span class="field-error">{"Email is required when using password"}</span> }
                        } else { html!{} } }
                    </div>

                    <hr style="border-top: dotted 1px #000; background-color: transparent; border-style: none none dotted; width: 100%;" />

                    <div class="auth-group">
                        <label for="oauth">{"OAUTH Token"}</label>
                        <input
                            id="oauth"
                            type="text"
                            class={if oauth_invalid || oauth_conflict { "full invalid" } else { "full" }}
                            value={(*oauth).clone()}
                            oninput={on_oauth_input}
                            placeholder="Your SpeleoDB OAuth token"
                        />
                        { if oauth_invalid {
                            html!{ <span class="field-error">{"Must be 40 hexadecimal characters"}</span> }
                        } else if oauth_conflict {
                            html!{ <span class="field-error">{"Cannot use both OAuth token AND email/password"}</span> }
                        } else { html!{} } }
                    </div>

                    <div class="accent-bar" aria-hidden="true" />

                    <div class="actions">
                        <button type="button" onclick={on_forget} style="background-color:#c84d4d;color:white;border:none;padding:8px 16px;border-radius:4px;cursor:pointer;font-weight:500;width: 14em;">{"Delete Saved Credentials"}</button>
                        <button type="button" onclick={on_reset} style="background-color:#6b7280;color:white;border:none;padding:8px 16px;border-radius:4px;cursor:pointer;font-weight:500;width: 14em;">{"Reset Form"}</button>
                        <button type="submit" disabled={is_connect_disabled} style="background-color:#2563eb;color:white;border:none;padding:8px 16px;border-radius:4px;cursor:pointer;font-weight:500;width: 14em;">{ if *loading { "Connecting..." } else { "Connect" } }</button>
                    </div>
                </form>

                { if *show_error {
                    html!{
                        <div class="modal">
                            <div class="modal-card">
                                <h3>{"Connection failed"}</h3>
                                { if *error_is_403 {
                                    html!{
                                        <>
                                            <p class="error-403"><strong>{"Invalid credentials"}</strong></p>
                                            <p>{"The email/password or OAuth token you provided is incorrect. Please check your credentials and try again."}</p>
                                        </>
                                    }
                                } else {
                                    html!{ <p>{ &*error_msg }</p> }
                                }}
                                <div style="display:flex; justify-content:flex-end; gap:8px; margin-top:12px;">
                                    <button onclick={close_error.clone()}>{"Close"}</button>
                                </div>
                            </div>
                        </div>
                    }
                } else { html!{<></>} } }
            </main>
        }
    }
}
