use crate::components::create_project_modal::CreateProjectModal;
use crate::components::project_listing_item::ProjectListingItem;
use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;
use common::api_types::ProjectInfo;
use common::ui_state::UiState;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectListingProps {
    pub ui_state: UiState,
}

#[function_component(ProjectListing)]
pub fn project_listing(ProjectListingProps { ui_state }: &ProjectListingProps) -> Html {
    let refreshed_on_load = use_state(|| false);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let show_create_modal = use_state(|| false);

    let refreshed_on_load_clone = refreshed_on_load.clone();
    let error_clone = error.clone();
    let loading_clone = loading.clone();
    use_effect(move || {
        if !*refreshed_on_load_clone {
            refreshed_on_load_clone.set(true);
            spawn_local(async move {
                match SPELEO_DB_CONTROLLER.fetch_projects().await {
                    Ok(()) => (),
                    Err(e) => {
                        error_clone.set(Some(e));
                        loading_clone.set(false);
                    }
                }
            });
        }
    });

    // Button handlers
    let on_create_new = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_| {
            show_create_modal.set(true);
        })
    };

    // Modal handlers
    let on_close_modal = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_: ()| {
            show_create_modal.set(false);
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
                    </div>
                    <div class="error-message" style="color: red; padding: 12px; border: 1px solid red; border-radius: 4px;">
                        <strong>{"Error: "}</strong>
                        <span>{ err_msg }</span>
                    </div>
                </section>
                {
                    if *show_create_modal {
                        html! { <CreateProjectModal on_close={on_close_modal}  /> }
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
                    </div>
                    <div class="projects-list" style="display: flex; flex-direction: column; gap: 12px; margin-top: 16px;">
                        { for ui_state.project_status.iter().map(|project| {
                            return html! {
                                <ProjectListingItem project={project.clone()} />
                            };
                        })}
                    </div>
                </section>

                {
                    if *show_create_modal {
                        html! {
                            <CreateProjectModal
                                on_close={on_close_modal}
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
