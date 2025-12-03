pub mod auth;
mod error;
pub mod project;

pub use error::Error;

use reqwest::Client;
use std::{sync::LazyLock, time::Duration};

static API_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("Failed to build API client")
});

fn get_api_client() -> Client {
    API_CLIENT.clone()
}
