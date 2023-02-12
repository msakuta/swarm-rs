use std::time::Duration;

use egui::{Color32, Frame, Painter, Pos2, Rect, Response, Stroke, Vec2};
use swarm_rs::{agent::AGENT_HALFLENGTH, game::Resource, AppData, CellState};

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
    img: MyImage,

    #[serde(skip)]
    app_data: AppData,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            img: MyImage::new(),
            app_data: AppData::new(),
        }
    }
}

impl TemplateApp {
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

        let Self { label, value, .. } = self;

        self.app_data.update();
        let (scroll_delta, pointer, delta) = {
            let input = ctx.input();
            (
                input.scroll_delta,
                input.pointer.primary_down(),
                input.pointer.delta(),
            )
        };
        if scroll_delta[1] < 0. {
            self.app_data.scale /= 1.2;
        } else if 0. < scroll_delta[1] {
            self.app_data.scale *= 1.2;
        }
        if pointer {
            self.app_data.origin[0] += delta[0] as f64 / self.app_data.scale;
            self.app_data.origin[1] += delta[1] as f64 / self.app_data.scale;
        }

        // println!("scroll_delta: {scroll_delta:?}");

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

        egui::SidePanel::left("side_panel").show(ctx, |ui| {
            ui.heading("Side Panel");

            ui.horizontal(|ui| {
                ui.label("Write something: ");
                ui.text_edit_singleline(label);
            });

            ui.add(egui::Slider::new(value, 0.0..=10.0).text("value"));
            if ui.button("Increment").clicked() {
                *value += 1.0;
            }
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            Frame::canvas(ui.style()).show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::hover());

                self.img.paint(&response, &painter, &self.app_data);

                paint_qtree(&response, &painter, &self.app_data);

                paint_resources(&response, &painter, &self.app_data);

                paint_agents(&response, &painter, &self.app_data);
            });

            // for y in 0..3 {
            //     let fy = y as f32 * 100. + 200.;
            //     for x in 0..3 {
            //         let fx = x as f32 * 100. + 200.;
            //         ui.painter().rect_stroke(
            //             Rect {
            //                 min: Pos2 { x: fx, y: fy },
            //                 max: Pos2 {
            //                     x: fx + 90.,
            //                     y: fy + 90.,
            //                 },
            //             },
            //             0.,
            //             Stroke {
            //                 width: 3.0,
            //                 color: Color32::RED,
            //             },
            //         );
            //     }
            // }
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

struct MyImage {
    texture: Option<egui::TextureHandle>,
}

impl MyImage {
    fn new() -> Self {
        Self { texture: None }
    }

    fn paint(&mut self, response: &Response, painter: &Painter, app_data: &AppData) {
        let texture: &egui::TextureHandle = self.texture.get_or_insert_with(|| {
            let (size, image) = app_data.labeled_image().unwrap_or_else(|| ([0, 0], vec![]));
            // Load the texture only once.
            painter.ctx().load_texture(
                "my-image",
                egui::ColorImage::from_rgb(size, &image),
                Default::default(),
            )
        });

        let to_screen = egui::emath::RectTransform::from_to(
            Rect::from_min_size(Pos2::ZERO, response.rect.size()),
            response.rect,
        );

        let size = texture.size_vec2() * app_data.scale as f32;
        let min =
            Vec2::new(app_data.origin[0] as f32, app_data.origin[1] as f32) * app_data.scale as f32;
        let max = min + size;
        let rect = Rect {
            min: min.to_pos2(),
            max: max.to_pos2(),
        };
        const UV: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        painter.image(
            texture.id(),
            to_screen.transform_rect(rect),
            UV,
            Color32::WHITE,
        );
    }
}

pub(crate) fn paint_qtree(response: &Response, painter: &Painter, data: &AppData) {
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    let scale = data.scale as f32;
    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32) * scale;

    data.with_qtree(|qtree_searcher| {
        let width = 1;
        const CELL_MARGIN: f32 = 0.1;

        // for (&[x, y], &freshness) in &qtree_searcher.get_cache_map().fresh_cells {
        //     let rect = Rect::new(
        //         x as f64 + CELL_MARGIN,
        //         y as f64 + CELL_MARGIN,
        //         x as f64 + width as f64 - CELL_MARGIN,
        //         y as f64 + width as f64 - CELL_MARGIN,
        //     );
        //     let rect = rect.to_path(1.);
        //     let color = match qtree_searcher.cache_map.get([x, y]) {
        //         CellState::Obstacle => (255, 127, 127),
        //         CellState::Occupied(_) => (255, 127, 255),
        //         CellState::Free => (0, 255, 127),
        //         _ => (255, 0, 255),
        //     };
        //     let brush = Color::rgba8(
        //         color.0,
        //         color.1,
        //         color.2,
        //         (freshness * 127 / FRESH_TICKS) as u8,
        //     );
        //     ctx.fill(*view_transform * rect, &brush);
        // }

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

fn paint_agents(response: &Response, painter: &Painter, data: &AppData) {
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    const AGENT_COLORS: [Color32; 2] = [
        Color32::from_rgb(0, 255, 127),
        Color32::from_rgb(255, 0, 63),
    ];

    let game = data.game.borrow();
    // let draw_rectangle = 1. / AGENT_HALFLENGTH < data.scale;
    let entities = &game.entities;

    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

    let to_point = |pos: [f64; 2]| {
        let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
    };

    for agent in entities.iter() {
        let agent = agent.borrow();
        let pos = to_point(agent.get_pos());
        // let circle = Circle::new(*view_transform * pos, 5.);
        let brush = AGENT_COLORS[agent.get_team() % AGENT_COLORS.len()];
        // ctx.fill(circle, brush);
        painter.circle_filled(pos, 5., brush);

        if !agent.is_agent() {
            painter.circle_stroke(pos, 10., Stroke {
                color: brush,
                width: 3.,
            });
        }

        // let resource = agent.resource();
        // if 0 < resource {
        //     use std::f64::consts::PI;
        //     let arc = Arc {
        //         center: *view_transform * pos,
        //         radii: Vec2::new(7.5, 7.5),
        //         start_angle: 0.,
        //         sweep_angle: resource as f64 * 2. * PI / agent.max_resource() as f64,
        //         x_rotation: -PI / 2.,
        //     };
        //     ctx.stroke(arc, &Color::YELLOW, 2.5);
        // }
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
        // let circle = Circle::new(pos, radius);
        // ctx.fill(circle, &Color::YELLOW);
        // ctx.stroke(circle, &Color::YELLOW, radius / 30.);
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
