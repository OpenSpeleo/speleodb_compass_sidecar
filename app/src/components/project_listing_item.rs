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
    let color_warn = "#ff7f00";
    let color_alarm = "#ff6b6b";
    let color_good = "#51cf66";
    let color_blue = "#228be6";
    let color_grey = "#868e96";
    let font_color_blue = "#2c3e50";

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
        "ADMIN" => color_warn,
        "READ_AND_WRITE" => color_blue,
        _ => color_grey,
    };
    let lock_color;
    let lock_status = if let Some(mutex) = project.active_mutex() {
        if &mutex.user == user_email {
            lock_color = color_warn;
            //return user locked version
            "🔒 by me"
        } else {
            lock_color = color_alarm;
            // locked by other user
            &format!("🔒 by {}", mutex.user)
        }
    } else {
        lock_color = color_good;
        "🔓 editable"
    };
    fn truncate_str_by_chars(s: &str, max_chars: usize) -> &str {
        match s.char_indices().nth(max_chars) {
            None => s,                   // The string is shorter than the max length
            Some((idx, _)) => &s[..idx], // Truncate at the byte index of the Nth char
        }
    }
    let truncated_name = truncate_str_by_chars(project.name(), 25).to_string();
    let mut icon_color = color_good;
    let icon_data = match project_status {
        common::ui_state::LocalProjectStatus::UpToDate => {
            IconData::FONT_AWESOME_SOLID_FILE_CIRCLE_CHECK
        }
        common::ui_state::LocalProjectStatus::Dirty => {
            icon_color = color_warn;
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
            icon_color = font_color_blue;
            IconData::FONT_AWESOME_SOLID_FILE_ARROW_DOWN
        }
        common::ui_state::LocalProjectStatus::DirtyAndOutOfDate => {
            icon_color = color_alarm;
            IconData::FONT_AWESOME_SOLID_FACE_SAD_CRY
        }
    };
    return html! {
        <div class={classes!("project-card")} onclick={on_card_click}>
            <span style={format!("padding: 4px 8px; border-radius: 4px; font-size: 12px; display:flex; gap: 12px; color: {};",icon_color)}>
                <Icon data={icon_data}></Icon>
                <h3 class={classes!("vertically-centered-text")} style={format!("margin: 0; font-size: 16px; color: {};",font_color_blue)}>
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
