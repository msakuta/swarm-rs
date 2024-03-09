#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod app_data;
mod bg_image;
pub use app::SwarmRsApp;

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
        Box::new(|cc| Box::new(SwarmRsApp::new(cc))),
    );
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    let mut web_options = eframe::WebOptions::default();

    // We insist to use dark theme, because light theme looks dumb.
    web_options.follow_system_theme = false;
    web_options.default_theme = eframe::Theme::Dark;

    wasm_bindgen_futures::spawn_local(async {
        eframe::start_web(
            "the_canvas_id", // hardcode it
            web_options,
            Box::new(|cc| Box::new(SwarmRsApp::new(cc))),
        )
        .await
        .expect("failed to start eframe");
    });
}
