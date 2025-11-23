use crate::components::modal::{Modal, ModalType};
use crate::speleo_db_controller::{Project, SPELEO_DB_CONTROLLER};
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[derive(Properties, PartialEq, Clone)]
pub struct ProjectDetailsProps {
    pub project: Project,
    #[prop_or_default]
    pub on_back: Callback<()>,
}

#[function_component(ProjectDetails)]
pub fn project_details(props: &ProjectDetailsProps) -> Html {
    let downloading = use_state(|| false);
    let show_readonly_modal = use_state(|| false);
    let show_success_modal = use_state(|| false);
    let error_message: UseStateHandle<Option<String>> = use_state(|| None);
    let project_folder_path: UseStateHandle<Option<String>> = use_state(|| None);
    let is_readonly = use_state(|| false);
    let download_complete = use_state(|| false); // Track if download is complete

    // Run the download workflow automatically on mount
    {
        let project_id = props.project.id.clone();
        let downloading = downloading.clone();
        let show_readonly_modal = show_readonly_modal.clone();
        let error_message = error_message.clone();
        let project_folder_path = project_folder_path.clone();
        let is_readonly = is_readonly.clone();
        let download_complete = download_complete.clone();

        use_effect_with((), move |_| {
            spawn_local(async move {
                downloading.set(true);
                
                // Step 1: Try to acquire project mutex
                match SPELEO_DB_CONTROLLER.acquire_project_mutex(&project_id).await {
                    Ok(locked) => {
                        if !locked {
                            // Mutex acquisition failed - read-only mode
                            is_readonly.set(true);
                            show_readonly_modal.set(true);
                        }
                    }
                    Err(_e) => {
                        // Mutex acquisition had an error, but we continue anyway
                        is_readonly.set(true);
                        show_readonly_modal.set(true);
                    }
                }

                // Step 2: Download the project (regardless of mutex status)
                let zip_path = match SPELEO_DB_CONTROLLER.download_project(&project_id).await {
                    Ok(path) => path,
                    Err(e) => {
                        error_message.set(Some(format!("Download failed: {}", e)));
                        downloading.set(false);
                        return;
                    }
                };

                // Step 3: Unzip the project
                let folder_path = match SPELEO_DB_CONTROLLER.unzip_project(&zip_path, &project_id).await {
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
                // Don't show success modal yet if readonly modal is showing
            });

            || ()
        });
    }

    // Close readonly modal and show success modal if download is complete
    let close_readonly_modal = {
        let show_readonly_modal = show_readonly_modal.clone();
        let show_success_modal = show_success_modal.clone();
        let download_complete = download_complete.clone();
        Callback::from(move |_| {
            show_readonly_modal.set(false);
            // Show success modal after readonly modal is dismissed
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
        
        use_effect_with(
            download_complete.clone(),
            move |complete| {
                // Only show success modal if download is complete, not readonly, and readonly modal isn't showing
                if **complete && !*is_readonly && !*show_readonly_modal {
                    show_success_modal.set(true);
                }
                || ()
            },
        );
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
        Callback::from(move |_| {
            let project_id = project_id.clone();
            spawn_local(async move {
                let _ = SPELEO_DB_CONTROLLER.open_folder(&project_id).await;
            });
        })
    };

    html! {
        <section style="width:100%;">
            <div style="margin-bottom: 16px;">
                <button onclick={props.on_back.reform(|_| ())}>{"← Back to Projects"}</button>
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
                    html! {}
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

            // Success modal
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
                            on_primary_action={on_open_folder}
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
