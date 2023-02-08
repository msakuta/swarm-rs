mod app_data;
mod behavior_tree_adapt;
// mod board_widget;
mod dijkstra;
mod marching_squares;
// mod paint_board;
mod perlin_noise;
mod rdp;
mod shape;
// mod widget;
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

use crate::{app_data::AppData};
// use druid::widget::prelude::*;
// use druid::{AppLauncher, LocalizedString, WindowDesc};

const WINDOW_WIDTH: f64 = 1200.;
const WINDOW_HEIGHT: f64 = 800.;

// fn main() {
//     // Log to stdout (if you run with `RUST_LOG=debug`).
//     // tracing_subscriber::fmt::init();

//     let native_options = eframe::NativeOptions::default();
//     eframe::run_native(
//         "eframe template",
//         native_options,
//         Box::new(|cc| Box::new(swarm_rs_eframe::TemplateApp::new(cc))),
//     );
// }

fn measure_time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = std::time::Instant::now();
    let ret = f();
    (ret, start.elapsed().as_secs_f64())
}
