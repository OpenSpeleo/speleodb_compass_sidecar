use crate::{
    components::{auth_screen::AuthScreen, loading_screen::LoadingScreen, main_layout::MainLayout},
    speleo_db_controller::SPELEO_DB_CONTROLLER,
};
use common::{LoadingState, UI_STATE_NOTIFICATION_KEY, UiState};
use futures::StreamExt;
use log::info;
use tauri_sys::event::listen;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component(App)]
pub fn app() -> Html {
    // UI state
    let ui_state = use_state(|| UiState::default());

    let loading_state = (*ui_state).loading_state.clone();
    use_effect(move || {
        if let LoadingState::NotStarted = loading_state {
            spawn_local(async { SPELEO_DB_CONTROLLER.ensure_initialized().await });
        }
    });

    // Listen for UI state updates from backend
    let ui_state_clone = ui_state.clone();
    spawn_local(async move {
        let mut ui_event_stream = listen::<UiState>(UI_STATE_NOTIFICATION_KEY).await.unwrap();
        while let Some(event) = ui_event_stream.next().await {
            let updated_ui_state = event.payload;
            info!("ui_state : {:?}", updated_ui_state);
            ui_state_clone.set(updated_ui_state);
        }
    });

    let loading_state = (*ui_state).loading_state.clone();
    match loading_state {
        LoadingState::Ready => {
            return html! {
                <MainLayout ui_state={(*ui_state).clone()}/>
            };
        }
        LoadingState::Unauthenticated => {
            return html! {
               <AuthScreen/>
            };
        }
        // All other states occur on the loading screen
        _ => {
            return html! {
                 <LoadingScreen loading_state={loading_state}/>
            };
        }
    }
}
