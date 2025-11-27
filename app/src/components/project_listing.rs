use crate::components::create_project_modal::CreateProjectModal;
use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;
use common::api_types::ProjectInfo;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectListingProps {
    #[prop_or_default]
    pub on_select: Callback<ProjectInfo>,
    #[prop_or(0)]
    pub refresh_trigger: u32,
}

#[function_component(ProjectListing)]
pub fn project_listing(props: &ProjectListingProps) -> Html {
    let projects: UseStateHandle<Vec<ProjectInfo>> = use_state(Vec::new);
    let loading = use_state(|| true);
    let error = use_state(|| None::<String>);
    let show_create_modal = use_state(|| false);

    // Fetch projects on mount and when refresh_trigger changes
    {
        let projects = projects.clone();
        let loading = loading.clone();
        let error = error.clone();
        let refresh_trigger = props.refresh_trigger;
        use_effect_with(refresh_trigger, move |_| {
            // Reset loading state when refresh is triggered
            loading.set(true);
            error.set(None);

            spawn_local(async move {
                match SPELEO_DB_CONTROLLER.fetch_projects().await {
                    Ok(project_list) => {
                        projects.set(project_list);
                        loading.set(false);
                    }
                    Err(e) => {
                        error.set(Some(e));
                        loading.set(false);
                    }
                }
            });
            || ()
        });
    }

    // Button handlers
    let on_create_new = {
        let show_create_modal = show_create_modal.clone();
        Callback::from(move |_| {
            show_create_modal.set(true);
        })
    };

    let on_refresh = {
        let projects = projects.clone();
        let loading = loading.clone();
        let error = error.clone();
        Callback::from(move |_| {
            let projects = projects.clone();
            let loading = loading.clone();
            let error = error.clone();
            loading.set(true);
            error.set(None);
            spawn_local(async move {
                match SPELEO_DB_CONTROLLER.fetch_projects().await {
                    Ok(project_list) => {
                        projects.set(project_list);
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
        let projects = projects.clone();
        let loading = loading.clone();
        let error = error.clone();
        let on_select = props.on_select.clone();

        Callback::from(move |new_project: ProjectInfo| {
            show_create_modal.set(false);

            // Refresh the project list
            let projects = projects.clone();
            let loading = loading.clone();
            let error = error.clone();
            let on_select = on_select.clone();
            let new_project = new_project.clone();

            loading.set(true);
            error.set(None);

            spawn_local(async move {
                match SPELEO_DB_CONTROLLER.fetch_projects().await {
                    Ok(project_list) => {
                        projects.set(project_list);
                        loading.set(false);

                        // Only open the new project AFTER the refresh completes
                        on_select.emit(new_project);
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
    if *loading {
        html! {
            <>
                <section style="width:100%;">
                    <h2>{"Project Listing"}</h2>
                    <div style="display: flex; justify-content: center; gap: 12px; margin-bottom: 16px;">
                        <button disabled={true}>{"Create New Project"}</button>
                        <button disabled={true}>{"Refresh Projects"}</button>
                    </div>
                    <p>{"Loading projects..."}</p>
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
    } else if let Some(err_msg) = &*error {
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
                        { for projects.iter().map(|project| {
                            let project_clone = project.clone();
                            let on_select = props.on_select.clone();
                            let on_card_click = Callback::from(move |_| {
                                on_select.emit(project_clone.clone());
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
