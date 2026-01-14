use yew::prelude::*;

#[derive(Clone, Copy, PartialEq)]
pub enum ModalType {
    Info,
    Warning,
    Success,
    //Error,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ModalProps {
    pub title: String,
    pub message: String,
    #[prop_or(ModalType::Info)]
    pub modal_type: ModalType,
    #[prop_or_default]
    pub on_close: Callback<()>,
    #[prop_or_default]
    pub primary_button_text: Option<String>,
    #[prop_or_default]
    pub on_primary_action: Callback<()>,
    #[prop_or_default]
    pub show_close_button: bool,
}

#[function_component(Modal)]
pub fn modal(props: &ModalProps) -> Html {
    let (icon, color) = match props.modal_type {
        ModalType::Info => ("ℹ️", "#3b82f6"),
        ModalType::Warning => ("⚠️", "#f59e0b"),
        ModalType::Success => ("✅", "#10b981"),
        //ModalType::Error => ("❌", "#ef4444"),
    };

    let close_handler = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    let primary_action_handler = {
        let on_primary = props.on_primary_action.clone();
        Callback::from(move |_| on_primary.emit(()))
    };

    html! {
        <div class="modal" style="
            position: fixed;
            top: 0;
            left: 0;
            width: 100vw;
            height: 100vh;
            background-color: rgba(0, 0, 0, 0.5);
            display: flex;
            align-items: center;
            justify-content: center;
            z-index: 1000;
        ">
            <div class="modal-card" style={format!("
                background-color: white;
                border-radius: 12px;
                padding: 24px;
                max-width: 500px;
                width: 90%;
                box-shadow: 0 10px 25px rgba(0, 0, 0, 0.2);
                border-top: 4px solid {};
            ", color)}>
                <div style="display: flex; align-items: center; gap: 12px; margin-bottom: 16px;">
                    <span style="font-size: 32px;">{icon}</span>
                    <h3 style="margin: 0; font-size: 20px; color: #1f2937;">{&props.title}</h3>
                </div>
                <p style="color: #4b5563; line-height: 1.6; margin-bottom: 20px; white-space: pre-line;">
                    {&props.message}
                </p>
                <div style="display: flex; justify-content: flex-end; gap: 12px;">
                    {
                        if props.show_close_button {
                            html! {
                                <button
                                    onclick={close_handler.clone()}
                                    style="
                                        padding: 8px 16px;
                                        border: 1px solid #d1d5db;
                                        border-radius: 6px;
                                        background-color: white;
                                        color: #374151;
                                        cursor: pointer;
                                        font-size: 14px;
                                        transition: background-color 0.2s;
                                    "
                                >
                                    {"Close"}
                                </button>
                            }
                        } else {
                            html! {}
                        }
                    }
                    {
                        if let Some(btn_text) = &props.primary_button_text {
                            html! {
                                <button
                                    onclick={primary_action_handler}
                                    style={format!("
                                        padding: 8px 16px;
                                        border: none;
                                        border-radius: 6px;
                                        background-color: {};
                                        color: white;
                                        cursor: pointer;
                                        font-size: 14px;
                                        font-weight: 500;
                                        transition: opacity 0.2s;
                                    ", color)}
                                >
                                    {btn_text}
                                </button>
                            }
                        } else {
                            html! {}
                        }
                    }
                </div>
            </div>
        </div>
    }
}
