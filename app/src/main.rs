mod app;
mod components;
mod error;
mod speleo_db_controller;
mod ui_constants;

use app::App;
pub use error::Error;
use serde::{Serialize, de::DeserializeOwned};

pub(crate) type Result<T> = core::result::Result<T, Error>;

/// Call a command in the backend.
///
/// # Example
///
/// ```rust,no_run
///
/// struct User<'a> {
///     user: &'a str,
///     password: &'a str
/// }
///
/// match invoke("login", &User { user: "tauri", password: "poiwe3h4r5ip3yrhtew9ty" }).await {
///     Ok(response) => {
///         println!("Login successful: {:?}", response);
///     }
///     Err(e) => {
///         eprintln!("Login failed: {:?}", e);
///     }
/// }
///
/// ```
///
/// @param cmd The command name.
/// @param args The optional arguments to pass to the command.
/// @return A promise resolving to or rejecting the backend response.
#[inline(always)]
pub async fn invoke<A: Serialize, R: DeserializeOwned>(cmd: &str, args: &A) -> crate::Result<R> {
    let raw = inner::invoke(cmd, serde_wasm_bindgen::to_value(args)?).await?;
    serde_wasm_bindgen::from_value(raw).map_err(Into::into)
}

/// Inner wasm-bindgen bindings.
mod inner {
    use wasm_bindgen::prelude::*;
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(catch,js_namespace = ["window", "__TAURI__", "core"])]
        pub async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
    }
}

fn main() {
    console_error_panic_hook::set_once();
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
