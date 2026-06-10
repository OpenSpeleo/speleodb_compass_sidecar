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

        /// Fire-and-forget variant of `invoke`. Dispatches the IPC synchronously
        /// and returns the JS Promise without awaiting it. Used from the panic
        /// hook, where the wasm instance is about to abort and async polling is
        /// unreliable.
        #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"], js_name = "invoke")]
        pub fn invoke_fire_and_forget(cmd: &str, args: JsValue) -> js_sys::Promise;
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrontendErrorArgs<'a> {
    message: String,
    context: &'a str,
}

/// Install a panic hook that (1) logs to the browser console as before and
/// (2) best-effort reports the panic to the backend, which forwards it to
/// Sentry. The report is fire-and-forget: a wasm panic aborts the instance, so
/// we dispatch the IPC synchronously and ignore the returned Promise.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        console_error_panic_hook::hook(info);
        let args = FrontendErrorArgs {
            message: info.to_string(),
            context: "panic",
        };
        if let Ok(js) = serde_wasm_bindgen::to_value(&args) {
            let _ = inner::invoke_fire_and_forget("report_frontend_error", js);
        }
    }));
}

fn main() {
    install_panic_hook();
    wasm_logger::init(wasm_logger::Config::default());
    yew::Renderer::<App>::new().render();
}
