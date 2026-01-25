use common::ui_state::UiState;
use yew::{Html, Properties, classes, function_component, html, use_state};

use crate::components::{project_details::ProjectDetails, project_listing::ProjectListing};

#[derive(Properties, PartialEq)]
pub struct MainLayoutProps {
    pub ui_state: UiState,
}

#[function_component(MainLayout)]
pub fn main_layout(&MainLayoutProps { ref ui_state }: &MainLayoutProps) -> Html {
    // Disconnect handler: clear OAuth token in prefs and form, reset UI to login

    let selected_project_info = if let Some(email) = &ui_state.user_email
        && let Some(selected_project_id) = ui_state.selected_project_id
    {
        let selected_project = ui_state
            .project_status
            .iter()
            .find(|p| (*p).id() == selected_project_id)
            .unwrap();
        Some((selected_project.clone(), email.to_string()))
    } else {
        None
    };

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
            <section style="width:100%;">
                {
                    if let Some((selected_project, email)) = selected_project_info {
                        html!{ <ProjectDetails project={selected_project} user_email={email} compass_open={ui_state.compass_open} /> }
                    } else {
                        html!{ <ProjectListing  ui_state={ui_state.clone()}/> }
                    }
                }
            </section>
        </main>
    };
}
