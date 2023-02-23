mod paint_game;

use std::rc::Rc;

use crate::{app_data::AppData, bg_image::BgImage};
use cgmath::Matrix3;
use egui::{Pos2, Ui};
use swarm_rs::game::{BoardParams, BoardType, GameParams, UpdateResult};

const WINDOW_HEIGHT: f64 = 800.;

#[derive(Debug, PartialEq)]
enum Panel {
    Main,
    GreenBTEditor,
    RedBTEditor,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum BTEditor {
    Agent,
    Spawner,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct BTSourceFiles {
    agent: String,
    spawner: String,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct SwarmRsApp {
    #[serde(skip)]
    img_gray: BgImage,

    #[serde(skip)]
    img_labels: BgImage,

    #[serde(skip)]
    open_panel: Panel,

    #[serde(skip)]
    open_bt_panel: BTEditor,

    show_labels: bool,

    #[serde(skip)]
    app_data: AppData,

    draw_circle: bool,

    board_type: BoardType,
    seed_text: String,
    xs: usize,
    ys: usize,
    maze_expansions: usize,
    agent_count: usize,

    bt_source_file: [BTSourceFiles; 2],

    #[serde(skip)]
    pub(crate) canvas_offset: Pos2,

    #[serde(skip)]
    mouse_pos: Option<Pos2>,

    #[serde(skip)]
    last_log: Option<String>,
}

impl Default for SwarmRsApp {
    fn default() -> Self {
        let seed = 123513;
        Self {
            seed_text: seed.to_string(),
            img_gray: BgImage::new(),
            img_labels: BgImage::new(),
            open_panel: Panel::Main,
            open_bt_panel: BTEditor::Agent,
            show_labels: false,
            app_data: AppData::new(WINDOW_HEIGHT),
            draw_circle: false,
            board_type: BoardType::Rooms,
            xs: 128,
            ys: 128,
            maze_expansions: 512,
            agent_count: 3,
            bt_source_file: [
                BTSourceFiles {
                    agent: "behavior_tree_config/green/agent.txt".to_owned(),
                    spawner: "behavior_tree_config/green/spawner.txt".to_owned(),
                },
                BTSourceFiles {
                    agent: "behavior_tree_config/red/agent.txt".to_owned(),
                    spawner: "behavior_tree_config/red/spawner.txt".to_owned(),
                },
            ],
            canvas_offset: Pos2::ZERO,
            mouse_pos: None,
            last_log: None,
        }
    }
}

impl SwarmRsApp {
    pub(crate) fn view_transform(&self) -> Matrix3<f64> {
        Matrix3::from_scale(self.app_data.scale)
            * Matrix3::from_translation(self.app_data.origin.into())
    }

    pub(crate) fn inverse_view_transform(&self) -> Matrix3<f64> {
        Matrix3::from_translation(-cgmath::Vector2::from(self.app_data.origin))
            * Matrix3::from_scale(1. / self.app_data.scale)
    }

    /// Called once before the first frame.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // This is also where you can customize the look and feel of egui using
        // `cc.egui_ctx.set_visuals` and `cc.egui_ctx.set_fonts`.

        // Load previous app state (if any).
        // Note that you must enable the `persistence` feature for this to work.
        let mut res = if let Some(storage) = cc.storage {
            eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default()
        } else {
            Self::default()
        };

        println!("Recreating Game with {:?}", (res.xs, res.ys));
        let params = BoardParams {
            shape: (res.xs, res.ys),
            seed: res.seed_text.parse().unwrap_or(1),
            simplify: 0.,
            maze_expansions: res.maze_expansions,
        };
        res.app_data.new_game(res.board_type, params, true);

        res
    }

    fn show_panel_ui(&mut self, ui: &mut Ui) {
        ui.add(egui::Checkbox::new(
            &mut self.app_data.game_params.paused,
            "Paused",
        ));

        ui.collapsing("New game options", |ui| {
            if ui.button("New game").clicked() {
                let params = BoardParams {
                    shape: (self.xs, self.ys),
                    seed: self.seed_text.parse().unwrap_or(1),
                    simplify: 0.,
                    maze_expansions: self.maze_expansions,
                };
                self.app_data.new_game(self.board_type, params, true);
                self.img_gray.clear();
                self.img_labels.clear();
            }

            ui.horizontal(|ui| {
                ui.radio_value(&mut self.board_type, BoardType::Rect, "Rect");
                ui.radio_value(&mut self.board_type, BoardType::Crank, "Crank");
                ui.radio_value(&mut self.board_type, BoardType::Perlin, "Perlin");
                ui.radio_value(&mut self.board_type, BoardType::Rooms, "Rooms");
                ui.radio_value(&mut self.board_type, BoardType::Maze, "Maze");
            });

            ui.horizontal(|ui| {
                ui.label("Width: ");
                ui.add(egui::Slider::new(&mut self.xs, 32..=1024));
            });
            ui.horizontal(|ui| {
                ui.label("Height: ");
                ui.add(egui::Slider::new(&mut self.ys, 32..=1024));
            });

            ui.horizontal(|ui| {
                ui.label("Seed");
                ui.text_edit_singleline(&mut self.seed_text);
            });
            ui.horizontal(|ui| {
                ui.label("Maze expansion");
                ui.add(egui::Slider::new(&mut self.maze_expansions, 32..=1024));
            });

            ui.horizontal(|ui| {
                ui.label("Agents");
                ui.add(egui::Slider::new(&mut self.agent_count, 1..=100));
            });
        });

        ui.collapsing("View options", |ui| {
            ui.horizontal(|ui| {
                ui.add(egui::Checkbox::new(&mut self.app_data.path_visible, "Path"));

                ui.add(egui::Checkbox::new(&mut self.draw_circle, "Circle"));
            });

            ui.horizontal(|ui| {
                ui.add(egui::Checkbox::new(
                    &mut self.app_data.qtree_visible,
                    "QTree",
                ));

                ui.add(egui::Checkbox::new(
                    &mut self.app_data.qtree_search_visible,
                    "QTree search",
                ));
            });

            ui.horizontal(|ui| {
                ui.add(egui::Checkbox::new(
                    &mut self.app_data.target_visible,
                    "Target line",
                ));

                ui.add(egui::Checkbox::new(
                    &mut self.app_data.entity_trace_visible,
                    "Trace line",
                ));
            });

            ui.add(egui::Checkbox::new(
                &mut self.app_data.entity_label_visible,
                "Entity labels",
            ));

            ui.add(egui::Checkbox::new(&mut self.show_labels, "Label image"));
        });

        ui.collapsing("Statistics", |ui| {
            let game = &self.app_data.game;

            ui.horizontal(|ui| {
                for team in 0..=1 {
                    ui.vertical(|ui| {
                        ui.group(|ui| {
                            ui.label(["Green team", "Red team"][team]);
                            ui.label(format!("Spawned: {}", game.stats[team].spawned));
                            ui.label(format!("Kills: {}", game.stats[team].kills));
                            ui.label(format!("Wins: {}", game.stats[team].wins));
                        });
                    });
                }
            });
        });

        ui.collapsing("Debug output", |ui| {
            let game = &self.app_data.game;

            ui.label(format!("Scale: {:.06}", self.app_data.scale));

            ui.label(format!("Cursor: {:?}", self.mouse_pos));

            ui.label({
                let profiler = game.qtree_profiler.borrow();
                format!(
                    "QTree update time: {:.06}ms, calls: {}",
                    profiler.get_average() * 1e3,
                    profiler.get_count()
                )
            });

            ui.label({
                let profiler = game.path_find_profiler.borrow();
                format!(
                    "Path find time: {:.06}ms, calls: {}",
                    profiler.get_average() * 1e3,
                    profiler.get_count()
                )
            });
        });

        ui.group(|ui| {
            ui.heading("Selected entity");

            ui.label(format!("Id: {:?}", self.app_data.selected_entity));

            let entity = self.app_data.selected_entity.and_then(|id| {
                self.app_data
                    .game
                    .entities
                    .iter()
                    .filter_map(|entity| entity.try_borrow().ok())
                    .find(|entity| entity.get_id() == id)
            });

            match &entity {
                Some(entity) => ui.label(format!("Team: {:?}", entity.get_team())),
                None => ui.label("Team: ?"),
            };

            match &entity {
                Some(entity) => ui.label(format!(
                    "Health: {} / {}",
                    entity.get_health(),
                    entity.get_max_health()
                )),
                None => ui.label("Health: ? / ?"),
            };

            match &entity {
                Some(entity) => ui.label(format!("Target: {:?}", entity.get_target())),
                None => ui.label("Target: ?"),
            };

            match &entity {
                Some(entity) => ui.label(format!(
                    "Resource: {} / {}",
                    entity.resource(),
                    entity.max_resource()
                )),
                None => ui.label("Resource: ? / ?"),
            };

            ui.label("Print log:");

            egui::ScrollArea::vertical()
                .always_show_scroll(true)
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    let mut source = if let Some(entity) = entity {
                        entity
                            .log_buffer()
                            .iter()
                            .fold("".to_owned(), |acc, cur| acc + "\n" + cur)
                    } else if let Some(last_log) = &self.last_log {
                        last_log.clone()
                    } else {
                        "".to_owned()
                    };
                    ui.add_enabled(
                        false,
                        egui::TextEdit::multiline(&mut source)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_rows(10)
                            .lock_focus(true)
                            .desired_width(f32::INFINITY),
                    );

                    // Keep the last log in a buffer in case the entity is destroyed
                    self.last_log = Some(source);
                });
        });
    }

    fn show_editor(
        &mut self,
        ui: &mut Ui,
        contents: impl Fn(&Self) -> Rc<String>,
        mut contents_mut: impl FnMut(&mut AppData) -> &mut Rc<String>,
        game_params_mut: impl Fn(&mut GameParams) -> &mut Rc<String>,
        file: impl Fn(&Self) -> &str,
        mut file_mut: impl FnMut(&mut Self) -> &mut String,
    ) {
        ui.label(&self.app_data.message);

        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                self.app_data
                    .try_load_behavior_tree(contents(self).clone(), &game_params_mut);
            }
            ui.text_edit_singleline(file_mut(self));
            if ui.button("Reload from file").clicked() {
                let file = file(self).to_owned();
                self.app_data
                    .try_load_from_file(&file, &mut contents_mut, &game_params_mut);
            }
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            let source = Rc::make_mut(contents_mut(&mut self.app_data));
            ui.add(
                egui::TextEdit::multiline(source)
                    .font(egui::TextStyle::Monospace)
                    .code_editor()
                    .desired_rows(10)
                    .lock_focus(true)
                    .desired_width(f32::INFINITY),
            );
        });
    }

    fn bt_editor(&mut self, ui: &mut Ui, team: usize) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.open_bt_panel, BTEditor::Agent, "Agent");
            ui.selectable_value(&mut self.open_bt_panel, BTEditor::Spawner, "Spawner");
        });
        let bt_type = self.open_bt_panel;

        self.show_editor(
            ui,
            |app_data| {
                let tc = &app_data.app_data.teams[team];
                match bt_type {
                    BTEditor::Agent => &tc.agent_source,
                    BTEditor::Spawner => &tc.spawner_source,
                }
                .clone()
            },
            |app_data| {
                let tc = &mut app_data.teams[team];
                match bt_type {
                    BTEditor::Agent => &mut tc.agent_source,
                    BTEditor::Spawner => &mut tc.spawner_source,
                }
            },
            |params| {
                let tc = &mut params.teams[team];
                match bt_type {
                    BTEditor::Agent => &mut tc.agent_source,
                    BTEditor::Spawner => &mut tc.spawner_source,
                }
            },
            |app_data| match bt_type {
                BTEditor::Agent => &app_data.bt_source_file[team].agent,
                BTEditor::Spawner => &app_data.bt_source_file[team].spawner,
            },
            |app_data| match bt_type {
                BTEditor::Agent => &mut app_data.bt_source_file[team].agent,
                BTEditor::Spawner => &mut app_data.bt_source_file[team].spawner,
            },
        );
    }
}

impl eframe::App for SwarmRsApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint();

        let dt = ctx.input().stable_dt.min(0.1);

        let update_res = self.app_data.update(dt as f64 * 1000., self.agent_count);

        if let Some(UpdateResult::TeamWon(_)) = update_res {
            let params = BoardParams {
                shape: (self.xs, self.ys),
                seed: self.seed_text.parse().unwrap_or(1),
                simplify: 0.,
                maze_expansions: self.maze_expansions,
            };
            self.app_data.new_game(self.board_type, params, false);
            self.img_gray.clear();
            self.img_labels.clear();
        }

        // Examples of how to create different panels and windows.
        // Pick whichever suits you.
        // Tip: a good default choice is to just keep the `CentralPanel`.
        // For inspiration and more examples, go to https://emilk.github.io/egui

        #[cfg(not(target_arch = "wasm32"))] // no File->Quit on web pages!
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            // The top panel is often a good place for a menu bar:
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Quit").clicked() {
                        _frame.close();
                    }
                });
            });
        });

        egui::SidePanel::right("side_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.open_panel, Panel::Main, "Main");
                ui.selectable_value(
                    &mut self.open_panel,
                    Panel::GreenBTEditor,
                    "Green behavior tree",
                );
                ui.selectable_value(
                    &mut self.open_panel,
                    Panel::RedBTEditor,
                    "Red behavior tree",
                );
            });

            match self.open_panel {
                Panel::Main => self.show_panel_ui(ui),
                Panel::GreenBTEditor => self.bt_editor(ui, 0),
                Panel::RedBTEditor => self.bt_editor(ui, 1),
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.paint_game(ui);
        });

        if false {
            egui::Window::new("Window").show(ctx, |ui| {
                ui.label("Windows can be moved by dragging them.");
                ui.label("They are automatically sized based on contents.");
                ui.label("You can turn on resizing and scrolling if you like.");
                ui.label("You would normally choose either panels OR windows.");
            });
        }
    }
}
