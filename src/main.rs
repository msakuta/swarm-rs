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
mod collision;
mod entity;
mod game;
mod spawner;
mod temp_ents;
mod triangle_utils;

use crate::{app_data::AppData, widget::make_widget};
use druid::widget::prelude::*;
use druid::{AppLauncher, LocalizedString, WindowDesc};

const WINDOW_WIDTH: f64 = 1200.;
const WINDOW_HEIGHT: f64 = 800.;

pub fn main() {
    let window = WindowDesc::new(make_widget)
        .window_size(Size {
            width: WINDOW_WIDTH,
            height: WINDOW_HEIGHT,
        })
        .resizable(true)
        .title(
            LocalizedString::new("custom-widget-demo-window-title").with_placeholder("Swarm-rs"),
        );

    AppLauncher::with_window(window)
        .use_simple_logger()
        .launch(AppData::new())
        .expect("launch failed");
}

fn measure_time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = std::time::Instant::now();
    let ret = f();
    (ret, start.elapsed().as_secs_f64())
}
