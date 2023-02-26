mod paint_game;

use std::path::Path;

use crate::{
    app_data::{AppData, BtType},
    bg_image::BgImage,
};
use cgmath::Matrix3;
use egui::{Color32, Pos2, RichText, Ui};
use swarm_rs::{
    game::{BoardParams, BoardType, UpdateResult},
    vfs::Vfs,
};

const WINDOW_HEIGHT: f64 = 800.;

#[derive(Debug, PartialEq)]
enum Panel {
    Main,
    BTEditor,
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
                    agent: "green/agent.txt".to_owned(),
                    spawner: "green/spawner.txt".to_owned(),
                },
                BTSourceFiles {
                    agent: "red/agent.txt".to_owned(),
                    spawner: "red/spawner.txt".to_owned(),
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

        // "Consume" the error, since we don't have a good way to communicate the error on the startup of
        // the program. At least it will show on console if you run native build.
        let warn = |res| {
            if let Err(e) = res {
                eprintln!("WARNING: error on loading behavior tree: {e}")
            }
        };

        // Restore behavior tree from previous selection. We need this because we don't store the whole content
        // of the behavior tree in serialized eframe state. The behavior tree source code can be big.
        if let Some(vfs) = res.app_data.vfs.take() {
            for team in 0..2 {
                warn(res.app_data.apply_bt(
                    vfs.as_ref(),
                    &res.bt_source_file[team].agent,
                    (team, BtType::Agent),
                ));
                warn(res.app_data.apply_bt(
                    vfs.as_ref(),
                    &res.bt_source_file[team].spawner,
                    (team, BtType::Spawner),
                ));
            }
            res.app_data.vfs = Some(vfs);
        }

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

            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Checkbox::new(&mut self.show_labels, "Label image"));

                ui.add(egui::Checkbox::new(
                    &mut self.app_data.game.enable_raycast_board,
                    "Raycast image",
                ));
            });
        });

        ui.collapsing("Statistics", |ui| {
            let game = &mut self.app_data.game;

            if ui.button("Reset").clicked() {
                game.stats = Default::default();
            }

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

    fn show_editor(&mut self, ui: &mut Ui) {
        let team_colors = [Color32::GREEN, Color32::RED];

        if ui.button("Reset all").clicked() {
            self.app_data.set_confirm_message(
                "Are you sure you want to reset all the source codes?".to_string(),
                Box::new(|app_data| {
                    if let Some(ref mut vfs) = app_data.vfs {
                        if let Err(e) = vfs.reset() {
                            app_data.set_message(e);
                        }
                    }
                }),
            )
        }

        ui.horizontal(|ui| {
            ui.label("BT to apply:");
            for (team, color) in team_colors.into_iter().enumerate() {
                ui.radio_value(
                    &mut self.app_data.selected_bt,
                    (team, BtType::Agent),
                    RichText::new("Agent").color(color),
                );
                ui.radio_value(
                    &mut self.app_data.selected_bt,
                    (team, BtType::Spawner),
                    RichText::new("Spawner").color(color),
                );
            }
        });

        ui.collapsing("Files", |ui| {
            if let Some(mut vfs) = self.app_data.vfs.take() {
                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut self.app_data.new_file_name);
                    if ui.button("Save to a New file").clicked() {
                        if let Err(e) =
                            vfs.save_file(&self.app_data.new_file_name, &self.app_data.bt_buffer)
                        {
                            self.app_data.set_message(format!("Save file error! {e}"))
                        }
                    }
                });

                for item in vfs.list_files() {
                    ui.horizontal(|ui| {
                        let mut file_name = RichText::new(&format!(
                            "{}  {item}",
                            if self.app_data.current_file_name == item && self.app_data.dirty {
                                "*"
                            } else {
                                " "
                            }
                        ));
                        if self.app_data.current_file_name == item {
                            // TODO: use black in light theme
                            file_name = file_name.underline().color(Color32::WHITE);
                        }
                        if ui.label(file_name).interact(egui::Sense::click()).clicked() {
                            let item = item.clone();
                            let load = move |app_data: &mut AppData, vfs: &mut Box<dyn Vfs>| match vfs.get_file(&item) {
                                Ok(content) => {
                                    app_data.current_file_name = item.clone();
                                    app_data.bt_buffer = content;
                                    app_data.dirty = false;
                                }
                                Err(e) => {
                                    app_data.set_message(format!("Load file error!: {e}"))
                                }
                            };
                            if self.app_data.dirty {
                                self.app_data.set_confirm_message("The file is not saved. Is it ok to discard edits and load from file?".to_owned(), Box::new(move |app_data| {
                                    if let Some(mut vfs) = app_data.vfs.take() {
                                        load(app_data, &mut vfs);
                                        app_data.vfs = Some(vfs);
                                    }
                                }));
                            } else {
                                load(&mut self.app_data, &mut vfs);
                            }
                        }
                        if ui.button("Save").clicked() {
                            let item_copy = item.clone();
                            let save = move |app_data: &mut AppData, vfs: &mut Box<dyn Vfs>| {
                                if let Err(e) = vfs.save_file(&item_copy, &app_data.bt_buffer) {
                                    app_data.set_message(format!("Save file error! {e}"))
                                } else {
                                    app_data.dirty = false;
                                }
                            };
                            if self.app_data.current_file_name == item {
                                save(&mut self.app_data, &mut vfs);
                            } else {
                                self.app_data.set_confirm_message("You are going to write to a different file from original. Are you sure?".to_owned(), Box::new(move |app_data| {
                                    if let Some(mut vfs) = app_data.vfs.take() {
                                        save(app_data, &mut vfs);
                                        app_data.vfs = Some(vfs);
                                    }
                                }));
                            }
                        }
                        if ui.button("Delete").clicked() {
                            let item = item.clone();
                            self.app_data.set_confirm_message(
                                format!("Do you want to delete {item:?}?"),
                                Box::new(move |this: &mut AppData| {
                                    if let Some(mut vfs) = this.vfs.take() {
                                        match vfs.delete_file(&item) {
                                            Ok(_) => this.set_message(
                                                "File deleted successfully!".to_string(),
                                            ),
                                            Err(e) => {
                                                this.set_message(format!("Delete file error! {e}"))
                                            }
                                        }
                                        this.vfs = Some(vfs);
                                    }
                                }),
                            );
                        }
                        if ui.button("Apply").clicked() {
                            if self.app_data.dirty {
                                self.app_data.set_message(
                                    "Save the file before applying the behavior tree".to_owned(),
                                );
                            } else {
                                match self.app_data.apply_bt(vfs.as_ref(), &item, self.app_data.selected_bt) {
                                    Ok(()) => {
                                        let bt_source = &mut self.bt_source_file
                                            [self.app_data.selected_bt.0];
                                        *match self.app_data.selected_bt.1 {
                                            BtType::Agent => &mut bt_source.agent,
                                            BtType::Spawner => &mut bt_source.spawner,
                                        } = item.to_owned();
                                    }
                                    Err(e) => {
                                        if !e.detail.is_empty() {
                                            self.app_data.set_message_with_payload(e.title, e.detail);
                                        } else {
                                            self.app_data.set_message(e.title);
                                        }
                                    }
                                }
                            }
                        }
                        for (bt_sources, color) in
                            self.bt_source_file.iter().zip(team_colors.into_iter())
                        {
                            if Path::new(&item) == Path::new(&bt_sources.agent) {
                                ui.label(RichText::new("Agent").color(color));
                            }
                            if Path::new(&item) == Path::new(&bt_sources.spawner) {
                                ui.label(RichText::new("Spawner").color(color));
                            }
                        }
                    });
                }
                self.app_data.vfs = Some(vfs);
            }
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
            let source = &mut self.app_data.bt_buffer;
            if ui
                .add(
                    egui::TextEdit::multiline(source)
                        .font(egui::TextStyle::Monospace)
                        .code_editor()
                        .desired_rows(10)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY),
                )
                .changed()
            {
                self.app_data.dirty = true;
            };
        });
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

        self.app_data.show_message(ctx);

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
                    Panel::BTEditor,
                    "Behavior tree editor",
                );
            });

            match self.open_panel {
                Panel::Main => self.show_panel_ui(ui),
                Panel::BTEditor => self.show_editor(ui),
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.paint_game(ui);
        });
    }
}
