use crate::components::modal::{Modal, ModalType};
use crate::speleo_db_controller::SPELEO_DB_CONTROLLER;
use speleodb_compass_common::ProjectMetadata;
use speleodb_compass_common::api_types::ProjectInfo;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectDetailsProps {
    pub project: ProjectInfo,
    #[prop_or_default]
    pub on_back: Callback<()>,
}

#[function_component(ProjectDetails)]
pub fn project_details(props: &ProjectDetailsProps) -> Html {
    let downloading = use_state(|| false);
    let uploading = use_state(|| false);
    let show_readonly_modal = use_state(|| false);
    let show_success_modal = use_state(|| false);
    let show_reload_confirm = use_state(|| false);
    let show_load_confirm = use_state(|| false);
    let show_upload_success = use_state(|| false);
    let show_no_changes_modal = use_state(|| false);
    let show_empty_project_modal = use_state(|| false);
    let error_message: UseStateHandle<Option<String>> = use_state(|| None);
    let upload_error: UseStateHandle<Option<String>> = use_state(|| None);
    let project_folder_path: UseStateHandle<Option<String>> = use_state(|| None);
    let is_readonly = use_state(|| false);
    let download_complete = use_state(|| false);
    let commit_message = use_state(String::new);
    let commit_message_error = use_state(|| false);
    let selected_zip: UseStateHandle<Option<String>> = use_state(|| None);

    // Run the download workflow automatically on mount
    {
        let project_id = props.project.id.clone();
        let downloading = downloading.clone();
        let show_readonly_modal = show_readonly_modal.clone();
        let show_empty_project_modal_effect = show_empty_project_modal.clone();
        let error_message = error_message.clone();
        let project_folder_path = project_folder_path.clone();
        let is_readonly = is_readonly.clone();
        let download_complete = download_complete.clone();

        use_effect_with((), move |_| {
            let project_id = project_id.clone();
            spawn_local(async move {
                downloading.set(true);

                // Step 1: Try to acquire project mutex
                match SPELEO_DB_CONTROLLER
                    .acquire_project_mutex(&project_id)
                    .await
                {
                    Ok(()) => {
                        // Mutex acquired! Set active project for shutdown hook
                        let _ = SPELEO_DB_CONTROLLER.set_active_project(&project_id).await;
                    }
                    Err(_e) => {
                        // Mutex acquisition had an error, but we continue anyway
                        is_readonly.set(true);
                        show_readonly_modal.set(true);
                    }
                }

                // Step 2: Download the project (regardless of mutex status)
                let zip_result = SPELEO_DB_CONTROLLER.download_project(&project_id).await;

                let zip_path = match zip_result {
                    Ok(path) => path,
                    Err(e) => {
                        // Check if this is an empty project error (422)
                        if e == "EMPTY_PROJECT_422" {
                            // Empty project - show info modal instead of error
                            downloading.set(false);
                            show_empty_project_modal_effect.set(true);
                            return;
                        }
                        error_message.set(Some(format!("Download failed: {}", e)));
                        downloading.set(false);
                        return;
                    }
                };

                // Step 3: Unzip the project
                let folder_path = match SPELEO_DB_CONTROLLER
                    .unzip_project(&zip_path, &project_id)
                    .await
                {
                    Ok(path) => path,
                    Err(e) => {
                        error_message.set(Some(format!("Failed to extract project: {}", e)));
                        downloading.set(false);
                        return;
                    }
                };

                // Step 4: Success!
                project_folder_path.set(Some(folder_path));
                downloading.set(false);
                download_complete.set(true);
            });

            // Cleanup: Clear active project when component unmounts
            move || {
                spawn_local(async move {
                    // Only clear active project; mutex already released via back button
                    let _ = SPELEO_DB_CONTROLLER.clear_active_project().await;
                });
            }
        });
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
    let on_open_folder = {
        let project_id = props.project.id.clone();
        Callback::from(move |_: ()| {
            let project_id = project_id.clone();
            spawn_local(async move {
                let _ = SPELEO_DB_CONTROLLER.open_folder(&project_id).await;
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
        let project_id = props.project.id.clone();
        let commit_message = commit_message.clone();
        let commit_message_error = commit_message_error.clone();
        let uploading = uploading.clone();
        let show_upload_success = show_upload_success.clone();
        let show_no_changes_modal = show_no_changes_modal.clone();
        let upload_error = upload_error.clone();

        Callback::from(move |_| {
            let project_id = project_id.clone();
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
                let zip_path = match SPELEO_DB_CONTROLLER.zip_project(&project_id).await {
                    Ok(p) => p,
                    Err(e) => {
                        upload_error.set(Some(format!("Failed to zip project: {}", e)));
                        uploading.set(false);
                        return;
                    }
                };

                // 2. Upload
                match SPELEO_DB_CONTROLLER
                    .upload_project(&project_id, &msg, &zip_path)
                    .await
                {
                    Ok(status) => {
                        if status == 304 {
                            show_no_changes_modal.set(true);
                        } else {
                            show_upload_success.set(true);
                        }
                    }
                    Err(e) => upload_error.set(Some(format!("Upload failed: {}", e))),
                }

                uploading.set(false);
            });
        })
    };

    // Reload Project Handler
    let on_confirm_reload = {
        let project_id = props.project.id.clone();
        let downloading = downloading.clone();
        let show_reload_confirm = show_reload_confirm.clone();
        let error_message = error_message.clone();
        let show_success_modal = show_success_modal.clone();

        Callback::from(move |_: ()| {
            let project_id = project_id.clone();
            let downloading = downloading.clone();
            let show_reload_confirm = show_reload_confirm.clone();
            let error_message = error_message.clone();
            let show_success_modal = show_success_modal.clone();

            show_reload_confirm.set(false);
            downloading.set(true);
            error_message.set(None);

            spawn_local(async move {
                let zip_path = match SPELEO_DB_CONTROLLER.download_project(&project_id).await {
                    Ok(path) => path,
                    Err(e) => {
                        error_message.set(Some(format!("Download failed: {}", e)));
                        downloading.set(false);
                        return;
                    }
                };

                match SPELEO_DB_CONTROLLER
                    .unzip_project(&zip_path, &project_id)
                    .await
                {
                    Ok(_) => {
                        downloading.set(false);
                        show_success_modal.set(true);
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Failed to extract project: {}", e)));
                        downloading.set(false);
                    }
                };
            });
        })
    };

    // Load from Disk Handler
    let on_import_from_disk = {
        let selected_zip = selected_zip.clone();
        let show_load_confirm = show_load_confirm.clone();
        let error_message = error_message.clone();
        let name = props.project.name.clone();
        let description = props.project.description.clone();
        let project_id = props.project.id.clone();

        Callback::from(move |_| {
            let selected_zip = selected_zip.clone();
            let show_load_confirm = show_load_confirm.clone();
            let error_message = error_message.clone();
            let compass_project = ProjectMetadata {
                id: project_id.parse().unwrap(),
                name: name.clone(),
                description: description.clone(),
            };
            spawn_local(async move {
                match SPELEO_DB_CONTROLLER
                    .import_compass_project(compass_project)
                    .await
                {
                    Ok(project) => {
                        selected_zip.set(project.project.mak_file);
                        show_load_confirm.set(true);
                    }
                    Err(e) => {
                        error_message.set(Some(format!("Failed to select file: {}", e)));
                    }
                }
            });
        })
    };

    // Confirm Load from Disk (Upload)
    let on_confirm_load = {
        let project_id = props.project.id.clone();
        let selected_zip = selected_zip.clone();
        let uploading = uploading.clone();
        let show_load_confirm = show_load_confirm.clone();
        let show_upload_success = show_upload_success.clone();
        let show_no_changes_modal = show_no_changes_modal.clone();
        let upload_error = upload_error.clone();

        Callback::from(move |_: ()| {
            let project_id = project_id.clone();
            let zip_path = (*selected_zip).clone().unwrap_or_default();
            let uploading = uploading.clone();
            let show_load_confirm = show_load_confirm.clone();
            let show_upload_success = show_upload_success.clone();
            let show_no_changes_modal = show_no_changes_modal.clone();
            let upload_error = upload_error.clone();

            show_load_confirm.set(false);
            uploading.set(true);
            upload_error.set(None);

            spawn_local(async move {
                match SPELEO_DB_CONTROLLER
                    .upload_project(&project_id, "Imported from disk", &zip_path)
                    .await
                {
                    Ok(status) => {
                        if status == 304 {
                            show_no_changes_modal.set(true);
                        } else {
                            show_upload_success.set(true);
                        }
                    }
                    Err(e) => upload_error.set(Some(format!("Upload failed: {}", e))),
                }
                uploading.set(false);
            });
        })
    };

    // Back button handler - release mutex before navigating back
    let on_back_click = {
        let project_id = props.project.id.clone();
        let on_back = props.on_back.clone();

        Callback::from(move |_| {
            let project_id = project_id.clone();
            let on_back = on_back.clone();

            spawn_local(async move {
                // Release mutex and clear active project first
                let _ = SPELEO_DB_CONTROLLER.release_mutex(&project_id).await;
                let _ = SPELEO_DB_CONTROLLER.clear_active_project().await;

                // Then navigate back (which will trigger refresh)
                on_back.emit(());
            });
        })
    };

    html! {
        <section style="width:100%;">
            <div style="margin-bottom: 16px; display: flex; justify-content: space-between; align-items: center;">
                <button onclick={on_back_click}>{"← Back to Projects"}</button>
                <button
                    onclick={on_open_folder.reform(|_| ())}
                    style="background-color: #10b981; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: 500;"
                >
                    {"🟢 Open Folder"}
                </button>
            </div>

            <h2>{"Project Details"}</h2>
            <p><strong>{"Project: "}</strong>{&props.project.name}</p>
            <p style="color: #6b7280; font-size: 14px;">{format!("ID: {}", props.project.id)}</p>

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
                    // Commit Section (Only if write access)
                    html! {

                        <div style="margin-top: 24px; padding-top: 24px; border-top: 1px solid #e5e7eb;">
                            <h3 style="margin-bottom: 12px;">{"📝 Commit Changes"}</h3>
                            <div style="margin-bottom: 16px;">
                                <input
                                    type="text"
                                    value={(*commit_message).clone()}
                                    oninput={onchange_message}
                                    placeholder="Describe your changes..."
                                    maxlength="255"
                                    style={format!(
                                        "width: 100%; padding: 8px; border: 1px solid {}; border-radius: 4px; font-family: inherit;",
                                        if *commit_message_error { "#ef4444" } else { "#d1d5db" }
                                    )}
                                />
                                {
                                    if *commit_message_error {
                                        html! {
                                            <p style="color: #ef4444; font-size: 12px; margin-top: 4px;">
                                                {"Please enter a commit message."}
                                            </p>
                                        }
                                    } else {
                                        html! {}
                                    }
                                }
                            </div>
                            <div style="display: flex; gap: 12px; flex-wrap: wrap; justify-content: center;">
                                <button
                                    onclick={on_save}
                                    disabled={*uploading}
                                    style="background-color: #2563eb; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; opacity: disabled ? 0.5 : 1;"
                                >
                                    {if *uploading { "Saving..." } else { "Save Project" }}
                                </button>
                                <button
                                    onclick={
                                        let show_reload_confirm = show_reload_confirm.clone();
                                        move |_| show_reload_confirm.set(true)
                                    }
                                    style="background-color: #f3f4f6; color: #1f2937; border: 1px solid #d1d5db; padding: 8px 16px; border-radius: 4px; cursor: pointer;"
                                >
                                    {"Reload Project"}
                                </button>
                            </div>
                            <div style="display: flex; justify-content: center; margin-top: 12px;">
                                <button
                                    onclick={on_import_from_disk}
                                    style="background-color: #10b981; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: 500;"
                                >
                                    {"Import from Disk"}
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

                }
            }

            // Read-only warning modal
            {
                if *show_readonly_modal {
                    html! {
                        <Modal
                            title="Read-Only Access"
                            message={format!(
                                "The project '{}' was opened in READ-ONLY mode.\n\n\
                                Modifications to this project cannot be saved because one of the following:\n\n\
                                - the project is currently locked by another user\n\
                                - you do not have edit permissions to the project\n\n\
                                Contact a Project Administrator if you believe this is a mistake.",
                                props.project.name
                            )}
                            modal_type={ModalType::Warning}
                            show_close_button={true}
                            on_close={close_readonly_modal}
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
                                let on_open_folder = on_open_folder.clone();
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

            // Reload Confirmation Modal
            {
                if *show_reload_confirm {
                    html! {
                        <Modal
                            title="Reload Project?"
                            message="Are you sure you want to reload this project? \n\n\
                                WARNING: This will overwrite any local changes you have made. \
                                This action cannot be undone."
                            modal_type={ModalType::Warning}
                            show_close_button={true}
                            primary_button_text={Some("Yes, Reload".to_string())}
                            on_close={move |_| show_reload_confirm.set(false)}
                            on_primary_action={on_confirm_reload}
                        />
                    }
                } else {
                    html! {}
                }
            }

            // Load from Disk Confirmation Modal
            {
                if *show_load_confirm {
                    html! {
                        <Modal
                            title="Import Project from Disk?"
                            message={format!(
                                "Are you sure you want to import '{}'? \n\n\
                                This will upload the selected file as the new version of this project.",
                                selected_zip.as_deref().unwrap_or("selected file")
                            )}
                            modal_type={ModalType::Warning}
                            show_close_button={true}
                            primary_button_text={Some("Yes, Import".to_string())}
                            on_close={move |_| show_load_confirm.set(false)}
                            on_primary_action={on_confirm_load}
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

            // Empty Project Modal (422)
            {
                {
                    let show_empty_project_modal_clone = show_empty_project_modal.clone();
                    if *show_empty_project_modal {
                        html! {
                            <Modal
                                title="Empty Project"
                                message="This project contains no Compass data yet.\n\nTo initialize the project, use the 'Import from Disk' button to upload your local project files."
                                modal_type={ModalType::Info}
                                show_close_button={true}
                                on_close={move |_| show_empty_project_modal_clone.set(false)}
                            />
                        }
                    } else {
                        html! {}
                    }
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
