mod api_info;
pub mod api_types;
pub mod ui_state;

pub use api_info::{ApiInfo, OauthToken};
pub use errors::Error;

#[cfg(debug_assertions)]
pub const API_BASE_URL: &str = "https://stage.speleodb.org";
#[cfg(not(debug_assertions))]
pub const API_BASE_URL: &str = "https://www.speleodb.org";

pub const SERVER_TIME_ZONE: &str = "US/Eastern";
