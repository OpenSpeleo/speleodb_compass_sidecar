pub mod auth;
pub mod project;

use reqwest::Client;
use std::{sync::LazyLock, time::Duration};

#[cfg(debug_assertions)]
pub const API_BASE_URL: &str = "https://stage.speleodb.org";
#[cfg(not(debug_assertions))]
pub const API_BASE_URL: &str = "https://www.speleodb.org";
const API_USER_AGENT: &str = concat!(
    "Tauri/SpeleoDB-Compass-Sidecar/v",
    env!("CARGO_PKG_VERSION")
);

static API_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .user_agent(API_USER_AGENT)
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build API client")
});

fn get_api_client() -> Client {
    API_CLIENT.clone()
}

#[cfg(test)]
mod tests {
    use super::API_USER_AGENT;

    #[test]
    fn api_user_agent_has_expected_format() {
        assert_eq!(
            API_USER_AGENT,
            format!(
                "Tauri/SpeleoDB-Compass-Sidecar/v{}",
                env!("CARGO_PKG_VERSION")
            )
        );
    }
}
