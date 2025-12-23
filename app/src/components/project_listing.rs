use crate::components::create_project_modal::CreateProjectModal;
use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;
use common::UiState;
use common::api_types::ProjectInfo;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectListingProps {
    pub ui_state: UiState,
}

#[function_component(ProjectListing)]
pub fn project_listing(ProjectListingProps { ui_state }: &ProjectListingProps) -> Html {
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let show_create_modal = use_state(|| false);

    // Button handlers
    let on_create_new = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_| {
            show_create_modal.set(true);
        })
    };

    let on_refresh = {
        let loading = loading.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let loading = loading.clone();
            let error = error.clone();
            loading.set(true);
            error.set(None);
            spawn_local(async move {
                match SPELEO_DB_CONTROLLER.fetch_projects().await {
                    Ok(project_list) => {
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                    }
                }
            });
        })
    };

    // Modal handlers
    let on_close_modal = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_: ()| {
            show_create_modal.set(false);
        })
    };

    let on_create_success = {
        let show_create_modal = show_create_modal.clone();
        let loading = loading.clone();
        let error = error.clone();

        Callback::from(move |new_project: ProjectInfo| {
            show_create_modal.set(false);

            // Refresh the project list
            let loading = loading.clone();
            let error = error.clone();
            let new_project = new_project.clone();

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                match SPELEO_DB_CONTROLLER.fetch_projects().await {
                    Ok(project_list) => {
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                    }
                }
            });
        })
    };

    // Render the UI
    if let Some(err_msg) = &*error {
        html! {
            <>
                <section style="width:100%;">
                    <h2>{"Project Listing"}</h2>
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <button onclick={on_create_new.clone()}>{"Create New Project"}</button>
                        <button onclick={on_refresh.clone()}>{"Refresh Projects"}</button>
                    </div>
                    <div class="error-message" style="color: red; padding: 12px; border: 1px solid red; border-radius: 4px;">
                        <strong>{"Error: "}</strong>
                        <span>{ err_msg }</span>
                    </div>
                </section>
                {
                    if *show_create_modal {
                        html! { <CreateProjectModal on_close={on_close_modal} on_success={on_create_success} /> }
                    } else {
                        html! {}
                    }
                }
            </>
        }
    } else {
        html! {
            <>
                <section style="width:100%;">
                    <h2>{"Project Listing"}</h2>
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <button onclick={on_create_new.clone()}>{"Create New Project"}</button>
                        <button onclick={on_refresh.clone()}>{"Refresh Projects"}</button>
                    </div>
                    <div class="projects-list" style="display: flex; flex-direction: column; gap: 12px; margin-top: 16px;">
                        { for ui_state.project_info.iter().map(|project| {
                            let project_id = project.id;
                            let on_card_click = Callback::from( move |_| {
                                spawn_local( async move {
                                    SPELEO_DB_CONTROLLER.set_active_project(project_id).await.unwrap();
                                });
                            });

                            let is_locked = project.active_mutex.is_some();
                            let lock_status = if is_locked { "🔒 Locked" } else { "🔓 Unlocked" };
                            let lock_color = if is_locked { "#ff6b6b" } else { "#51cf66" };

                            html! {
                                <div
                                    class="project-card"
                                    onclick={on_card_click}
                                    style={
                                        "border: 1px solid #ddd; \
                                         border-radius: 8px; \
                                         padding: 16px; \
                                         cursor: pointer; \
                                         transition: all 0.2s; \
                                         background-color: white; \
                                         box-shadow: 0 2px 4px rgba(0,0,0,0.1); \
                                         display: flex; \
                                         justify-content: space-between; \
                                         align-items: center;"
                                    }
                                >
                                    <h3 style="margin: 0; font-size: 18px; color: #2c3e50;">
                                        { &project.name }
                                    </h3>
                                    <div style="display: flex; gap: 12px; align-items: center;">
                                        <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;",
                                        if project.permission == "ADMIN" { " #ff7f00" } else if project.permission == "READ_AND_WRITE" { "#228be6" } else { "#868e96" }
                                        )}>
                                            { &project.permission }
                                        </span>
                                        <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;", lock_color)}>
                                            { lock_status }
                                        </span>
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                </section>

                {
                    if *show_create_modal {
                        html! {
                            <CreateProjectModal
                                on_close={on_close_modal}
                                on_success={on_create_success}
                            />
                        }
                    } else {
                        html! {}
                    }
                }
            </>
        }
    }
}
