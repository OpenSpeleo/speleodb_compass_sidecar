use crate::components::create_project_modal::CreateProjectModal;
use crate::components::project_listing_item::ProjectListingItem;
use common::ui_state::UiState;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectListingProps {
    pub ui_state: UiState,
}

#[function_component(ProjectListing)]
pub fn project_listing(ProjectListingProps { ui_state }: &ProjectListingProps) -> Html {
    let error = use_state(|| None::<String>);
    let show_create_modal = use_state(|| false);
    let user_email = ui_state.user_email.clone().unwrap();
    let project_list = ui_state.project_status.clone();
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
                <section style="max-width: 100vw">
                <div style="display:flex; justify-content:space-between;align-items:center;">
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <h2>{"Project Listing"}</h2>
                    </div>
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <button onclick={on_create_new.clone()}>{"Create New Project"}</button>
                    </div>
                </div>
                    <div class="projects-list" style="display: flex; flex-direction: column; gap: 12px; margin-top: 16px;">
                        { for project_list.iter().map(|project| {
                            return html! {
                                <ProjectListingItem project={project.clone()} user_email={user_email.clone()} />
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
