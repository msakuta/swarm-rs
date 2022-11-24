use crate::{game::Game, WINDOW_HEIGHT};

use druid::{Data, Lens, Vec2};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone, PartialEq, Eq, Data)]
pub(crate) enum LineMode {
    None,
    Line,
    Polygon,
}

#[derive(Clone, Lens, Data)]
pub(crate) struct AppData {
    pub(crate) rows_text: String,
    pub(crate) columns_text: String,
    pub(crate) seed_text: String,
    pub(crate) game: Game,
    pub(crate) simplify_text: String,
    pub(crate) line_mode: LineMode,
    pub(crate) simplified_visible: bool,
    pub(crate) triangulation_visible: bool,
    pub(crate) unpassable_visible: bool,
    pub(crate) triangle_label_visible: bool,
    pub(crate) origin: Vec2,
    pub(crate) scale: f64,
    pub(crate) message: String,
    pub(crate) get_board_time: f64,
    #[data(ignore)]
    pub(crate) render_board_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
    pub(crate) path_visible: bool,
    pub(crate) avoidance_visible: bool,
    pub(crate) avoidance_circle_visible: bool,
    pub(crate) target_visible: bool,
    pub(crate) entity_label_visible: bool,
    pub(crate) entity_trace_visible: bool,
    pub(crate) source_visible: bool,
    /// This buffer is not yet applied to the game.
    pub(crate) source_buffer: Rc<String>,
}

impl AppData {
    pub(crate) fn new() -> Self {
        let mut game = Game::new();
        let seed = 123513;
        let scale = WINDOW_HEIGHT / game.ys as f64;

        let source_buffer = Rc::new(include_str!("../test_avoidance.txt").to_string());

        game.source = source_buffer.clone();

        Self {
            rows_text: game.xs.to_string(),
            columns_text: game.ys.to_string(),
            seed_text: seed.to_string(),
            simplify_text: game.simplify.to_string(),
            game,
            line_mode: LineMode::None,
            simplified_visible: false,
            triangulation_visible: false,
            unpassable_visible: false,
            triangle_label_visible: false,
            origin: Vec2::new(0., 0.),
            scale,
            message: "".to_string(),
            render_board_time: Cell::new(0.),
            get_board_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
            path_visible: true,
            avoidance_visible: true,
            avoidance_circle_visible: true,
            target_visible: false,
            entity_label_visible: true,
            entity_trace_visible: false,
            source_visible: false,
            source_buffer,
        }
    }

    pub(crate) fn update(&mut self) {
        self.game.update();
    }
}

pub(crate) fn is_passable_at(board: &[bool], shape: (usize, usize), pos: [f64; 2]) -> bool {
    let pos = [pos[0] as isize, pos[1] as isize];
    if pos[0] < 0 || shape.0 as isize <= pos[0] || pos[1] < 0 || shape.1 as isize <= pos[1] {
        false
    } else {
        let pos = [pos[0] as usize, pos[1] as usize];
        board[pos[0] + shape.0 * pos[1]]
    }
}
