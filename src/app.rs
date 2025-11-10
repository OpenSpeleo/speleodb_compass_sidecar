use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use wasm_bindgen::JsValue;
use yew::prelude::*;

use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;


#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

#[function_component(App)]
pub fn app() -> Html {
    // Fields
    let instance = use_state(|| "https://www.speleoDB.org".to_string());
    let email = use_state(|| String::new());
    let password = use_state(|| String::new());
    let oauth = use_state(|| String::new());

    #[derive(Serialize, Deserialize)]
    struct Prefs {
        instance: String,
        #[serde(default)]
        email: String,
        #[serde(default)]
        password: String,
        oauth: String,
    }

    // Load saved prefs on startup (if any)
    {
        let instance = instance.clone();
        let email = email.clone();
        let password = password.clone();
        let oauth = oauth.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                let rv = invoke("load_user_prefs", JsValue::NULL).await;
                if let Some(s) = rv.as_string() {
                    match serde_json::from_str::<Prefs>(&s) {
                        Ok(p) => {
                            if !p.instance.is_empty() {
                                instance.set(p.instance);
                            }
                            email.set(p.email);
                            password.set(p.password);
                            oauth.set(p.oauth);
                        }
                        Err(_) => {}
                    }
                }
            });

            || ()
        });
    }

    // UI state
    let loading = use_state(|| false);
    let logged_in = use_state(|| false);
    let show_error = use_state(|| false);
    let error_msg = use_state(|| String::new());
    let error_is_403 = use_state(|| false);

    // Validation helpers
    let validate_instance = |val: &str| -> bool {
        let trimmed = val.trim_end_matches('/');
        trimmed.starts_with("http://") || trimmed.starts_with("https://")
    };

    let validate_email = |val: &str| -> bool {
        if val.is_empty() { return true; } // empty is ok
        let parts: Vec<&str> = val.split('@').collect();
        parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.') && parts[1].len() > 2
    };

    let validate_oauth = |val: &str| -> bool {
        if val.is_empty() { return true; } // empty is ok
        val.len() == 40 && val.chars().all(|c| c.is_ascii_hexdigit())
    };

    let form_valid = {
        let instance = instance.clone();
        let oauth = oauth.clone();
        let email = email.clone();
        let password = password.clone();
        move || {
            // instance must start with http:// or https://
            if !validate_instance(&instance) { return false; }
            // exactly one auth method: either oauth or email+password
            let oauth_ok = !oauth.is_empty() && oauth.len() == 40 && oauth.chars().all(|c| c.is_ascii_hexdigit());
            let pass_ok = oauth.is_empty() && !email.is_empty() && !password.is_empty() && validate_email(&email);
            oauth_ok ^ pass_ok
        }
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
            email.set(String::new());
            password.set(String::new());
            oauth.set(String::new());
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
                let _ = invoke("forget_user_prefs", JsValue::NULL).await;
                instance.set("https://www.speleoDB.org".to_string());
                email.set(String::new());
                password.set(String::new());
                oauth.set(String::new());
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
        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();
            // quick validation
            if !validate_instance(&*instance) {
                error_msg.set("SpeleoDB instance must start with http:// or https://".to_string());
                error_is_403.set(false);
                show_error.set(true);
                return;
            }

            let oauth_ok = !oauth.is_empty() && oauth.len() == 40 && oauth.chars().all(|c| c.is_ascii_hexdigit());
            let pass_ok = oauth.is_empty() && !email.is_empty() && !password.is_empty() && {
                let parts: Vec<&str> = email.split('@').collect();
                parts.len() == 2 && parts[1].contains('.')
            };

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
                // use the singleton controller to authenticate; it will save prefs on success
                match SPELEO_DB_CONTROLLER
                    .authenticate(&email_val, &password_val, &oauth_val, &instance_val)
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
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            let mut value = input.value();
            // Auto-trim trailing slash
            value = value.trim_end_matches('/').to_string();
            instance.set(value);
        })
    };

    let on_email_input = {
        let email = email.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            email.set(input.value());
        })
    };

    let on_password_input = {
        let password = password.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            password.set(input.value());
        })
    };

    let on_oauth_input = {
        let oauth = oauth.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            oauth.set(input.value());
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

    let back_to_form = {
        let logged_in = logged_in.clone();
        Callback::from(move |_| {
            logged_in.set(false);
        })
    };

    // Derived UI state
    let is_form_valid = form_valid();
    let is_connect_disabled = *loading || !is_form_valid;
    
    // Check if both auth methods are being used (mutually exclusive)
    let has_oauth = !oauth.is_empty();
    let has_email = !email.is_empty();
    let has_password = !password.is_empty();
    let both_auth_methods_used = has_oauth && (has_email || has_password);
    
    // Field validation state for visual feedback
    let instance_invalid = !instance.is_empty() && !validate_instance(&instance);
    let email_invalid = !email.is_empty() && !validate_email(&email);
    let oauth_invalid = !oauth.is_empty() && !validate_oauth(&oauth);
    
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
                <h1>{"SUCCESS"}</h1>
                <p>{"You are now logged in."}</p>
                <button onclick={back_to_form}>{"Back"}</button>
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
                            class={if instance_invalid { "full invalid" } else { "full" }}
                            value={(*instance).clone()} 
                            oninput={on_instance_input} 
                            placeholder="https://www.speleodb.org"
                        />
                        { if instance_invalid {
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
                            placeholder="40-character hexadecimal token"
                        />
                        { if oauth_invalid {
                            html!{ <span class="field-error">{"Must be 40 hexadecimal characters"}</span> }
                        } else if oauth_conflict {
                            html!{ <span class="field-error">{"Cannot use both OAuth token AND email/password"}</span> }
                        } else { html!{} } }
                    </div>

                    <div class="accent-bar" aria-hidden="true" />

                    <div class="actions">
                        <button type="button" onclick={on_reset}>{"Reset Form"}</button>
                        <button type="button" onclick={on_forget}>{"Forget Credentials"}</button>
                        <button type="submit" disabled={is_connect_disabled}>{ if *loading { "Connecting..." } else { "Connect" } }</button>
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
