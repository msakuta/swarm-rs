mod paint_game;

use std::{rc::Rc, time::Duration};

use crate::{app_data::AppData, bg_image::BgImage};
use cgmath::Matrix3;
use egui::{Pos2, Ui};
use swarm_rs::game::{BoardType, GameParams};

const WINDOW_HEIGHT: f64 = 800.;
const AGENT_SOURCE_FILE: &'static str = "behavior_tree_config/agent.txt";
const SPAWNER_SOURCE_FILE: &'static str = "behavior_tree_config/spawner.txt";

#[derive(Debug, PartialEq)]
enum Panel {
    Main,
    AgentEditor,
    SpawnerEditor,
}

/// We derive Deserialize/Serialize so we can persist app state on shutdown.
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(default)] // if we add new fields, give them default values when deserializing old state
pub struct TemplateApp {
    // Example stuff:
    label: String,

    // this how you opt-out of serialization of a member
    #[serde(skip)]
    value: f32,

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

    xs: usize,
    ys: usize,
    maze_expansions: usize,
    agent_count: usize,

    agent_source_file: String,
    spawner_source_file: String,

    #[serde(skip)]
    pub(crate) canvas_offset: Pos2,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            img_gray: BgImage::new(),
            img_labels: BgImage::new(),
            open_panel: Panel::Main,
            show_labels: false,
            app_data: AppData::new(WINDOW_HEIGHT),
            draw_circle: false,
            xs: 128,
            ys: 128,
            maze_expansions: 512,
            agent_count: 3,
            agent_source_file: AGENT_SOURCE_FILE.to_owned(),
            spawner_source_file: SPAWNER_SOURCE_FILE.to_owned(),
            canvas_offset: Pos2::ZERO,
        }
    }
}

impl TemplateApp {
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
        if let Some(storage) = cc.storage {
            return eframe::get_value(storage, eframe::APP_KEY).unwrap_or_default();
        }

        Default::default()
    }

    fn show_panel_ui(&mut self, ui: &mut Ui) {
        ui.heading("Side Panel");

        ui.add(egui::Checkbox::new(
            &mut self.app_data.game_params.paused,
            "Paused",
        ));

        ui.group(|ui| {
            ui.label("New game options");

            if ui.button("New game").clicked() {
                self.app_data.xs_text = self.xs.to_string();
                self.app_data.ys_text = self.ys.to_string();
                self.app_data.maze_expansions = self.maze_expansions.to_string();
                self.app_data.new_game();
                self.img_gray.clear();
                self.img_labels.clear();
            }

            ui.radio_value(&mut self.app_data.board_type, BoardType::Rect, "Rect");
            ui.radio_value(&mut self.app_data.board_type, BoardType::Crank, "Crank");
            ui.radio_value(&mut self.app_data.board_type, BoardType::Perlin, "Perlin");
            ui.radio_value(&mut self.app_data.board_type, BoardType::Rooms, "Rooms");
            ui.radio_value(&mut self.app_data.board_type, BoardType::Maze, "Maze");

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
                ui.text_edit_singleline(&mut self.app_data.seed_text);
            });
            ui.horizontal(|ui| {
                ui.label("Maze expansion");
                ui.add(egui::Slider::new(&mut self.maze_expansions, 32..=1024));
            });

            ui.horizontal(|ui| {
                ui.label("Agents");
                ui.add(egui::Slider::new(&mut self.agent_count, 1..=100));
                self.app_data.agent_count_text = self.agent_count.to_string();
            });
        });

        ui.group(|ui| {
            ui.label("View options");

            ui.group(|ui| {
                ui.add(egui::Checkbox::new(&mut self.app_data.path_visible, "Path"));

                ui.add(egui::Checkbox::new(&mut self.draw_circle, "Circle"));
            });

            ui.add(egui::Checkbox::new(
                &mut self.app_data.qtree_visible,
                "QTree",
            ));

            ui.add(egui::Checkbox::new(
                &mut self.app_data.qtree_search_visible,
                "QTree search",
            ));

            ui.add(egui::Checkbox::new(
                &mut self.app_data.target_visible,
                "Target line",
            ));

            ui.add(egui::Checkbox::new(
                &mut self.app_data.entity_trace_visible,
                "Trace line",
            ));

            ui.add(egui::Checkbox::new(
                &mut self.app_data.entity_label_visible,
                "Entity labels",
            ));

            ui.add(egui::Checkbox::new(&mut self.show_labels, "Label image"));
        });
    }

    fn show_editor(
        &mut self,
        ui: &mut Ui,
        contents: impl Fn(&Self) -> Rc<String>,
        contents_mut: fn(&mut AppData) -> &mut Rc<String>,
        game_params_mut: fn(&mut GameParams) -> &mut Rc<String>,
        file: impl Fn(&Self) -> &str,
        mut file_mut: impl FnMut(&mut Self) -> &mut String,
    ) {
        ui.label(&self.app_data.message);

        ui.horizontal(|ui| {
            if ui.button("Apply").clicked() {
                self.app_data
                    .try_load_behavior_tree(contents(self).clone(), game_params_mut);
            }
            ui.text_edit_singleline(file_mut(self));
            if ui.button("Reload from file").clicked() {
                let file = file(self).to_owned();
                self.app_data
                    .try_load_from_file(&file, contents_mut, game_params_mut);
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
}

impl eframe::App for TemplateApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(16));

        let dt = ctx.input().stable_dt.min(0.1);

        self.app_data.update(dt as f64 * 1000.);

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
                    Panel::AgentEditor,
                    "Agent behavior tree",
                );
                ui.selectable_value(
                    &mut self.open_panel,
                    Panel::SpawnerEditor,
                    "Spawner behavior tree",
                );
            });
            match self.open_panel {
                Panel::Main => self.show_panel_ui(ui),
                Panel::AgentEditor => self.show_editor(
                    ui,
                    |app_data| app_data.app_data.agent_source_buffer.clone(),
                    |app_data| &mut app_data.agent_source_buffer,
                    |params| &mut params.agent_source,
                    |app_data| &app_data.agent_source_file,
                    |app_data| &mut app_data.agent_source_file,
                ),
                Panel::SpawnerEditor => self.show_editor(
                    ui,
                    |app_data| app_data.app_data.spawner_source_buffer.clone(),
                    |app_data| &mut app_data.spawner_source_buffer,
                    |params| &mut params.spawner_source,
                    |app_data| &app_data.spawner_source_file,
                    |app_data| &mut app_data.spawner_source_file,
                ),
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
