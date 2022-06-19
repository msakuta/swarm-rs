mod app_data;
mod board_widget;
mod marching_squares;
mod paint_board;
mod perlin_noise;
mod rdp;
mod shape;
mod widget;
#[macro_use]
mod macros;

use crate::{app_data::AppData, widget::make_widget};
use druid::{AppLauncher, Data, Lens, LocalizedString, Size, WindowDesc};
use wasm_bindgen::prelude::*;

#[derive(Clone, Data, Lens)]
struct HelloState {
    name: String,
}

#[wasm_bindgen]
pub fn wasm_main() {
    // This hook is necessary to get panic messages in the console
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    main()
}

pub fn main() {
    let window = WindowDesc::new(make_widget)
        .window_size(Size {
            width: 1200.0,
            height: 800.0,
        })
        .resizable(true)
        .title(
            LocalizedString::new("custom-widget-demo-window-title")
                .with_placeholder("Mesh Transform Editor"),
        );

    AppLauncher::with_window(window)
        .use_simple_logger()
        .launch(AppData::new())
        .expect("launch failed");
}

#[macro_export]
macro_rules! measure_time {
    {$exec:expr} => {{
        $exec;
        0.
    }}
}
