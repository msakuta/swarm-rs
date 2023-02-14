use std::time::Duration;

use cgmath::{InnerSpace, Matrix2, Matrix3, Point2, Rad, Transform, Vector2};
use eframe::epaint;
use egui::{pos2, Color32, Frame, Painter, Pos2, Rect, Response, Stroke, TextureOptions, Vec2};
use swarm_rs::{
    agent::{AgentClass, AGENT_HALFLENGTH, BULLET_RADIUS},
    game::Resource,
    AppData, Bullet, CellState,
};

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

    #[serde(skip)]
    canvas_offset: Pos2,
}

impl Default for TemplateApp {
    fn default() -> Self {
        Self {
            // Example stuff:
            label: "Hello World!".to_owned(),
            value: 2.7,
            img: MyImage::new(),
            app_data: AppData::new(),
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
        let (scroll_delta, pointer, delta, interact_pos) = {
            let input = ctx.input();
            let interact_pos =
                input.pointer.interact_pos().unwrap_or(Pos2::ZERO) - self.canvas_offset;
            (
                input.scroll_delta,
                input.pointer.primary_down(),
                input.pointer.delta(),
                Vector2::new(interact_pos.x as f64, interact_pos.y as f64),
            )
        };

        if scroll_delta[1] != 0. {
            let old_offset = transform_vector(&self.inverse_view_transform(), interact_pos);
            if scroll_delta[1] < 0. {
                self.app_data.scale /= 1.2;
            } else if 0. < scroll_delta[1] {
                self.app_data.scale *= 1.2;
            }
            let new_offset = transform_vector(&self.inverse_view_transform(), interact_pos);
            let diff: Vector2<f64> = new_offset - old_offset;
            self.app_data.origin = (Vector2::<f64>::from(self.app_data.origin) + diff).into();
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

            ui.add(egui::Checkbox::new(
                &mut self.app_data.game_params.paused,
                "Paused",
            ));

            ui.add(egui::Checkbox::new(&mut self.app_data.path_visible, "Path"));

            ui.add(egui::Checkbox::new(
                &mut self.app_data.qtree_visible,
                "QTree",
            ));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            // The central panel the region left after adding TopPanel's and SidePanel's

            Frame::canvas(ui.style()).show(ui, |ui| {
                let (response, painter) =
                    ui.allocate_painter(ui.available_size(), egui::Sense::hover());

                self.canvas_offset = response.rect.min;

                self.img.paint(&response, &painter, &self.app_data);

                paint_qtree(&response, &painter, &self.app_data);

                paint_resources(&response, &painter, &self.app_data);

                paint_agents(&response, &painter, &self.app_data, &self.view_transform());

                paint_bullets(&response, &painter, &self.app_data);
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
                TextureOptions {
                    magnification: egui::TextureFilter::Nearest,
                    minification: egui::TextureFilter::Linear,
                },
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

fn paint_agents(
    response: &Response,
    painter: &Painter,
    data: &AppData,
    view_transform: &Matrix3<f64>,
) {
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

        let render_ring = |path: &[Pos2]| {
            if let Some(first) = path.first() {
                for (p0, p1) in path
                    .iter()
                    .zip(path.iter().skip(1).chain(std::iter::once(first)))
                {
                    painter.line_segment(
                        [*p0, *p1],
                        Stroke {
                            color: brush,
                            width: 1.,
                        },
                    );
                }
            }
        };

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
                render_ring(&path);
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

        let render_line_string = |path: &[Pos2]| {
            for (p0, p1) in path.iter().zip(path.iter().skip(1)) {
                painter.line_segment(
                    [*p0, *p1],
                    Stroke {
                        color: brush,
                        width: 1.,
                    },
                );
            }
        };

        if data.path_visible {
            let avoidance_drawn = 'breaky: {
                let Some(path) = agent
                    .get_avoidance_path_array() else {
                        break 'breaky None;
                    };
                if path.len() == 0 {
                    break 'breaky None;
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
                render_line_string(&path);
                path.last().copied()
            };
            if let Some(path) = agent.get_path() {
                let mut bez_path = if let Some(first) = avoidance_drawn {
                    vec![first]
                } else {
                    vec![]
                };
                bez_path.extend(path.iter().map(|node| to_point(node.pos)));
                bez_path.push(view_pos);
                render_line_string(&bez_path);
                // if draw_circle {
                //     let circle = Circle::new(to_point(point.pos), point.radius);
                //     ctx.stroke(*view_transform * circle, brush, 1.);
                //     bez_path.line_to(to_point(point.pos));
                // }
            }
        }

        if 5. < data.scale {
            let health = agent.get_health_rate() as f32;
            let view_pos_left =
                transform_point(view_transform, [agent_pos.x - 1., agent_pos.y - 1.]);
            let view_pos_right =
                transform_point(view_transform, [agent_pos.x + 1., agent_pos.y - 1.]);
            // if matches!(agent.get_class(), Some(AgentClass::Fighter)) {
            //     let mut cross = BezPath::new();
            //     let base = view_pos_left + Vec2::new(8., -5.);
            //     cross.move_to(base + Vec2::new(-8., -25.));
            //     cross.line_to(base + Vec2::new(0., -20.));
            //     cross.line_to(base + Vec2::new(8., -25.));
            //     cross.line_to(base + Vec2::new(8., -20.));
            //     cross.line_to(base + Vec2::new(0., -15.));
            //     cross.line_to(base + Vec2::new(-8., -20.));
            //     cross.close_path();
            //     ctx.fill(&cross, &Color::YELLOW);
            //     ctx.stroke(cross, brush, 1.);
            // }
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
