use ::swarm_rs::{
    behavior_tree_lite::parse_file,
    game::{BoardParams, BoardType, Game, GameParams, TeamConfig},
    qtree::QTreeSearcher,
};
use swarm_rs::game::UpdateResult;

use std::rc::Rc;

pub struct AppData {
    pub game: Game,
    pub game_params: GameParams,
    pub(crate) selected_entity: Option<usize>,
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
    pub(crate) teams: [TeamConfig; 2],
    pub(crate) global_render_time: f64,
}

impl AppData {
    pub fn new(window_height: f64) -> Self {
        let mut game = Game::new();
        let scale = window_height / game.shape().1 as f64;

        let teams = [
            TeamConfig {
                agent_source: Rc::new(collapse_newlines(include_str!(
                    "../../behavior_tree_config/green/agent.txt"
                ))),
                spawner_source: Rc::new(collapse_newlines(include_str!(
                    "../../behavior_tree_config/green/spawner.txt"
                ))),
            },
            TeamConfig {
                agent_source: Rc::new(collapse_newlines(include_str!(
                    "../../behavior_tree_config/red/agent.txt"
                ))),
                spawner_source: Rc::new(collapse_newlines(include_str!(
                    "../../behavior_tree_config/red/spawner.txt"
                ))),
            },
        ];

        println!(
            "Green bt: {g} red bt: {r}",
            g = teams[0].agent_source.len(),
            r = teams[1].agent_source.len()
        );

        let mut game_params = GameParams::new();
        game_params.teams = teams.clone();

        game.set_params(&game_params);
        game.init();

        Self {
            game,
            game_params,
            selected_entity: None,
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
            teams,
            global_render_time: 0.,
        }
    }

    pub fn update(&mut self, delta_time: f64, agent_count: usize) -> Option<UpdateResult> {
        self.game_params.agent_count = agent_count;
        let game = &mut self.game;
        game.set_params(&self.game_params);
        let interval = game.interval;
        let update_res = if !self.game_params.paused {
            let update_res = game.update();
            if let UpdateResult::TeamWon(team) = update_res {
                self.big_message = ["Green team won!!", "Red team won!!"][team].to_string();
                self.big_message_time = 5000.;
            }
            Some(update_res)
        } else {
            None
        };

        self.big_message_time = (self.big_message_time - delta_time).max(0.);

        self.global_render_time += interval;
        update_res
    }

    pub fn new_game(&mut self, board_type: BoardType, params: BoardParams, show_message: bool) {
        let ref mut game = self.game;
        game.new_board(board_type, &params);
        game.init();

        if show_message {
            self.big_message = "Game Start".to_string();
            self.big_message_time = 5000.;
        }
    }

    pub fn with_qtree(&self, f: impl FnOnce(&QTreeSearcher)) {
        let game = &self.game;
        f(&game.qtree);
    }

    pub fn try_load_behavior_tree(
        &mut self,
        src: Rc<String>,
        setter: &impl Fn(&mut GameParams) -> &mut Rc<String>,
    ) -> bool {
        fn count_newlines(src: &str) -> usize {
            src.lines().count()
        }

        // Check the syntax before applying
        match parse_file(&src) {
            Ok(("", _)) => {
                *setter(&mut self.game_params) = src.clone();
                self.message = format!("Behavior tree applied! {}", Rc::strong_count(&src));
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
        get_mut: &mut impl FnMut(&mut AppData) -> &mut Rc<String>,
        setter: &impl Fn(&mut GameParams) -> &mut Rc<String>,
    ) {
        match std::fs::read_to_string(file) {
            Ok(s) => {
                let s = Rc::new(collapse_newlines(&s));
                if self.try_load_behavior_tree(s.clone(), setter) {
                    *get_mut(self) = s;
                }
            }
            Err(e) => self.message = format!("Read file error! {e:?}"),
        }
    }
}

/// Windows still uses CRLF
fn collapse_newlines(s: &str) -> String {
    // Can we skip replacing in *nix and newer Mac? Maybe, but it's such a fast operation
    // that we don't gain much by "optimizing" for the platform.
    s.replace("\r\n", "\n")
}
