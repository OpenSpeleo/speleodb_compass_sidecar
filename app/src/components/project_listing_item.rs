use common::ui_state::ProjectStatus;
use wasm_bindgen_futures::spawn_local;
use yew::{Callback, Html, Properties, function_component, html};
use yew_icons::{Icon, IconData};

use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;

#[derive(Properties, PartialEq)]
pub struct ProjectListingItemProps {
    pub project: ProjectStatus,
}

#[function_component(ProjectListingItem)]
pub fn project_listing_item_layout(
    ProjectListingItemProps { project }: &ProjectListingItemProps,
) -> Html {
    let project_id = project.id();
    let project_status = project.local_status().clone();
    let on_card_click = Callback::from(move |_| {
        spawn_local(async move {
            SPELEO_DB_CONTROLLER
                .set_active_project(project_id)
                .await
                .unwrap();
        });
    });

    let is_locked = project.active_mutex().is_some();
    let lock_status = if let Some(mutex) = project.active_mutex() {
        &format!("🔒 {}", mutex.user)
    } else {
        "🔓 editable"
    };
    let lock_color = if is_locked { "#ff6b6b" } else { "#51cf66" };
    return html! {
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
                 "
            }
        >
            <div style="display: flex; gap: 12px; align-items: center; color:#2c3e50;">
                <h3 style="margin: 0; font-size: 18px; color: #2c3e50;">
                    { project.name() }
                </h3>
                {match project_status {
                    common::ui_state::LocalProjectStatus::UpToDate=>html!{<Icon data={IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK}/>},
                    common::ui_state::LocalProjectStatus::Dirty=>html!{<Icon data={IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_EXCLAMATION}/>},
                    common::ui_state::LocalProjectStatus::RemoteOnly=>html!{<Icon data={IconData::FONT_AWESOME_SOLID_FILE_ARROW_DOWN}/>},
                    common::ui_state::LocalProjectStatus::Unknown => html!{<Icon data={IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK}/>},
                    common::ui_state::LocalProjectStatus::EmptyLocal =>html!{<Icon data={IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_PLUS}/>},
                    common::ui_state::LocalProjectStatus::OutOfDate => html!{<Icon data={IconData::FONT_AWESOME_SOLID_FILE_ARROW_DOWN}/>},
                    common::ui_state::LocalProjectStatus::DirtyAndOutOfDate => html!{<Icon data={IconData::FONT_AWESOME_SOLID_FACE_SAD_CRY}/>},
                }}
                <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;", lock_color)}>
                    { format!("{project_status:?}") }
                </span>
                <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;",
                if project.permission() == "ADMIN" { " #ff7f00" } else if project.permission() == "READ_AND_WRITE" { "#228be6" } else { "#868e96" }
                )}>
                    { project.permission() }
                </span>
                <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;", lock_color)}>
                    { lock_status }
                </span>
            </div>
        </div>
    };
}
