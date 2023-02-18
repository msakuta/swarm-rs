mod app_data;
mod board_widget;
mod paint_board;
mod widget;
pub mod agent {
    pub mod avoidance {
        pub mod render;
        pub(crate) use self::render::AvoidanceRenderParams;
    }
}
mod qtree {
    pub mod render;
}

use crate::{
    app_data::{AppData, WINDOW_HEIGHT, WINDOW_WIDTH},
    widget::make_widget,
};

use ::druid::{AppLauncher, LocalizedString, Size, WindowDesc};

pub fn main() {
    log::set_max_level(log::LevelFilter::Off);

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
