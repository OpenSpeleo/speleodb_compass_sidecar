use crate::{
    components::{auth_screen::AuthScreen, loading_screen::LoadingScreen, main_layout::MainLayout},
    speleo_db_controller::SPELEO_DB_CONTROLLER,
};
use common::ui_state::{LoadingState, UiState};
use futures::StreamExt;
use log::info;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

/// Event key for UI state notifications (must match backend)
const UI_STATE_EVENT: &str = "ui-state-update";

#[function_component(App)]
pub fn app() -> Html {
    let ui_state = use_state(UiState::default);

    // Initialize app and subscribe to UI state updates via events
    {
        let ui_state = ui_state.clone();
        use_effect_with((), move |_| {
            spawn_local(async move {
                // Listen for UI state events from backend
                let mut event_stream = tauri_sys::event::listen::<UiState>(UI_STATE_EVENT)
                    .await
                    .expect("Failed to listen for UI state events");

                // Start initialization after listener is set up
                SPELEO_DB_CONTROLLER.ensure_initialized().await;

                // Process events as they arrive
                while let Some(event) = event_stream.next().await {
                    info!("ui_state: {:?}", event.payload);
                    ui_state.set(event.payload);
                }
            });
        });
    }

    let loading_state = (*ui_state).loading_state.clone();
    match loading_state {
        LoadingState::Ready => {
            html! {
                <MainLayout ui_state={(*ui_state).clone()}/>
            }
        }
        LoadingState::Unauthenticated => {
            html! {
               <AuthScreen/>
            }
        }
        // All other states occur on the loading screen
        _ => {
            html! {
                 <LoadingScreen loading_state={loading_state}/>
            }
        }
    }
}
