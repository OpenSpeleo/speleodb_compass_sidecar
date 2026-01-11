use common::ui_state::ProjectStatus;
use wasm_bindgen_futures::spawn_local;
use yew::{Callback, Html, Properties, classes, function_component, html};
use yew_icons::{Icon, IconData};

use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;

#[derive(Properties, PartialEq)]
pub struct ProjectListingItemProps {
    pub project: ProjectStatus,
    pub user_email: String,
}

#[function_component(ProjectListingItem)]
pub fn project_listing_item_layout(
    ProjectListingItemProps {
        project,
        user_email,
    }: &ProjectListingItemProps,
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
    let project_permission = project.permission();
    let permission_color = match project_permission {
        "ADMIN" => "#ff7f00",
        "READ_AND_WRITE" => "#228be6",
        _ => "#868e96",
    };
    let lock_color;
    let lock_status = if let Some(mutex) = project.active_mutex() {
        if &mutex.user == user_email {
            lock_color = "#ff7f00";
            //return user locked version
            "🔒 by me"
        } else {
            lock_color = "#ff6b6b";
            // locked by other user
            &format!("🔒 by {}", mutex.user)
        }
    } else {
        lock_color = "#51cf66";
        "🔓 editable"
    };
    // let lock_color = if is_locked { "#ff6b6b" } else { "#51cf66" };
    fn truncate_str_by_chars(s: &str, max_chars: usize) -> &str {
        match s.char_indices().nth(max_chars) {
            None => s,                   // The string is shorter than the max length
            Some((idx, _)) => &s[..idx], // Truncate at the byte index of the Nth char
        }
    }
    let truncated_name = truncate_str_by_chars(project.name(), 25).to_string();
    let icon_data = match project_status {
        common::ui_state::LocalProjectStatus::UpToDate => {
            IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK
        }
        common::ui_state::LocalProjectStatus::Dirty => {
            IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_EXCLAMATION
        }
        common::ui_state::LocalProjectStatus::RemoteOnly => {
            IconData::FONT_AWESOME_SOLID_FILE_ARROW_DOWN
        }
        common::ui_state::LocalProjectStatus::Unknown => {
            IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK
        }
        common::ui_state::LocalProjectStatus::EmptyLocal => {
            IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_PLUS
        }
        common::ui_state::LocalProjectStatus::OutOfDate => {
            IconData::FONT_AWESOME_SOLID_FILE_ARROW_DOWN
        }
        common::ui_state::LocalProjectStatus::DirtyAndOutOfDate => {
            IconData::FONT_AWESOME_SOLID_FACE_SAD_CRY
        }
    };
    return html! {
        <div class={classes!("project-card")} onclick={on_card_click}>
            <span style="padding: 4px 8px; border-radius: 4px; font-size: 12px; display:flex; background-color; blue; color: #2c3e50; gap: 12px">
                <Icon data={icon_data} style="font-size: 12px;"></Icon>
                <h3 class={classes!("vertically-centered-text")} style="margin: 0; font-size: 16px; ">
                    { truncated_name }
                </h3>
            </span>
            <div style="display: flex; gap: 12px">
                <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;",
                    permission_color
                )}>
                    { project_permission }
                </span>
                <span style={format!("padding: 4px 8px; border-radius: 4px; background-color: {}; color: white; font-size: 12px; font-weight: bold;", lock_color)}>
                    { lock_status }
                </span>
            </div>
        </div>
    };
}
