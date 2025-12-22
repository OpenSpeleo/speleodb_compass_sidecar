use crate::{
    components::{
        auth_screen::AuthScreen, loading_screen::LoadingScreen, project_details::ProjectDetails,
        project_listing::ProjectListing,
    },
    speleo_db_controller::{SPELEO_DB_CONTROLLER, SpeleoDBController},
};
use common::{LoadingState, UI_STATE_NOTIFICATION_KEY, UiState, api_types::ProjectInfo};
use futures::StreamExt;
use log::{error, info};
use tauri_sys::event::listen;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum ActiveTab {
    Listing,
    Details,
}

#[function_component(App)]
pub fn app() -> Html {
    // UI state
    let ui_state = use_state(|| UiState::default());
    let show_error = use_state(|| false);
    let error_msg = use_state(String::new);
    let active_tab = use_state(|| ActiveTab::Listing);
    let selected_project: UseStateHandle<Option<ProjectInfo>> = use_state(|| None);
    let refresh_trigger = use_state(|| 0u32);

    let loading_state = (*ui_state).loading_state.clone();
    use_effect(move || {
        if let LoadingState::NotStarted = loading_state {
            spawn_local(async { SPELEO_DB_CONTROLLER.ensure_initialized().await });
        }
    });

    let ui_state_clone = ui_state.clone();
    spawn_local(async move {
        let mut booted_stream = listen::<UiState>(UI_STATE_NOTIFICATION_KEY).await.unwrap();
        while let Some(event) = booted_stream.next().await {
            let updated_ui_state = event.payload;
            info!("ui_state : {:?}", updated_ui_state);
            ui_state_clone.set(updated_ui_state);
        }
    });

    // Disconnect handler: clear OAuth token in prefs and form, reset UI to login
    let on_disconnect = {
        let error_msg = error_msg.clone();
        let show_error = show_error.clone();
        Callback::from(move |_| {
            let error_msg = error_msg.clone();
            let show_error = show_error.clone();
            spawn_local(async move {
                info!("Signing out...");
                match SPELEO_DB_CONTROLLER.sign_out().await {
                    Ok(_) => {
                        info!("Signed out successfully.");
                    }
                    Err(e) => {
                        error!("Error signing out: {}", e);
                        error_msg.set(format!("Error signing out: {}", e));
                        show_error.set(true);
                    }
                }
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

    let loading_state = (*ui_state).loading_state.clone();
    match loading_state {
        LoadingState::Ready => {
            return html! {
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
            };
        }
        LoadingState::Unauthenticated => {
            return html! {
               <AuthScreen/>
            };
        }
        // All other states occur on the loading screen
        _ => {
            return html! {
                 <LoadingScreen loading_state={loading_state}/>
            };
        }
    }
}
