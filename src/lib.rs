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
pub mod game;
mod mesh;
pub mod qtree;
mod spawner;
mod temp_ents;
pub mod triangle_utils;

pub use crate::agent::Bullet;
pub use crate::qtree::CellState;
pub use behavior_tree_lite;

// #[wasm_bindgen]
// pub fn wasm_main() {
//     // This hook is necessary to get panic messages in the console
//     std::panic::set_hook(Box::new(console_error_panic_hook::hook));
//     main()
// }

// pub fn main() {
//     let window = WindowDesc::new(make_widget())
//         .window_size(Size {
//             width: WINDOW_WIDTH,
//             height: WINDOW_HEIGHT,
//         })
//         .resizable(true)
//         .title(
//             LocalizedString::new("custom-widget-demo-window-title").with_placeholder("Swarm-rs"),
//         );

//     AppLauncher::with_window(window)
//         .log_to_console()
//         .launch(AppData::new())
//         .expect("launch failed");
// }

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
