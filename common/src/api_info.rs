use crate::API_BASE_URL;
use serde::{Deserialize, Serialize};
use url::Url;

// TODO:: Add newtype to encapsulate Oauth token validaiton
#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct OauthToken(String);

impl AsRef<str> for OauthToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ApiInfo {
    instance: Url,
    oauth_token: Option<String>,
}

impl Default for ApiInfo {
    fn default() -> Self {
        Self {
            instance: API_BASE_URL.parse().unwrap(),
            oauth_token: None,
        }
    }
}

impl ApiInfo {
    pub fn new(instance: Url, oauth_token: Option<String>) -> Self {
        Self {
            instance,
            oauth_token,
        }
    }

    pub fn instance(&self) -> &Url {
        &self.instance
    }

    pub fn oauth_token(&self) -> Option<&str> {
        self.oauth_token.as_deref()
    }
}
