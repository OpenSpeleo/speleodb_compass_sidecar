//! The project details component displays information about a specific project,
//! allows users to open the project in compass(Or just the folder on non-windows platforms),
//!  commit changes with a message, and jump back to the project list.
//! TODO:
//! [ ] Only enable back button if up-to-date (not dirty) or in read-only mode
//! [ ] Show project_status indicating local change (whether read only or not, whether up to date or not, )
//! [ ] Only show commit section if user has write access and local changes are present
//! [ ] Investigate making files read-only when in read-only mode
//! [ ] Show whether Compass is being tracked open on Windows

use crate::components::modal::{Modal, ModalType};
use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;
use common::api_types::ProjectSaveResult;
use common::ui_state::{LocalProjectStatus, UiState};
use log::info;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectDetailsProps {
    pub ui_state: UiState,
}

#[function_component(ProjectDetails)]
pub fn project_details(&ProjectDetailsProps { ref ui_state }: &ProjectDetailsProps) -> Html {
    let selected_project_id = ui_state.selected_project_id.unwrap();
    let selected_project = ui_state
        .project_status
        .iter()
        .find(|p| (*p).id() == selected_project_id)
        .unwrap();
    let is_dirty = use_state(|| selected_project.is_dirty());
    let initialized = use_state(|| false);

    let downloading = use_state(|| false);
    let uploading = use_state(|| false);
    let show_readonly_modal = use_state(|| false);
    let show_success_modal = use_state(|| false);
    let show_reload_confirm = use_state(|| false);
    let show_upload_success = use_state(|| false);
    let show_no_changes_modal = use_state(|| false);
    let show_empty_project_modal = use_state(|| false);
    let error_message: UseStateHandle<Option<String>> = use_state(|| None);
    let upload_error: UseStateHandle<Option<String>> = use_state(|| None);
    let is_readonly = use_state(|| false);
    let download_complete = use_state(|| false);
    let commit_message = use_state(String::new);
    let commit_message_error = use_state(|| false);

    // On mount: Check if we need to show any modals based on project status
    if !*initialized {
        // On mount, check if the project is read-only
        if selected_project.active_mutex().is_some()
            && &selected_project.active_mutex().as_ref().unwrap().user
                != ui_state.user_email.as_ref().unwrap()
            || selected_project.permission() == "READ_ONLY"
        {
            is_readonly.set(true);
            show_readonly_modal.set(true);
        } else if let LocalProjectStatus::EmptyLocal = selected_project.local_status() {
            show_empty_project_modal.set(true);
        }
        initialized.set(true);
    }

    // Close readonly modal and show success modal if download is complete
    let close_readonly_modal = {
        let show_readonly_modal = show_readonly_modal.clone();
        let show_success_modal = show_success_modal.clone();
        let download_complete = download_complete.clone();
        Callback::from(move |_| {
            show_readonly_modal.set(false);
            if *download_complete {
                show_success_modal.set(true);
            }
        })
    };

    // Effect to show success modal immediately if there was no readonly modal
    {
        let show_success_modal = show_success_modal.clone();
        let download_complete = download_complete.clone();
        let is_readonly = is_readonly.clone();
        let show_readonly_modal = show_readonly_modal.clone();

        use_effect_with(download_complete.clone(), move |complete| {
            if **complete && !*is_readonly && !*show_readonly_modal {
                show_success_modal.set(true);
            }
            || ()
        });
    }

    // Close success modal
    let close_success_modal = {
        let show_success_modal = show_success_modal.clone();
        Callback::from(move |_| {
            show_success_modal.set(false);
        })
    };

    // Open folder handler
    let on_open_project = {
        let project_id = selected_project.id();
        Callback::from(move |_: ()| {
            spawn_local(async move {
                let _ = SPELEO_DB_CONTROLLER.open_project(project_id).await;
            });
        })
    };

    // Commit message handler
    let onchange_message = {
        let commit_message = commit_message.clone();
        let commit_message_error = commit_message_error.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            commit_message.set(input.value());
            if !input.value().trim().is_empty() {
                commit_message_error.set(false);
            }
        })
    };

    // Save Project Handler
    let on_save = {
        let project_id = selected_project.id();
        let commit_message = commit_message.clone();
        let commit_message_error = commit_message_error.clone();
        let uploading = uploading.clone();
        let show_upload_success = show_upload_success.clone();
        let show_no_changes_modal = show_no_changes_modal.clone();
        let upload_error = upload_error.clone();

        Callback::from(move |_| {
            info!(
                "Saving project {} with commit message: {}",
                project_id,
                (*commit_message).clone()
            );
            let msg = (*commit_message).clone();

            if msg.trim().is_empty() {
                commit_message_error.set(true);
                return;
            }

            let uploading = uploading.clone();
            let show_upload_success = show_upload_success.clone();
            let show_no_changes_modal = show_no_changes_modal.clone();
            let upload_error = upload_error.clone();

            uploading.set(true);
            upload_error.set(None);

            spawn_local(async move {
                // 1. ZIP project
                uploading.set(true);
                match SPELEO_DB_CONTROLLER.save_project(project_id, &msg).await {
                    Ok(upload_result) => match upload_result {
                        ProjectSaveResult::NoChanges => {
                            show_no_changes_modal.set(true);
                        }
                        ProjectSaveResult::Saved => {
                            show_upload_success.set(true);
                        }
                    },
                    Err(e) => {
                        upload_error.set(Some(format!("Failed to zip project: {}", e)));
                        uploading.set(false);
                        return;
                    }
                };
                uploading.set(false);
            });
        })
    };

    // Load from Disk Handler
    let on_import_from_disk = {
        let show_empty_project_modal = show_empty_project_modal.clone();
        let error_message = error_message.clone();
        let project_id = selected_project.id();
        Callback::from(move |_: ()| {
            let show_empty_project_modal = show_empty_project_modal.clone();
            let error_message = error_message.clone();
            spawn_local(async move {
                match SPELEO_DB_CONTROLLER
                    .import_compass_project(project_id)
                    .await
                {
                    Ok(()) => {
                        show_empty_project_modal.set(false);
                    }
                    Err(e) => {
                        show_empty_project_modal.set(false);
                        error_message.set(Some(format!("Failed to select file: {}", e)));
                    }
                }
            });
        })
    };

    // Back button handler - release mutex before navigating back
    let on_back_click = {
        Callback::from(move |_| {
            spawn_local(async move {
                // Release mutex and clear active project first
                let _ = SPELEO_DB_CONTROLLER.clear_active_project().await;
            });
        })
    };

    html! {
        <section style="width:100%;">
            <div style="width: 100%; margin-bottom: 16px; display: flex; justify-content: space-between; align-items: center;">
                <button style="background-color: #10b981; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: 500;opacity: disabled ? 0.5 : 1;" onclick={on_back_click} disabled={*is_dirty}>{"← Back to Projects"}</button>
                <button
                    onclick={on_open_project.reform(|_| ())}
                    style=" color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: 500;"
                >
                    {"Open in Compass"}
                </button>
            </div>

            <h2><strong>{"Project: "}</strong>{&selected_project.name()}</h2>
            <p style="color: #6b7280; font-size: 14px;">{format!("ID: {}", selected_project.id())}</p>

            {
                if *downloading {
                    html! {
                        <div style="
                            padding: 24px;
                            text-align: center;
                            background-color: #f3f4f6;
                            border-radius: 8px;
                            margin: 20px 0;
                        ">
                            <div style="
                                border: 4px solid #e5e7eb;
                                border-top-color: #3b82f6;
                                border-radius: 50%;
                                width: 48px;
                                height: 48px;
                                animation: spin 1s linear infinite;
                                margin: 0 auto 16px;
                            " />
                            <p style="color: #4b5563; font-size: 16px;">
                                {"Downloading and extracting project..."}
                            </p>
                        </div>
                    }
                } else if let Some(err) = &*error_message {
                    html! {
                        <div style="
                            padding: 16px;
                            background-color: #fee2e2;
                            border: 1px solid #ef4444;
                            border-radius: 8px;
                            margin: 20px 0;
                        ">
                            <strong style="color: #dc2626;">{"Error: "}</strong>
                            <span style="color: #991b1b;">{err}</span>
                        </div>
                    }
                } else {
                    html! {}
                }
            }

            {
                if *is_readonly {
                    html! {
                        <div style="
                            padding: 12px 16px;
                            background-color: #fef3c7;
                            border-left: 4px solid #f59e0b;
                            border-radius: 4px;
                            margin: 20px 0;
                        ">
                            <strong style="color: #92400e;">{"⚠️ Read-Only Mode"}</strong>
                            <p style="color: #78350f; margin-top: 4px; font-size: 14px;">
                                {"This project is opened in read-only mode. Modifications cannot be saved."}
                            </p>
                        </div>
                    }
                } else {
                    if *is_dirty {
                    // Commit Section (Only if write access)
                        html! {
                            <div style="margin-top: 24px; padding-top: 24px; border-top: 1px solid #e5e7eb;">
                                <h3 style="margin-bottom: 12px;">{"📝 Commit Changes"}</h3>
                                <div style="margin-bottom: 16px;
                                display: flex; flex-direction: column;">
                                    <textarea
                                        rows="4"
                                        type="text"
                                        value={(*commit_message).clone()}
                                        oninput={onchange_message}
                                        placeholder="Describe your changes (max 255 characters)"
                                        maxlength="255"
                                        style={format!(
                                            "max-width: 100%; flex: 1; padding: 8px; border: 1px solid {}; border-radius: 4px; font-family: inherit;",
                                            if *commit_message_error { "#ef4444" } else { "#d1d5db" }
                                        )}
                                    />
                                    {
                                        if *commit_message_error {
                                            html! {
                                                <div>
                                                <p style="color: #ef4444; font-size: 12px; margin-top: 4px;">
                                                    {"Please, enter a commit message."}
                                                </p></div>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                                <div style="display: flex; gap: 12px; flex-wrap: wrap; justify-content: center;">
                                    <button
                                        onclick={on_save}
                                        disabled={ !*is_dirty || *uploading}
                                        style="background-color: #2563eb; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; opacity: disabled ? 0.5 : 1;"
                                    >
                                        {if *uploading { "Saving..." } else { "Save Project" }}
                                    </button>
                                    <button
                                        onclick={
                                            let show_reload_confirm = show_reload_confirm.clone();
                                            move |_| show_reload_confirm.set(true)
                                        }
                                        style="background-color: #f3f4f6; color: #1f2937; border: 1px solid #d1d5db; padding: 8px 16px; border-radius: 4px; cursor: pointer;opacity: disabled ? 0.5 : 1;"
                                    >
                                        {"Reload Project"}
                                    </button>
                                </div>
                                {
                                    if let Some(err) = &*upload_error {
                                        html! {
                                            <div style="margin-top: 12px; color: #dc2626; font-size: 14px; text-align: center;">
                                                {format!("Error: {}", err)}
                                            </div>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                        }
                    } else {
                        html!{<></>}
                    }
                }
            }
            {
            if *show_readonly_modal {
                return html! {
                    <Modal
                        title="Read-Only Access"
                        message={format!(
                            "The project '{}' was opened in READ-ONLY mode.\n\n\
                            Modifications to this project cannot be saved because \n
                            - the project is currently locked by another user, or
                            - you do not have permission to edit the project\n\n\
                            Contact a Project Administrator if you believe this is a mistake.",
                            selected_project.name()
                        )}
                        modal_type={ModalType::Warning}
                        show_close_button={true}
                        on_close={close_readonly_modal}
                    />
                };
            } else if *show_empty_project_modal {
                let on_import_from_disk = on_import_from_disk.clone();
                html! {
                    <Modal
                        title="Empty Project"
                        message="This project contains no Compass data yet.\n\nTo initialize the project, use the 'Import from Disk' button to upload your local project files."
                        modal_type={ModalType::Info}
                        primary_button_text="Import Compass Project From Disk"
                        on_primary_action={on_import_from_disk}
                    />
                }
            } else {
                html! {}
            }
                    }

            // Success modal (Download)
            {
                if *show_success_modal {
                    html! {
                        <Modal
                            title="Project Downloaded Successfully"
                            message="The project has been successfully downloaded and extracted. \
                                Click 'Open Folder' to view the project files in your file explorer."
                            modal_type={ModalType::Success}
                            show_close_button={true}
                            primary_button_text={Some("Open Folder".to_string())}
                            on_close={close_success_modal}
                            on_primary_action={
                                let on_open_folder = on_open_project.clone();
                                let show_success_modal = show_success_modal.clone();
                                Callback::from(move |_| {
                                    on_open_folder.emit(());
                                    show_success_modal.set(false);
                                })
                            }
                        />
                    }
                } else {
                    html! {}
                }
            }

            // Upload Success Modal
            {
                if *show_upload_success {
                    html! {
                        <Modal
                            title="Project Saved Successfully"
                            message="Your changes have been successfully committed and uploaded to the server."
                            modal_type={ModalType::Success}
                            show_close_button={true}
                            on_close={move |_| show_upload_success.set(false)}
                        />
                    }
                } else {
                    html! {}
                }
            }

            // No Changes Modal (304)
            {
                if *show_no_changes_modal {
                    html! {
                        <Modal
                            title="No Changes Detected"
                            message="The project on the server is already identical to your local version. \n\nNo changes were saved."
                            modal_type={ModalType::Warning}
                            show_close_button={true}
                            on_close={move |_| show_no_changes_modal.set(false)}
                        />
                    }
                } else {
                    html! {}
                }
            }


            // Add CSS for spinner animation
            <style>
                {r#"
                    @keyframes spin {
                        0% { transform: rotate(0deg); }
                        100% { transform: rotate(360deg); }
                    }
                "#}
            </style>
        </section>
    }
}
