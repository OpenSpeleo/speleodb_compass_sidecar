use common::ui_state::UiState;
use log::{error, info};
use wasm_bindgen_futures::spawn_local;
use yew::{Callback, Html, Properties, function_component, html};

use crate::{
    components::{
        project_details::ProjectDetails, project_listing::ProjectListing,
        project_listing_item::_ProjectListingItemProps::user_email,
    },
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
            <header style="display:flex; justify-content:space-between; align-items:center;flex-direction: column; margin-bottom:24px; width:100%;">
                <div style="display: flex-row">
                    <div style="gap:8px;">
                        <button style="background-color:red; color:white;" onclick={on_disconnect.clone()}>{ "Sign Out" }</button>
                    </div>
                </div>
                <h1>{"SpeleoDB - Compass Sidecar"}</h1>

            </header>
            <section>
                {
                    if let Some(selected_project) = &ui_state.selected_project {
                        let email:String = ui_state.user_email.unwrap_or_default().to_string();
                        let selected_project = ui_state.project_status.iter().find(|p| (*p).id() == *selected_project).unwrap();
                        html!{ <ProjectDetails  user_email={email} project={selected_project.clone()} /> }
                    } else {
                        html!{ <ProjectListing  ui_state={ui_state}/> }
                    }
                }
            </section>
        </main>
    };
}
