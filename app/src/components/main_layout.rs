use common::UiState;
use log::{error, info};
use wasm_bindgen_futures::spawn_local;
use yew::{Callback, Html, Properties, function_component, html};

use crate::{
    components::{project_details::ProjectDetails, project_listing::ProjectListing},
    speleo_db_controller::SPELEO_DB_CONTROLLER,
};

#[derive(Properties, PartialEq)]
pub struct MainLayoutProps {
    pub ui_state: UiState,
}

#[function_component(MainLayout)]
pub fn main_layout(&MainLayoutProps { ref ui_state }: &MainLayoutProps) -> Html {
    // Disconnect handler: clear OAuth token in prefs and form, reset UI to login
    let on_disconnect = {
        Callback::from(move |_| {
            spawn_local(async move {
                info!("Signing out...");
                match SPELEO_DB_CONTROLLER.sign_out().await {
                    Ok(_) => {
                        info!("Signed out successfully.");
                    }
                    Err(e) => {
                        error!("Error signing out: {}", e);
                    }
                }
            });
        })
    };
    let ui_state = ui_state.clone();
    return html! {
        <main class="container">
            <header style="display:flex; justify-content:space-between; align-items:center; margin-bottom:24px; width:100%;">
                <h1>{"SpeleoDB - Compass Sidecar"}</h1>
                <div style="gap:8px;">
                    <button style="background-color:red; color:white;" onclick={on_disconnect.clone()}>{ "Sign Out" }</button>
                </div>
            </header>
            <section style="width:100%;">
                {
                    if let Some(selected_project) = &ui_state.selected_project {
                        let selected_project = ui_state.project_info.iter().find(|p| p.id == *selected_project).unwrap();
                        html!{ <ProjectDetails project={selected_project.clone()} /> }
                    } else {
                        html!{ <ProjectListing  ui_state={ui_state}/> }
                    }
                }
            </section>
        </main>
    };
}
