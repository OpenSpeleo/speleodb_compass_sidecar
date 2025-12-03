pub mod auth;
mod error;
pub mod project;

pub use error::Error;

use reqwest::Client;
use std::{sync::LazyLock, time::Duration};

#[cfg(debug_assertions)]
const API_BASE_URL: &str = "https://stage.speleodb.org";
#[cfg(not(debug_assertions))]
const API_BASE_URL: &str = "https://www.speleodb.com";

static API_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build API client")
});

fn get_api_client() -> Client {
    API_CLIENT.clone()
}
