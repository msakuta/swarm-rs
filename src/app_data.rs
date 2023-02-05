use crate::{
    agent::AvoidanceRenderParams,
    game::{BoardType, Game, GameParams},
    WINDOW_HEIGHT,
};

use druid::{Data, Lens, Point, Vec2};
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
    pub(crate) maze_expansions: String,
    pub(crate) board_type: BoardType,
    pub(crate) game: Rc<RefCell<Game>>,
    pub(crate) game_params: GameParams,
    pub(crate) simplify_text: String,
    pub(crate) line_mode: LineMode,
    pub(crate) simplified_visible: bool,
    pub(crate) triangulation_visible: bool,
    pub(crate) unpassable_visible: bool,
    pub(crate) triangle_label_visible: bool,
    pub(crate) origin: Vec2,
    pub(crate) scale: f64,
    pub(crate) message: String,
    pub(super) mouse_pos: Option<Point>,
    pub(crate) get_board_time: f64,
    #[data(ignore)]
    pub(crate) render_board_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
    pub(crate) path_visible: bool,
    pub(crate) avoidance_render_params: AvoidanceRenderParams,
    pub qtree_visible: bool,
    pub qtree_search_visible: bool,
    pub(crate) target_visible: bool,
    pub(crate) entity_label_visible: bool,
    pub(crate) entity_trace_visible: bool,
    pub(crate) source_visible: bool,
    pub(crate) source_file: String,
    /// This buffer is not yet applied to the game.
    pub(crate) agent_source_buffer: Rc<String>,
    pub(crate) spawner_source_buffer: Rc<String>,
    pub(crate) global_render_time: f64,
}

impl AppData {
    pub(crate) fn new() -> Self {
        let game = Game::new();
        let seed = 123513;
        let scale = WINDOW_HEIGHT / game.ys as f64;
        let maze_expansion = 2000;

        const SOURCE_FILE: &'static str = "behavior_tree.txt";

        let agent_source_buffer =
            Rc::new(include_str!("../behavior_tree_config/agent.txt").to_string());
        let spawner_source_buffer =
            Rc::new(include_str!("../behavior_tree_config/spawner.txt").to_string());

        let mut game_params = GameParams::new();
        game_params.agent_source = agent_source_buffer.clone();
        game_params.spawner_source = spawner_source_buffer.clone();

        Self {
            rows_text: game.xs.to_string(),
            columns_text: game.ys.to_string(),
            seed_text: seed.to_string(),
            maze_expansions: maze_expansion.to_string(),
            board_type: BoardType::Perlin,
            simplify_text: game.simplify.to_string(),
            game: Rc::new(RefCell::new(game)),
            game_params,
            line_mode: LineMode::None,
            simplified_visible: false,
            triangulation_visible: false,
            unpassable_visible: false,
            triangle_label_visible: false,
            origin: Vec2::new(0., 0.),
            scale,
            message: "".to_string(),
            mouse_pos: None,
            render_board_time: Cell::new(0.),
            get_board_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
            path_visible: true,
            avoidance_render_params: AvoidanceRenderParams::new(),
            qtree_visible: false,
            qtree_search_visible: false,
            target_visible: false,
            entity_label_visible: true,
            entity_trace_visible: false,
            source_visible: false,
            source_file: SOURCE_FILE.to_string(),
            agent_source_buffer,
            spawner_source_buffer,
            global_render_time: 0.,
        }
    }

    pub(crate) fn update(&mut self) -> (bool, f64) {
        let mut game = self.game.borrow_mut();
        game.set_params(&self.game_params);
        if !self.game_params.paused {
            game.update();
        }
        self.global_render_time += game.interval;
        (self.game_params.paused, game.interval)
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
