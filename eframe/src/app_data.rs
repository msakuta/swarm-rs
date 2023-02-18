use ::swarm_rs::{
    behavior_tree_lite::parse_file,
    game::{BoardParams, BoardType, Game, GameParams},
    qtree::QTreeSearcher,
};

use std::rc::Rc;

pub struct AppData {
    pub xs_text: String,
    pub ys_text: String,
    pub seed_text: String,
    pub maze_expansions: String,
    pub board_type: BoardType,
    pub game: Game,
    pub game_params: GameParams,
    pub(crate) simplify_text: String,
    pub agent_count_text: String,
    pub origin: [f64; 2],
    pub scale: f64,
    pub message: String,
    pub(crate) big_message: String,
    pub big_message_time: f64,
    pub path_visible: bool,
    // pub(crate) avoidance_render_params: AvoidanceRenderParams,
    pub qtree_visible: bool,
    pub qtree_search_visible: bool,
    pub target_visible: bool,
    pub(crate) entity_label_visible: bool,
    pub(crate) entity_trace_visible: bool,
    /// This buffer is not yet applied to the game.
    pub agent_source_buffer: Rc<String>,
    pub spawner_source_buffer: Rc<String>,
    pub(crate) global_render_time: f64,
}

impl AppData {
    pub fn new(window_height: f64) -> Self {
        let mut game = Game::new();
        let seed = 123513;
        let scale = window_height / game.shape().1 as f64;
        let maze_expansion = 2000;

        let agent_source_buffer =
            Rc::new(include_str!("../../behavior_tree_config/agent.txt").to_string());
        let spawner_source_buffer =
            Rc::new(include_str!("../../behavior_tree_config/spawner.txt").to_string());

        let mut game_params = GameParams::new();
        game_params.agent_source = agent_source_buffer.clone();
        game_params.spawner_source = spawner_source_buffer.clone();

        game.set_params(&game_params);
        game.init();

        Self {
            xs_text: game.shape().0.to_string(),
            ys_text: game.shape().1.to_string(),
            seed_text: seed.to_string(),
            maze_expansions: maze_expansion.to_string(),
            board_type: BoardType::Perlin,
            simplify_text: game.simplify.to_string(),
            agent_count_text: game.agent_count.to_string(),
            game,
            game_params,
            origin: [0., 0.],
            scale,
            message: "".to_string(),
            big_message: "Game Start".to_string(),
            big_message_time: 5000.,
            path_visible: true,
            qtree_visible: false,
            qtree_search_visible: false,
            target_visible: false,
            entity_label_visible: true,
            entity_trace_visible: false,
            agent_source_buffer,
            spawner_source_buffer,
            global_render_time: 0.,
        }
    }

    pub fn update(&mut self) -> (bool, f64) {
        self.game_params.agent_count = self.agent_count_text.parse().unwrap_or(3);
        let game = &mut self.game;
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
        let ref mut game = self.game;
        game.new_board(self.board_type, &params);
        game.init();

        self.big_message = "Game Start".to_string();
        self.big_message_time = 5000.;
    }

    pub fn with_qtree(&self, f: impl FnOnce(&QTreeSearcher)) {
        let game = &self.game;
        f(&game.qtree);
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

    pub fn try_load_from_file(
        &mut self,
        file: &str,
        get_mut: fn(&mut AppData) -> &mut Rc<String>,
        setter: fn(&mut GameParams) -> &mut Rc<String>,
    ) {
        match std::fs::read_to_string(file) {
            Ok(s) => {
                let s = Rc::new(s);
                if self.try_load_behavior_tree(s.clone(), setter) {
                    *get_mut(self) = s;
                }
            }
            Err(e) => self.message = format!("Read file error! {e:?}"),
        }
    }
}
