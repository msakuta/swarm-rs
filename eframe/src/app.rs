use std::{rc::Rc, time::Duration};

use cgmath::{InnerSpace, Matrix2, Matrix3, Point2, Rad, Transform, Vector2};
use eframe::epaint::{self, PathShape};
use egui::{pos2, Color32, Frame, Painter, Pos2, Rect, Response, Stroke, Ui, Vec2};
use swarm_rs::{
    agent::{AgentClass, AGENT_HALFLENGTH, BULLET_RADIUS},
    game::{BoardType, GameParams, Resource},
    qtree::FRESH_TICKS,
    AppData, Bullet, CellState,
};

use crate::bg_image::BgImage;

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
    canvas_offset: Pos2,
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
            app_data: AppData::new(),
            draw_circle: false,
            xs: 128,
            ys: 128,
            maze_expansions: 512,
            agent_count: 3,
            agent_source_file: "../behavior_tree_config/agent.txt".to_owned(),
            spawner_source_file: "../behavior_tree_config/spawner.txt".to_owned(),
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

/// Transform a vector (delta). Equivalent to `(m * v.extend(0.)).truncate()`.
fn transform_vector(m: &Matrix3<f64>, v: impl Into<Vector2<f64>>) -> Vector2<f64> {
    // Transform trait is implemented for both Point2 and Point3, so we need to repeat fully qualified method call
    <Matrix3<f64> as Transform<Point2<f64>>>::transform_vector(m, v.into())
}

/// Transform a point. Equivalent to `(m * v.extend(1.)).truncate()`.
fn transform_point(m: &Matrix3<f64>, v: impl Into<Point2<f64>>) -> Point2<f64> {
    // I don't really get the point of having the vector and the point as different types.
    <Matrix3<f64> as Transform<Point2<f64>>>::transform_point(m, v.into())
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

        self.app_data.update();

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
            struct UiResult {
                scroll_delta: Vec2,
                pointer: bool,
                delta: Vec2,
                interact_pos: Vector2<f64>,
            }

            let ui_result = {
                let input = ui.input();
                let interact_pos =
                    input.pointer.interact_pos().unwrap_or(Pos2::ZERO) - self.canvas_offset;
                UiResult {
                    scroll_delta: input.scroll_delta,
                    pointer: input.pointer.primary_down(),
                    delta: input.pointer.delta(),
                    interact_pos: Vector2::new(interact_pos.x as f64, interact_pos.y as f64),
                }
            };

            if ui.ui_contains_pointer() {
                if ui_result.scroll_delta[1] != 0. {
                    let old_offset =
                        transform_vector(&self.inverse_view_transform(), ui_result.interact_pos);
                    if ui_result.scroll_delta[1] < 0. {
                        self.app_data.scale /= 1.2;
                    } else if 0. < ui_result.scroll_delta[1] {
                        self.app_data.scale *= 1.2;
                    }
                    let new_offset =
                        transform_vector(&self.inverse_view_transform(), ui_result.interact_pos);
                    let diff: Vector2<f64> = new_offset - old_offset;
                    self.app_data.origin =
                        (Vector2::<f64>::from(self.app_data.origin) + diff).into();
                }

                if ui_result.pointer {
                    self.app_data.origin[0] += ui_result.delta[0] as f64 / self.app_data.scale;
                    self.app_data.origin[1] += ui_result.delta[1] as f64 / self.app_data.scale;
                }
            }

            // println!("scroll_delta: {scroll_delta:?}");

            Frame::canvas(ui.style()).show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::hover());

                self.canvas_offset = response.rect.min;

                if self.show_labels {
                    self.img_labels
                        .paint(&response, &painter, &self.app_data, |app_data| {
                            let (size, image) =
                                app_data.labeled_image().unwrap_or_else(|| ([0, 0], vec![]));
                            egui::ColorImage::from_rgb(size, &image)
                        });
                } else {
                    self.img_gray
                        .paint(&response, &painter, &self.app_data, |app_data| {
                            let (size, image) = app_data
                                .occupancy_image()
                                .unwrap_or_else(|| ([0, 0], vec![]));
                            let image = image
                                .into_iter()
                                .map(|b| std::iter::repeat(b).take(3))
                                .flatten()
                                .collect::<Vec<_>>();
                            egui::ColorImage::from_rgb(size, &image)
                        });
                }

                render_search_tree(&self.app_data, &response, &painter);

                paint_qtree(&response, &painter, &self.app_data);

                paint_resources(&response, &painter, &self.app_data);

                paint_agents(&response, &painter, self, &self.view_transform());

                paint_bullets(&response, &painter, &self.app_data);
            });
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

pub(crate) fn paint_qtree(response: &Response, painter: &Painter, data: &AppData) {
    if !data.qtree_visible {
        return;
    }
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    let scale = data.scale as f32;
    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32) * scale;

    data.with_qtree(|qtree_searcher| {
        const CELL_MARGIN: f32 = 0.1;

        let cache_map = qtree_searcher.get_cache_map();

        for (&[x, y], &freshness) in &cache_map.fresh_cells {
            let width = 1.;
            let cell_pos = Vec2::new(x as f32, y as f32);
            let min_margin = Vec2::splat(CELL_MARGIN);
            let max_margin = Vec2::splat(width as f32 - CELL_MARGIN);
            let rect = to_screen.transform_rect(Rect {
                min: ((cell_pos + min_margin) * scale + offset).to_pos2(),
                max: ((cell_pos + max_margin) * scale + offset).to_pos2(),
            });
            let color = match cache_map.get([x, y]) {
                CellState::Obstacle => (255, 127, 127),
                CellState::Occupied(_) => (255, 127, 255),
                CellState::Free => (0, 255, 127),
                _ => (255, 0, 255),
            };
            let brush = Color32::from_rgba_unmultiplied(
                color.0,
                color.1,
                color.2,
                (freshness * 127 / FRESH_TICKS) as u8,
            );
            painter.rect_filled(rect, 0., brush);
        }

        let qtree = qtree_searcher.get_qtree();

        for (level, cells) in qtree.levels.iter().enumerate() {
            let width = qtree.width(level);
            for (cell, state) in cells
                .iter()
                .filter(|(_, state)| !matches!(state, CellState::Mixed))
            {
                let (x, y) = (
                    cell[0] << (qtree.toplevel - level),
                    cell[1] << (qtree.toplevel - level),
                );
                let cell_pos = Vec2::new(x as f32, y as f32);
                let min_margin = Vec2::splat(CELL_MARGIN);
                let max_margin = Vec2::splat(width as f32 - CELL_MARGIN);
                let rect = Rect {
                    min: ((cell_pos + min_margin) * scale + offset).to_pos2(),
                    max: ((cell_pos + max_margin) * scale + offset).to_pos2(),
                };
                let rect = to_screen.transform_rect(rect);
                // let rect = rect.to_path(1.);
                painter.rect_stroke(
                    rect,
                    0.,
                    Stroke {
                        width: 1.,
                        color: match state {
                            CellState::Obstacle => Color32::from_rgb(255, 127, 127),
                            CellState::Occupied(_) => Color32::from_rgb(255, 127, 255),
                            CellState::Free => Color32::from_rgb(0, 255, 127),
                            _ => Color32::from_rgb(255, 0, 255),
                        },
                    },
                );
            }
        }
    });
}

fn render_search_tree(data: &AppData, response: &Response, painter: &Painter) {
    if !data.qtree_search_visible {
        return;
    }

    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    let game = data.game.borrow();
    let entities = &game.entities;
    for entity in entities {
        let Ok(entity) = entity.try_borrow() else {
            continue;
        };

        let Some(st) = entity.get_search_tree() else {
            continue;
        };

        let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

        let to_point = |pos: [f64; 2]| {
            let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
            to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
        };

        let brush = Color32::WHITE;
        let nodes = st.get_nodes();
        for [start, end] in st.get_edges() {
            painter.line_segment(
                [to_point(nodes[*start]), to_point(nodes[*end])],
                (1., brush),
            );
        }
    }
}

fn paint_agents(
    response: &Response,
    painter: &Painter,
    app: &TemplateApp,
    view_transform: &Matrix3<f64>,
) {
    let data = &app.app_data;
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    const AGENT_COLORS: [Color32; 2] = [
        Color32::from_rgb(0, 255, 127),
        Color32::from_rgb(255, 0, 63),
    ];

    let game = data.game.borrow();

    let entities = &game.entities;

    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

    let draw_rectangle = 1. / AGENT_HALFLENGTH < data.scale;

    let to_point = |pos: [f64; 2]| {
        let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
    };

    for agent in entities.iter() {
        let agent = agent.borrow();
        let agent_pos = agent.get_pos();
        let pos = to_point(agent_pos);
        let brush = AGENT_COLORS[agent.get_team() % AGENT_COLORS.len()];
        painter.circle_filled(pos, 5., brush);

        if !agent.is_agent() {
            painter.circle_stroke(
                pos,
                10.,
                Stroke {
                    color: brush,
                    width: 3.,
                },
            );
        }

        let resource = agent.resource();
        if 0 < resource {
            use std::f64::consts::PI;
            let f = resource as f64 * 2. * PI / agent.max_resource() as f64;
            let count = 10; // There is no reason to pick this value, but it seems to work fine.
            for (i0, i1) in (0..count).zip(1..=count) {
                let theta0 = (i0 as f64 / count as f64 * f) as f32;
                let theta1 = (i1 as f64 / count as f64 * f) as f32;
                let p0 = Vec2::new(theta0.cos(), theta0.sin()) * 7.5 + pos.to_vec2();
                let p1 = Vec2::new(theta1.cos(), theta1.sin()) * 7.5 + pos.to_vec2();
                painter.line_segment(
                    [p0.to_pos2(), p1.to_pos2()],
                    Stroke {
                        color: Color32::YELLOW,
                        width: 2.5,
                    },
                );
            }
        }

        let agent_pos = agent.get_pos();
        let agent_pos = Vector2::from(agent_pos);
        let view_pos = to_point(agent_pos.into());

        if let Some(orient) = agent.get_orient() {
            let class = agent.get_class().unwrap_or(AgentClass::Worker);
            let length = if matches!(class, AgentClass::Fighter) {
                20.
            } else {
                10.
            };
            let dest = egui::pos2(
                view_pos.x + (orient.cos() * length) as f32,
                view_pos.y + (orient.sin() * length) as f32,
            );
            let orient_line = [view_pos, dest];
            painter.line_segment(
                orient_line,
                Stroke {
                    color: brush,
                    width: 3.,
                },
            );

            if draw_rectangle {
                let mut path = vec![];
                let rotation = Matrix2::from_angle(Rad(orient));
                class.vertices(|v| {
                    let vertex = rotation * Vector2::from(v) + agent_pos;
                    path.push(to_point(vertex.into()));
                });
                painter.add(PathShape::closed_line(path, (1., brush)));
            }
        } else {
            let aabb = agent.get_aabb();
            let rect = Rect {
                min: to_point([aabb[0], aabb[1]]),
                max: to_point([aabb[2], aabb[3]]),
            };
            painter.rect_stroke(
                rect,
                0.,
                Stroke {
                    color: brush,
                    width: 1.,
                },
            );
        }

        if data.path_visible {
            let mut path = 'avoidance_path: {
                let Some(path) = agent
                    .get_avoidance_path_array() else {
                        break 'avoidance_path vec![];
                    };
                if path.len() == 0 {
                    break 'avoidance_path vec![];
                }
                let path = if let Some(goal) = agent.get_goal() {
                    path.iter()
                        .copied()
                        .chain(std::iter::once([goal.x, goal.y]))
                        .map(to_point)
                        .collect::<Vec<Pos2>>()
                } else {
                    path.iter().copied().map(to_point).collect::<Vec<Pos2>>()
                };
                path
            };
            if let Some(global_path) = agent.get_path() {
                path.extend(global_path.iter().map(|node| to_point(node.pos)));
                path.push(view_pos);
                if app.draw_circle {
                    for point in global_path {
                        let circle = to_point(point.pos);
                        painter.circle_stroke(
                            circle,
                            (point.radius * data.scale) as f32,
                            (1., brush),
                        );
                    }
                }
            }
            painter.add(PathShape::line(path, (1., brush)));
        }

        if 5. < data.scale {
            let health = agent.get_health_rate() as f32;
            let view_pos_left =
                transform_point(view_transform, [agent_pos.x - 1., agent_pos.y - 1.]);
            let view_pos_right =
                transform_point(view_transform, [agent_pos.x + 1., agent_pos.y - 1.]);
            if matches!(agent.get_class(), Some(AgentClass::Fighter)) {
                let base =
                    pos2(view_pos_left.x as f32, view_pos_left.y as f32) + Vec2::new(8., -5.);
                let cross = vec![
                    to_screen.transform_pos(base + Vec2::new(-8., -25.)),
                    to_screen.transform_pos(base + Vec2::new(0., -20.)),
                    to_screen.transform_pos(base + Vec2::new(8., -25.)),
                    to_screen.transform_pos(base + Vec2::new(8., -20.)),
                    to_screen.transform_pos(base + Vec2::new(0., -15.)),
                    to_screen.transform_pos(base + Vec2::new(-8., -20.)),
                ];
                painter.add(PathShape::convex_polygon(
                    vec![cross[0], cross[1], cross[4], cross[5]],
                    brush,
                    Stroke::NONE,
                ));
                painter.add(PathShape::convex_polygon(
                    vec![cross[1], cross[2], cross[3], cross[4]],
                    brush,
                    Stroke::NONE,
                ));
                painter.add(PathShape::closed_line(cross, (1., Color32::YELLOW)));
            }
            let l = (view_pos_left.x) as f32;
            let r = (view_pos_right.x) as f32;
            let t = (view_pos_left.y - 15.) as f32;
            let b = (view_pos_left.y - 10.) as f32;
            let rect = Rect {
                min: pos2(l, t),
                max: pos2(r, b),
            };
            painter.rect_filled(to_screen.transform_rect(rect), 0., Color32::RED);
            let health_rect = Rect {
                min: pos2(l, t),
                max: pos2(l + health * (r - l), b),
            };
            painter.rect_filled(
                to_screen.transform_rect(health_rect),
                0.,
                Color32::from_rgb(0, 191, 0),
            );
        }
    }
}

fn paint_bullets(response: &Response, painter: &Painter, data: &AppData) {
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

    let to_point = |pos: [f64; 2]| {
        let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
    };

    let draw_bullet = |painter: &Painter, bullet: &Bullet, radius: f64| {
        painter.circle(
            to_point(bullet.pos),
            radius as f32,
            if bullet.team == 0 {
                Color32::WHITE
            } else {
                Color32::from_rgb(255, 0, 255)
            },
            Stroke {
                color: Color32::YELLOW,
                width: 1.,
            },
        );
    };

    const TARGET_PIXELS: f64 = 3.;

    let game = data.game.borrow();

    let draw_small = data.scale < TARGET_PIXELS / BULLET_RADIUS;

    for bullet in game.bullets.iter() {
        let pos = Vector2::from(bullet.pos);
        let velo = Vector2::from(bullet.velo).normalize();
        let length = bullet
            .traveled
            .min(2. * Vector2::from(bullet.velo).magnitude());
        let tail = pos - velo * length;
        if matches!(bullet.shooter_class, AgentClass::Fighter) {
            let perp = Vector2::new(velo.y, -velo.x) * BULLET_RADIUS;
            let trail = epaint::PathShape {
                points: vec![
                    to_point((pos + perp).into()),
                    to_point((pos - perp).into()),
                    to_point(tail.into()),
                ],
                closed: true,
                fill: Color32::from_rgb(255, 191, 63),
                stroke: Default::default(),
            };
            painter.add(trail);
            if !draw_small {
                draw_bullet(painter, bullet, BULLET_RADIUS * data.scale);
            }
        } else {
            let trail = [to_point((pos + velo).into()), to_point((pos - velo).into())];
            painter.line_segment(
                trail,
                Stroke {
                    color: Color32::from_rgb(255, 191, 63),
                    width: 0.075 * data.scale as f32,
                },
            );
        };
    }

    if draw_small {
        for bullet in game.bullets.iter() {
            if matches!(bullet.shooter_class, AgentClass::Fighter) {
                draw_bullet(painter, bullet, TARGET_PIXELS);
            }
        }
    }
}

fn paint_resources(response: &Response, painter: &Painter, data: &AppData) {
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    const TARGET_PIXELS: f64 = 10.;

    let draw_resource = |resource: &Resource, pos: Pos2| {
        let radius = ((resource.amount as f64).sqrt() / TARGET_PIXELS * data.scale) as f32;
        painter.circle_filled(pos, radius, Color32::YELLOW);
    };

    let game = data.game.borrow();

    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

    let to_point = |pos: [f64; 2]| {
        let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
    };

    for resource in &game.resources {
        draw_resource(resource, to_point(resource.pos));
    }
}
