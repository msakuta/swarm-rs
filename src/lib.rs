mod app_data;
mod behavior_tree_adapt;
mod board_widget;
mod dijkstra;
mod marching_squares;
mod paint_board;
mod perlin_noise;
mod rdp;
mod shape;
mod widget;
#[macro_use]
mod macros;
mod agent;
mod collision;
mod entity;
mod game;
mod mesh;
mod qtree;
mod spawner;
mod temp_ents;
mod triangle_utils;

use crate::{app_data::AppData, widget::make_widget};
use druid::{AppLauncher, LocalizedString, Size, WindowDesc};
use wasm_bindgen::prelude::*;

const WINDOW_WIDTH: f64 = 1200.;
const WINDOW_HEIGHT: f64 = 800.;

#[wasm_bindgen]
pub fn wasm_main() {
    // This hook is necessary to get panic messages in the console
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main()
}

pub fn main() {
    let window = WindowDesc::new(make_widget())
        .window_size(Size {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        })
        .resizable(true)
        .title(
            LocalizedString::new("custom-widget-demo-window-title").with_placeholder("Swarm-rs"),
        );

    AppLauncher::with_window(window)
        .log_to_console()
        .launch(AppData::new())
        .expect("launch failed");
}

fn measure_time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    (f(), 0.)
}
