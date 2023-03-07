use druid::{Point, Vec2};
use swarm_rs::{
    behavior_tree_lite::parse_file,
    // agent::AvoidanceRenderParams,
    game::{BoardParams, BoardType, Game, GameParams, TeamConfig},
};

use ::druid::{Data, Lens};

use crate::agent::avoidance::AvoidanceRenderParams;

use std::{cell::RefCell, rc::Rc};

pub(crate) const WINDOW_WIDTH: f64 = 1200.;
pub(crate) const WINDOW_HEIGHT: f64 = 800.;

#[derive(Clone, PartialEq, Eq, Data)]
pub enum LineMode {
    None,
    Line,
    Polygon,
}

#[derive(Clone, Data, Lens)]
pub struct AppData {
    pub xs_text: String,
    pub ys_text: String,
    pub seed_text: String,
    pub maze_expansions: String,
    pub board_type: BoardType,
    pub game: Rc<RefCell<Game>>,
    pub game_params: GameParams,
    pub(crate) simplify_text: String,
    pub agent_count_text: String,
    pub(crate) line_mode: LineMode,
    pub(crate) simplified_visible: bool,
    pub(crate) triangulation_visible: bool,
    pub(crate) unpassable_visible: bool,
    pub(crate) triangle_label_visible: bool,
    pub(crate) show_label_image: bool,
    pub origin: Vec2,
    pub scale: f64,
    pub message: String,
    pub(crate) big_message: String,
    pub big_message_time: f64,
    pub(super) mouse_pos: Option<Point>,
    pub(crate) get_board_time: f64,
    // pub(crate) render_board_time: Cell<f64>,
    pub(crate) render_stats: Rc<RefCell<String>>,
    pub path_visible: bool,
    pub(crate) avoidance_render_params: AvoidanceRenderParams,
    pub qtree_visible: bool,
    pub qtree_search_visible: bool,
    pub target_visible: bool,
    pub(crate) entity_label_visible: bool,
    pub(crate) entity_trace_visible: bool,
    pub agent_source_file: String,
    /// This buffer is not yet applied to the game.
    pub agent_source_buffer: Rc<String>,
    pub(crate) spawner_source_file: String,
    pub spawner_source_buffer: Rc<String>,
    pub(crate) global_render_time: f64,
}

impl AppData {
    pub fn new() -> Self {
        let mut game = Game::new();
        let seed = 123513;
        let scale = WINDOW_HEIGHT / game.shape().1 as f64;
        let maze_expansion = 2000;

        const AGENT_SOURCE_FILE: &'static str = "behavior_tree_config/green/agent.btc";
        const SPAWNER_SOURCE_FILE: &'static str = "behavior_tree_config/green/spawner.btc";

        let teams = [
            TeamConfig {
                agent_source: Rc::new(
                    include_str!("../../behavior_tree_config/green/agent.btc").to_string(),
                ),
                spawner_source: Rc::new(
                    include_str!("../../behavior_tree_config/green/spawner.btc").to_string(),
                ),
            },
            TeamConfig {
                agent_source: Rc::new(
                    include_str!("../../behavior_tree_config/red/agent.btc").to_string(),
                ),
                spawner_source: Rc::new(
                    include_str!("../../behavior_tree_config/red/spawner.btc").to_string(),
                ),
            },
        ];

        let mut game_params = GameParams::new();
        game_params.teams = teams.clone();

        game.set_params(&game_params);
        game.init();

        Self {
            xs_text: game.shape().0.to_string(),
            ys_text: game.shape().1.to_string(),
            seed_text: seed.to_string(),
            maze_expansions: maze_expansion.to_string(),
            board_type: BoardType::Rooms,
            simplify_text: game.simplify.to_string(),
            agent_count_text: game.params.agent_count.to_string(),
            game: Rc::new(RefCell::new(game)),
            game_params,
            line_mode: LineMode::None,
            simplified_visible: false,
            triangulation_visible: false,
            unpassable_visible: false,
            triangle_label_visible: false,
            show_label_image: false,
            origin: Vec2::ZERO,
            scale,
            message: "".to_string(),
            big_message: "Game Start".to_string(),
            big_message_time: 5000.,
            mouse_pos: None,
            // render_board_time: Cell::new(0.),
            get_board_time: 0.,
            render_stats: Rc::new(RefCell::new("".to_string())),
            path_visible: true,
            avoidance_render_params: AvoidanceRenderParams::new(),
            qtree_visible: false,
            qtree_search_visible: false,
            target_visible: false,
            entity_label_visible: true,
            entity_trace_visible: false,
            agent_source_file: AGENT_SOURCE_FILE.to_string(),
            agent_source_buffer: teams[0].agent_source.clone(),
            spawner_source_file: SPAWNER_SOURCE_FILE.to_string(),
            spawner_source_buffer: teams[0].spawner_source.clone(),
            global_render_time: 0.,
        }
    }

    pub fn update(&mut self) -> (bool, f64) {
        self.game_params.agent_count = self.agent_count_text.parse().unwrap_or(3);
        let mut game = self.game.borrow_mut();
        game.set_params(&self.game_params);
        let interval = game.interval;
        if !self.game_params.paused {
            let update_res = game.update();
            if let swarm_rs::game::UpdateResult::TeamWon(team) = update_res {
                drop(game);
                self.new_game();
                self.big_message = ["Green team won!!", "Red team won!!"][team].to_string();
                self.big_message_time = 5000.;
            }
        }
        self.global_render_time += interval;
        (self.game_params.paused, interval)
    }

    pub fn new_game(&mut self) {
        let xs = self.xs_text.parse().unwrap_or(64);
        let ys = self.ys_text.parse().unwrap_or(64);
        let seed = self.seed_text.parse().unwrap_or(1);
        let simplify = self.simplify_text.parse().unwrap_or(1.);
        let params = BoardParams {
            shape: (xs, ys),
            seed,
            simplify,
            maze_expansions: self.maze_expansions.parse().unwrap_or(1),
        };
        let mut game = self.game.borrow_mut();
        game.new_board(self.board_type, &params);
        game.init();

        self.big_message = "Game Start".to_string();
        self.big_message_time = 5000.;
    }

    pub fn try_load_behavior_tree(
        &mut self,
        src: Rc<String>,
        setter: fn(&mut GameParams) -> &mut Rc<String>,
    ) -> bool {
        fn count_newlines(src: &str) -> usize {
            src.lines().count()
        }

        // Check the syntax before applying
        match parse_file(&src) {
            Ok(("", _)) => {
                *setter(&mut self.game_params) = src.clone();
                self.message = format!(
                    "Behavior tree applied! {}",
                    Rc::strong_count(&self.agent_source_buffer)
                );
                true
            }
            Ok((rest, _)) => {
                let parsed_src = &src[..rest.as_ptr() as usize - src.as_ptr() as usize];
                self.message = format!(
                    "Behavior tree source ended unexpectedly at ({}) {:?}",
                    count_newlines(parsed_src),
                    rest
                );
                false
            }
            Err(e) => {
                self.message = format!("Behavior tree failed to parse: {}", e);
                false
            }
        }
    }
}
