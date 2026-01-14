use common::ui_state::UiState;
use yew::{Html, Properties, classes, function_component, html};

use crate::components::{project_details::ProjectDetails, project_listing::ProjectListing};

#[derive(Properties, PartialEq)]
pub struct MainLayoutProps {
    pub ui_state: UiState,
}

#[function_component(MainLayout)]
pub fn main_layout(&MainLayoutProps { ref ui_state }: &MainLayoutProps) -> Html {
    // Disconnect handler: clear OAuth token in prefs and form, reset UI to login

    let ui_state = ui_state.clone();
    return html! {
        <main class="container">
            <header style="display:flex;
                justify-content:space-around;
                align-items:center;flex-direction: row;
                margin-bottom:24px;
                width:96vw;
            ">
                <div>
                    <h1 class={classes!("vertically-centered-text")} >{"SpeleoDB Compass Sidecar"}</h1>
                </div>
            </header>
            <section>
                {
                    if let Some(_selected_project_id) = &ui_state.selected_project_id {
                        // let email:String = ui_state.user_email.unwrap_or_default().to_string();
                        // let selected_project = ui_state.project_status.iter().find(|p| (*p).id() == *selected_project_id).unwrap();
                        html!{ <ProjectDetails ui_state={ui_state} /> }
                    } else {
                        html!{ <ProjectListing  ui_state={ui_state}/> }
                    }
                }
            </section>
        </main>
    };
}
