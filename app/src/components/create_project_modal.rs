use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;
use common::api_types::ProjectInfo;
use std::collections::BTreeMap;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlInputElement;
use web_sys::HtmlTextAreaElement;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct CreateProjectModalProps {
    pub on_close: Callback<()>,
    pub on_success: Callback<ProjectInfo>,
}

#[function_component(CreateProjectModal)]
pub fn create_project_modal(props: &CreateProjectModalProps) -> Html {
    let name = use_state(String::new);
    let description = use_state(String::new);
    let country = use_state(String::new);
    let latitude = use_state(String::new);
    let longitude = use_state(String::new);

    let error_message = use_state(|| None::<String>);
    let is_submitting = use_state(|| false);

    // Load countries and sort by name
    let countries: Vec<(String, String)> = {
        let json = include_str!("assets/countries.json");
        let countries_map: BTreeMap<String, String> =
            serde_json::from_str(json).unwrap_or_default();
        let mut countries_vec: Vec<(String, String)> = countries_map.into_iter().collect();
        // Sort by country name (value) instead of code (key)
        countries_vec.sort_by(|a, b| a.1.cmp(&b.1));
        countries_vec
    };

    let on_submit = {
        let name = name.clone();
        let description = description.clone();
        let country = country.clone();
        let latitude = latitude.clone();
        let longitude = longitude.clone();
        let error_message = error_message.clone();
        let is_submitting = is_submitting.clone();
        let on_success = props.on_success.clone();

        Callback::from(move |e: SubmitEvent| {
            e.prevent_default();

            let name_val = (*name).clone();
            let desc_val = (*description).clone();
            let country_val = (*country).clone();
            let lat_val = (*latitude).clone();
            let lon_val = (*longitude).clone();

            // Validation
            if name_val.trim().is_empty() {
                error_message.set(Some("Project name is required".to_string()));
                return;
            }
            if name_val.len() > 255 {
                error_message.set(Some(
                    "Project name must be less than 255 characters".to_string(),
                ));
                return;
            }
            if desc_val.trim().is_empty() {
                error_message.set(Some("Description is required".to_string()));
                return;
            }
            if country_val.is_empty() {
                error_message.set(Some("Please select a country".to_string()));
                return;
            }
            // Validate Lat/Lon if provided
            if !lat_val.is_empty() && lat_val.parse::<f64>().is_err() {
                error_message.set(Some("Latitude must be a valid number".to_string()));
                return;
            }
            if !lon_val.is_empty() && lon_val.parse::<f64>().is_err() {
                error_message.set(Some("Longitude must be a valid number".to_string()));
                return;
            }

            let error_message = error_message.clone();
            let is_submitting = is_submitting.clone();
            let on_success = on_success.clone();

            is_submitting.set(true);
            error_message.set(None);

            spawn_local(async move {
                match SPELEO_DB_CONTROLLER
                    .create_project(
                        &name_val,
                        &desc_val,
                        &country_val,
                        if lat_val.is_empty() {
                            None
                        } else {
                            Some(lat_val.as_str())
                        },
                        if lon_val.is_empty() {
                            None
                        } else {
                            Some(lon_val.as_str())
                        },
                    )
                    .await
                {
                    Ok(project) => {
                        is_submitting.set(false);
                        on_success.emit(project);
                    }
                    Err(e) => {
                        is_submitting.set(false);
                        error_message.set(Some(e));
                    }
                }
            });
        })
    };

    html! {
        <div class="modal" style="
            position: fixed;
            top: 0;
            left: 0;
            width: 100vw;
            height: 100vh;
            background-color: rgba(0, 0, 0, 0.5);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 1000;
        ">
            <div class="modal-card" style="
                background-color: rgba(41, 62, 112, 1);
                color: #f6f6f6;
                border-radius: 12px;
                padding: 24px;
                max-width: 600px;
                width: 90%;
                box-shadow: 0 10px 25px rgba(0, 0, 0, 0.2);
                max-height: 90vh;
                overflow-y: auto;
            ">
                <h2 style="margin-top: 0; margin-bottom: 20px; color: #f6f6f6;">{"Create New Project"}</h2>

                {
                    if let Some(msg) = &*error_message {
                        html! {
                            <div style="
                                padding: 12px;
                                background-color: #fee2e2;
                                border: 1px solid #ef4444;
                                border-radius: 6px;
                                margin-bottom: 16px;
                                color: #b91c1c;
                            ">
                                {msg}
                            </div>
                        }
                    } else {
                        html! {}
                    }
                }

                <form onsubmit={on_submit}>
                    <div style="margin-bottom: 16px;">
                        <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #f6f6f6;">{"Project Name *"}</label>
                        <input
                            type="text"
                            value={(*name).clone()}
                            oninput={Callback::from(move |e: InputEvent| {
                                let input: HtmlInputElement = e.target_unchecked_into();
                                name.set(input.value());
                            })}
                            style="width: 100%; padding: 8px; border: 1px solid transparent; border-radius: 8px; box-sizing: border-box; font-family: inherit; font-size: 14px; background-color: rgb(205, 205, 205); color: #313131;"
                            placeholder="My Awesome Cave Project"
                            disabled={*is_submitting}
                        />
                    </div>

                    <div style="margin-bottom: 16px;">
                        <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #f6f6f6;">{"Description *"}</label>
                        <textarea
                            value={(*description).clone()}
                            oninput={Callback::from(move |e: InputEvent| {
                                let input: HtmlTextAreaElement = e.target_unchecked_into();
                                description.set(input.value());
                            })}
                            style="width: 100%; padding: 8px; border: 1px solid transparent; border-radius: 8px; min-height: 80px; box-sizing: border-box; font-family: inherit; font-size: 14px; resize: vertical; background-color: rgb(205, 205, 205); color: #313131;"
                            placeholder="Describe the project..."
                            disabled={*is_submitting}
                        />
                    </div>

                    <div style="margin-bottom: 16px;">
                        <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #f6f6f6;">{"Country *"}</label>
                        <select
                            value={(*country).clone()}
                            onchange={Callback::from(move |e: Event| {
                                if let Some(select) = e.target().and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok()) {
                                    country.set(select.value());
                                }
                            })}
                            style="width: 100%; padding: 8px; border: 1px solid transparent; border-radius: 8px; box-sizing: border-box; font-family: inherit; font-size: 14px; background-color: rgb(205, 205, 205); color: #313131;"
                            disabled={*is_submitting}
                        >
                            <option value="">{"Select a country..."}</option>
                            {
                                for countries.iter().map(|(code, name)| {
                                    html! {
                                        <option value={code.clone()}>{name}</option>
                                    }
                                })
                            }
                        </select>
                    </div>

                    <div style="margin-bottom: 24px;">
                        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 12px;">
                            <div>
                                <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #f6f6f6;">{"Latitude"}</label>
                                <input
                                    type="text"
                                    value={(*latitude).clone()}
                                    oninput={Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        latitude.set(input.value());
                                    })}
                                    style="width: 100%; padding: 8px; border: 1px solid transparent; border-radius: 8px; box-sizing: border-box; font-family: inherit; font-size: 14px; background-color: rgb(205, 205, 205); color: #313131;"
                                    placeholder="e.g. 45.1234"
                                    disabled={*is_submitting}
                                />
                            </div>
                            <div>
                                <label style="display: block; margin-bottom: 4px; font-weight: 500; color: #f6f6f6;">{"Longitude"}</label>
                                <input
                                    type="text"
                                    value={(*longitude).clone()}
                                    oninput={Callback::from(move |e: InputEvent| {
                                        let input: HtmlInputElement = e.target_unchecked_into();
                                        longitude.set(input.value());
                                    })}
                                    style="width: 100%; padding: 8px; border: 1px solid transparent; border-radius: 8px; box-sizing: border-box; font-family: inherit; font-size: 14px; background-color: rgb(205, 205, 205); color: #313131;"
                                    placeholder="e.g. -93.5678"
                                    disabled={*is_submitting}
                                />
                            </div>
                        </div>
                    </div>

                    <div style="display: flex; justify-content: flex-end; gap: 12px;">
                        <button
                            type="button"
                            onclick={props.on_close.reform(|_| ())}
                            style="
                                padding: 8px 16px;
                                border: 1px solid #d1d5db;
                                border-radius: 6px;
                                background-color: white;
                                color: #374151;
                                cursor: pointer;
                            "
                            disabled={*is_submitting}
                        >
                            {"Cancel"}
                        </button>
                        <button
                            type="submit"
                            style="
                                padding: 8px 16px;
                                border: none;
                                border-radius: 6px;
                                background-color: #2563eb;
                                color: white;
                                cursor: pointer;
                                font-weight: 500;
                            "
                            disabled={*is_submitting}
                        >
                            {if *is_submitting { "Creating..." } else { "Create Project" }}
                        </button>
                    </div>
                </form>
            </div>
        </div>
    }
}
