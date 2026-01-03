use common::ui_state::LoadingState;
use yew::{Html, Properties, function_component, html};

#[derive(Properties, PartialEq)]
pub struct LoadingScreenProps {
    pub loading_state: LoadingState,
}

#[function_component(LoadingScreen)]
pub fn loading_screen(&LoadingScreenProps { ref loading_state }: &LoadingScreenProps) -> Html {
    html! {
        <div style="
            position: fixed;
            top: 0;
            left: 0;
            right: 0;
            bottom: 0;
            display: flex;
            flex-direction: column;
            align-items: center;
            z-index: 9999;
            backdrop-filter: blur(2px);
        ">
            <div style="
                width:100%;
                height: 100%;
                display: flex;
                flex-direction: column;
                justify-content: space-between;
                align-items: center;
            ">
                <img src="public/speleodb_long.png" class="logo speleodb" alt="SpeleoDB logo"/>
                <div class="container" style="
                    width:100%;
                    height: 100%;
                ">
                <div style="
                    padding: 32px 48px;
                    border-radius: 12px;
                    display: flex;
                    flex-direction: column;
                    justify-content: center;
                    align-items: center;">
                    <div class="spinner" />
                    <p style="
                        padding-top: 16px;
                        color: #1f2937;
                        font-size: 18px;
                        font-weight: 500;
                        margin: 0;
                    ">
                    {
                        match loading_state {
                        LoadingState::NotStarted=>
                            "Initializing...".to_string()
                        ,
                        LoadingState::CheckingForUpdates =>
                            "Checking for updates...".to_string()
                        ,
                        LoadingState::Updating=>
                            "Updating application...".to_string()
                        ,
                        LoadingState::LoadingPrefs =>
                            "Loading user preferences...".to_string()
                    ,
                        LoadingState::Authenticating =>
                            "Authenticating user...".to_string()
                        ,
                        LoadingState::LoadingProjects =>
                            "Loading projects...".to_string()
                        ,
                        LoadingState::Failed(e)=> format!("Error: {}", e),
                        _=>"Starting application...".to_string()
                        }
                    }
                    </p>
                    </div>
                </div>
            </div>
            <style>
                {r#"
                    @keyframes spin {
                        0% { transform: rotate(0deg); }
                        100% { transform: rotate(360deg); }
                    }
                "#}
            </style>
        </div>
    }
}
