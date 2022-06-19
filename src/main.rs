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
mod agent;

use crate::{app_data::AppData, widget::make_widget};
use druid::widget::prelude::*;
use druid::{AppLauncher, LocalizedString, WindowDesc};

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
        let start = std::time::Instant::now();
        $exec;
        start.elapsed().as_micros() as f64 / 1e6
    }}
}
