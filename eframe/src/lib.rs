#![warn(clippy::all, rust_2018_idioms)]

mod app;
mod app_data;
mod bg_image;
pub use app::SwarmRsApp;

#[cfg(target_arch = "wasm32")]
mod wasm_utils;
