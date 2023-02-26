use ::swarm_rs::{
    behavior_tree_lite::parse_file,
    game::{BoardParams, BoardType, Game, GameParams, TeamConfig},
    qtree::QTreeSearcher,
};

use swarm_rs::{game::UpdateResult, vfs::Vfs};

#[cfg(not(target_arch = "wasm32"))]
use swarm_rs::vfs::FileVfs;

use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BTEditor {
    Agent,
    Spawner,
}

pub struct AppData {
    pub game: Game,
    pub game_params: GameParams,
    pub(crate) selected_entity: Option<usize>,
    pub origin: [f64; 2],
    pub scale: f64,
    message: String,
    /// Optional payload for detailed data about the error, can be long
    message_payload: String,
    message_visible: bool,
    confirming: bool,
    confirmed: Option<Box<dyn FnOnce(&mut Self)>>,
    pub(crate) big_message: String,
    pub big_message_time: f64,
    pub path_visible: bool,
    // pub(crate) avoidance_render_params: AvoidanceRenderParams,
    pub qtree_visible: bool,
    pub qtree_search_visible: bool,
    pub target_visible: bool,
    pub(crate) entity_label_visible: bool,
    pub(crate) entity_trace_visible: bool,
    pub(crate) global_render_time: f64,
    pub(crate) selected_bt: (usize, BTEditor),
    pub(crate) new_file_name: String,
    pub(crate) current_file_name: String,
    /// This buffer is not yet applied to the game.
    pub(crate) bt_buffer: String,
    pub(crate) dirty: bool,
    pub(crate) vfs: Option<Box<dyn Vfs>>,
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

        #[cfg(target_arch = "wasm32")]
        let vfs = crate::wasm_utils::LocalStorageVfs::new();

        #[cfg(not(target_arch = "wasm32"))]
        let vfs = FileVfs::new();

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
            message_payload: "".to_string(),
            message_visible: false,
            confirming: false,
            confirmed: None,
            big_message: "Game Start".to_string(),
            big_message_time: 5000.,
            path_visible: true,
            qtree_visible: false,
            qtree_search_visible: false,
            target_visible: false,
            entity_label_visible: true,
            entity_trace_visible: false,
            global_render_time: 0.,
            selected_bt: (0, BTEditor::Agent),
            new_file_name: "agent.txt".to_owned(),
            current_file_name: "".to_owned(),
            bt_buffer: "".to_owned(),
            dirty: false,
            vfs: Some(Box::new(vfs)),
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
    ) -> Result<(), ()> {
        fn count_newlines(src: &str) -> usize {
            src.lines().count()
        }

        // Check the syntax before applying
        match parse_file(&src) {
            Ok(("", _)) => {
                *setter(&mut self.game_params) = src.clone();
                Ok(())
            }
            Ok((rest, _)) => {
                let parsed_src = &src[..rest.as_ptr() as usize - src.as_ptr() as usize];
                self.set_message_with_payload(
                    format!(
                        "Behavior tree source ended unexpectedly at ({})",
                        count_newlines(parsed_src)
                    ),
                    format!("Rest: {rest}"),
                );
                Err(())
            }
            Err(e) => {
                self.set_message(format!("Behavior tree failed to parse: {}", e));
                Err(())
            }
        }
    }

    pub(crate) fn apply_bt(
        &mut self,
        vfs: &dyn Vfs,
        file_name: &str,
        team: usize,
        bt_type: BTEditor,
    ) -> Result<(), String> {
        let content = vfs.get_file(file_name)?;

        if self
            .try_load_behavior_tree(Rc::new(content), &mut |params: &mut GameParams| {
                let tc = &mut params.teams[team];
                match bt_type {
                    BTEditor::Agent => &mut tc.agent_source,
                    BTEditor::Spawner => &mut tc.spawner_source,
                }
            })
            .is_ok()
        {
            Ok(())
        } else {
            Ok(()) //("Load behavior tree failed".to_owned())
        }
    }

    pub(crate) fn _get_message(&self) -> &str {
        &self.message
    }

    pub(crate) fn set_message(&mut self, message: String) {
        self.message = message;
        self.message_payload = "".to_string();
        self.message_visible = true;
        self.confirming = false;
    }

    pub(crate) fn set_confirm_message(
        &mut self,
        message: String,
        confirmed: Box<dyn FnOnce(&mut Self)>,
    ) {
        self.message = message;
        self.message_payload = "".to_string();
        self.message_visible = true;
        self.confirming = true;
        self.confirmed = Some(confirmed);
    }

    pub(crate) fn set_message_with_payload(&mut self, message: String, payload: String) {
        self.message = message;
        self.message_payload = payload;
        self.message_visible = true;
        self.confirming = false;
    }

    pub(crate) fn show_message(&mut self, ctx: &egui::Context) {
        if !self.message.is_empty() {
            let mut hide = false;
            let mut confirmed = false;
            egui::Window::new("Message")
                .open(&mut self.message_visible)
                .collapsible(false)
                .fixed_size([
                    400.,
                    if !self.message_payload.is_empty() {
                        400.
                    } else {
                        50.
                    },
                ])
                .show(ctx, |ui| {
                    ui.vertical_centered_justified(|ui| {
                        ui.label(
                            egui::RichText::new(&self.message)
                                .font(egui::FontId::proportional(18.)),
                        );

                        if !self.message_payload.is_empty() {
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.add_enabled(
                                    false,
                                    egui::TextEdit::multiline(&mut self.message_payload)
                                        .font(egui::TextStyle::Monospace)
                                        .code_editor(),
                                );
                            });
                        }
                        if self.confirming {
                            ui.horizontal_centered(|ui| {
                                if ui.button("Ok").clicked() {
                                    hide = true;
                                    confirmed = true;
                                }
                                if ui.button("Cancel").clicked() {
                                    hide = true;
                                    self.confirmed = None;
                                }
                            });
                        }
                    });
                });
            if hide {
                self.message_visible = false;
            }
            if confirmed {
                if let Some(confirmed) = self.confirmed.take() {
                    confirmed(self);
                }
            }
        }
    }
}

/// Windows still uses CRLF
fn collapse_newlines(s: &str) -> String {
    // Can we skip replacing in *nix and newer Mac? Maybe, but it's such a fast operation
    // that we don't gain much by "optimizing" for the platform.
    s.replace("\r\n", "\n")
}
