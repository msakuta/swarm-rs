mod behavior_tree_adapt;
// mod board_widget;
mod dijkstra;
pub mod marching_squares;
// mod paint_board;
pub mod perlin_noise;
mod rdp;
mod shape;
// mod widget;
#[macro_use]
mod macros;
pub mod agent;
mod collision;
pub mod entity;
pub mod fog_of_war;
pub mod game;
mod mesh;
pub mod qtree;
mod spawner;
mod temp_ents;
pub mod triangle_utils;
pub mod vfs;

pub use crate::agent::Bullet;
pub use crate::{behavior_tree_adapt::BehaviorTree, qtree::CellState};
pub use behavior_tree_lite;

#[cfg(not(target_arch = "wasm32"))]
fn measure_time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    let start = std::time::Instant::now();
    let ret = f();
    (ret, start.elapsed().as_secs_f64())
}

#[cfg(target_arch = "wasm32")]
fn measure_time<T>(f: impl FnOnce() -> T) -> (T, f64) {
    (f(), 0.)
}
