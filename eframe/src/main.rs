#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod app_data;
mod bg_image;
pub use app::SwarmRsApp;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod wasm_utils;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    // tracing_subscriber::fmt::init();

    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "swarm-rs application in eframe",
        native_options,
        Box::new(|cc| Ok(Box::new(SwarmRsApp::new(cc)))),
    )
    .unwrap();
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    #[derive(Clone)]
    #[wasm_bindgen]
    struct WebHandle {
        runner: eframe::WebRunner,
    }

    impl WebHandle {
        pub fn new() -> Self {
            Self {
                runner: eframe::WebRunner::new(),
            }
        }

        pub async fn start(
            &self,
            canvas: web_sys::HtmlCanvasElement,
        ) -> Result<(), wasm_bindgen::JsValue> {
            self.runner
                .start(
                    canvas,
                    eframe::WebOptions::default(),
                    Box::new(|cc| Ok(Box::new(SwarmRsApp::new(cc)))),
                )
                .await
        }
    }

    wasm_bindgen_futures::spawn_local(async {
        let canvas = web_sys::window()
            .expect("no global `window` exists")
            .document()
            .expect("should have a document")
            .get_element_by_id("the_canvas_id")
            .expect("should have #the_canvas_id on the page")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#the_canvas_id should be a <canvas> element");
        WebHandle::new()
            .start(canvas)
            .await
            .expect("failed to start eframe");
    });
}
