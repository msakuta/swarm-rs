use std::cell::RefCell;

use crate::app_data::AppData;
use cgmath::{InnerSpace, Matrix2, Matrix3, MetricSpace, Point2, Rad, Transform, Vector2};
use eframe::{
    emath::RectTransform,
    epaint::{self, PathShape},
};
use egui::{pos2, Align2, Color32, FontId, Frame, Painter, Pos2, Rect, Response, Stroke, Ui, Vec2};
use swarm_rs::{
    agent::{AgentClass, AGENT_HALFLENGTH, BULLET_RADIUS},
    entity::Entity,
    game::Resource,
    qtree::FRESH_TICKS,
    Bullet, CellState,
};

use super::SwarmRsApp;

/// Transform a vector (delta). Equivalent to `(m * v.extend(0.)).truncate()`.
fn _transform_vector(m: &Matrix3<f64>, v: impl Into<Vector2<f64>>) -> Vector2<f64> {
    // Transform trait is implemented for both Point2 and Point3, so we need to repeat fully qualified method call
    <Matrix3<f64> as Transform<Point2<f64>>>::transform_vector(m, v.into())
}

/// Transform a point. Equivalent to `(m * v.extend(1.)).truncate()`.
fn transform_point(m: &Matrix3<f64>, v: impl Into<Point2<f64>>) -> Point2<f64> {
    // I don't really get the point of having the vector and the point as different types.
    <Matrix3<f64> as Transform<Point2<f64>>>::transform_point(m, v.into())
}

/// In points
const SCREEN_SELECT_RADIUS: f64 = 20.;

impl SwarmRsApp {
    pub(crate) fn paint_game(&mut self, ui: &mut Ui) {
        struct UiResult {
            scroll_delta: f32,
            zoom_delta: f32,
            pointer: bool,
            delta: Vec2,
            interact_pos: Point2<f64>,
            hover_pos: Option<Pos2>,
            clicked: bool,
        }

        let ui_result = {
            let input = ui.input();
            let interact_pos =
                input.pointer.interact_pos().unwrap_or(Pos2::ZERO) - self.canvas_offset;

            UiResult {
                scroll_delta: input.scroll_delta[1],
                zoom_delta: if input.multi_touch().is_some() {
                    input.zoom_delta()
                } else {
                    1.
                },
                pointer: input.pointer.primary_down(),
                delta: input.pointer.delta(),
                interact_pos: Point2::new(interact_pos.x as f64, interact_pos.y as f64),
                hover_pos: input.pointer.hover_pos(),
                clicked: input.pointer.primary_released(),
            }
        };

        if ui.ui_contains_pointer() {
            if ui_result.scroll_delta != 0. || ui_result.zoom_delta != 1. {
                let old_offset =
                    transform_point(&self.inverse_view_transform(), ui_result.interact_pos);
                if ui_result.scroll_delta < 0. {
                    self.app_data.scale /= 1.2;
                } else if 0. < ui_result.scroll_delta {
                    self.app_data.scale *= 1.2;
                } else if ui_result.zoom_delta != 1. {
                    self.app_data.scale *= ui_result.zoom_delta as f64;
                }
                let new_offset =
                    transform_point(&self.inverse_view_transform(), ui_result.interact_pos);
                let diff = new_offset - old_offset;
                self.app_data.origin = (Vector2::<f64>::from(self.app_data.origin) + diff).into();
            }

            if ui_result.pointer {
                self.app_data.origin[0] += ui_result.delta[0] as f64 / self.app_data.scale;
                self.app_data.origin[1] += ui_result.delta[1] as f64 / self.app_data.scale;
            }

            if ui_result.clicked {
                let view_transform = self.view_transform();
                self.app_data.selected_entity = self
                    .app_data
                    .game
                    .entities
                    .iter()
                    .find(|entity| {
                        let entity = entity.borrow();
                        let pos = Point2::from(entity.get_pos());
                        let screen_pos = transform_point(&view_transform, pos);
                        screen_pos.distance2(ui_result.interact_pos) < SCREEN_SELECT_RADIUS.powf(2.)
                    })
                    .map(|entity| entity.borrow().get_id());
                println!(
                    "Clicked {:?}, selected {:?}",
                    self.mouse_pos, self.app_data.selected_entity,
                );
            }
        }

        Frame::canvas(ui.style()).show(ui, |ui| {
            let (response, painter) =
                ui.allocate_painter(ui.available_size(), egui::Sense::hover());

            self.canvas_offset = response.rect.min;

            self.mouse_pos = ui_result.hover_pos.map(|pos| {
                let from_screen = egui::emath::RectTransform::from_to(
                    response.rect,
                    Rect::from_min_size(Pos2::ZERO, response.rect.size()),
                );
                let pos = from_screen.transform_pos(pos);
                let point = Point2::new(pos.x as f64, pos.y as f64);
                let transformed = transform_point(&self.inverse_view_transform(), point);
                pos2(transformed.x as f32, transformed.y as f32)
            });

            let image_getter = |app_data: &AppData| {
                let (size, image) = app_data
                    .game
                    .occupancy_image(&app_data.fog_active, app_data.colored_fog)
                    .unwrap_or_else(|| ([0, 0], vec![]));
                egui::ColorImage::from_rgb(size, &image)
            };

            if self.show_labels {
                self.img_labels
                    .paint(&response, &painter, &self.app_data, |app_data| {
                        let (size, image) = app_data
                            .game
                            .labeled_image()
                            .unwrap_or_else(|| ([0, 0], vec![]));
                        egui::ColorImage::from_rgb(size, &image)
                    });
            } else if self.app_data.game.enable_raycast_board {
                let raycast_board = self.app_data.game.raycast_board.borrow();
                let ray_dirty = raycast_board.iter().any(|p| *p != 0);
                let ray_valid = raycast_board.len() == self.app_data.game.board.len();
                if ray_dirty || true {
                    self.img_gray.clear();
                }
                if ray_valid {
                    self.img_gray.paint(
                        &response,
                        &painter,
                        &self.app_data,
                        |app_data: &AppData| {
                            let (size, image) = app_data
                                .game
                                .occupancy_image(&app_data.fog_active, app_data.colored_fog)
                                .unwrap_or_else(|| ([0, 0], vec![]));
                            let image = image
                                .into_iter()
                                .zip(raycast_board.iter())
                                .map(|(b, r)| {
                                    let br = b.saturating_add(r * 32);
                                    [br, br, b]
                                })
                                .flatten()
                                .collect::<Vec<_>>();
                            egui::ColorImage::from_rgb(size, &image)
                        },
                    )
                } else {
                    self.img_gray
                        .paint(&response, &painter, &self.app_data, image_getter)
                };
            } else {
                self.img_gray.clear();
                self.img_gray
                    .paint(&response, &painter, &self.app_data, image_getter);
            }

            render_search_tree(&self.app_data, &response, &painter);

            paint_qtree(&response, &painter, &self.app_data);

            paint_resources(&response, &painter, &self.app_data);

            paint_agents((&response, &painter), self, &self.view_transform());

            paint_bullets(&response, &painter, &self.app_data);

            paint_big_message(&response, &painter, &self.app_data, ui.available_size());
        });
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

    let game = &data.game;
    let entities = &game.entities;
    for entity in entities {
        let Ok(entity) = entity.try_borrow() else {
            continue;
        };

        if data
            .selected_entity
            .map(|id| id != entity.get_id())
            .unwrap_or(false)
        {
            continue;
        }

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

const AGENT_COLORS: [Color32; 2] = [
    Color32::from_rgb(0, 255, 127),
    Color32::from_rgb(255, 0, 63),
];

const SELECTED_COLOR: Color32 = Color32::WHITE;

fn paint_agents(bundle: (&Response, &Painter), app: &SwarmRsApp, view_transform: &Matrix3<f64>) {
    let (response, _painter) = bundle;
    let data = &app.app_data;
    let to_screen = egui::emath::RectTransform::from_to(
        Rect::from_min_size(Pos2::ZERO, response.rect.size()),
        response.rect,
    );

    let game = &data.game;

    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

    let to_point = |pos: [f64; 2]| {
        let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
    };

    fn vec2f64(v: [i32; 2]) -> [f64; 2] {
        [v[0] as f64 + 0.5, v[1] as f64 + 0.5]
    }

    for graph in &game.fog_graph_real {
        for pix in graph {
            bundle.1.line_segment(
                [to_point(vec2f64(pix[0])), to_point(vec2f64(pix[1]))],
                (1., Color32::WHITE),
            );
        }
    }

    let filter_team = |team| {
        move |res: &&RefCell<Entity>| {
            let entity = res.borrow();
            entity.get_team() == team || game.is_clear_fog_at(team, entity.get_pos())
        }
    };

    match (data.fog_active[0], data.fog_active[1]) {
        (false, false) | (true, true) => {
            for agent in game.entities.iter() {
                paint_real_agent(bundle, app, agent, view_transform, &to_point, to_screen);
            }
        }
        (true, false) => {
            for agent in game.entities.iter().filter(filter_team(0)) {
                paint_real_agent(bundle, app, agent, view_transform, &to_point, to_screen);
            }
            paint_shadow_agents(bundle.1, app, 0, &to_point);
        }
        (false, true) => {
            for agent in game.entities.iter().filter(filter_team(1)) {
                paint_real_agent(bundle, app, agent, view_transform, &to_point, to_screen);
            }
            paint_shadow_agents(bundle.1, app, 1, to_point);
        }
    }
}

fn paint_real_agent(
    (_response, painter): (&Response, &Painter),
    app: &SwarmRsApp,
    agent: &RefCell<Entity>,
    view_transform: &Matrix3<f64>,
    to_point: &impl Fn([f64; 2]) -> Pos2,
    to_screen: RectTransform,
) {
    let data = &app.app_data;
    let draw_rectangle = 1. / AGENT_HALFLENGTH < data.scale;

    let agent = agent.borrow();
    let agent_pos = agent.get_pos();
    let pos = to_point(agent_pos);
    let brush = if app.app_data.selected_entity == Some(agent.get_id()) {
        SELECTED_COLOR
    } else {
        AGENT_COLORS[agent.get_team() % AGENT_COLORS.len()]
    };
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
    } else if let Entity::Agent(agent) = &agent as &Entity {
        // use std::f64::consts::PI;
        let orient = Matrix2::from_angle(Rad(agent.orient));
        let render_wheel = |base, steer| {
            let steer = Matrix2::from_angle(Rad(steer + agent.orient));
            let center = orient * Vector2::new(base, 0.);
            let rear = center + steer * Vector2::new(-0.05, 0.) + Vector2::from(agent_pos);
            let front = center + steer * Vector2::new(0.05, 0.) + Vector2::from(agent_pos);
            painter.line_segment(
                [to_point(rear.into()), to_point(front.into())],
                (1., Color32::WHITE),
            );
        };

        const WHEEL_BASE: f64 = 1.;
        const REAR_OFFSET: f64 = -WHEEL_BASE / 2.;

        render_wheel(0.5, agent.steer);
        render_wheel(REAR_OFFSET, 0.);

        if f64::EPSILON < agent.steer.abs() {
            let r = WHEEL_BASE / agent.steer.tan();
            let rear_axis = orient * Vector2::new(REAR_OFFSET, 0.) + Vector2::from(agent_pos);
            let ind = orient * Vector2::new(0., r) + rear_axis;
            painter.line_segment(
                [to_point(rear_axis.into()), to_point(ind.into())],
                (1., Color32::WHITE),
            );
        }
    }

    let resource = agent.resource();
    if 0 < resource {
        use std::f64::consts::PI;
        let f = resource as f64 * 2. * PI / agent.max_resource() as f64;
        let count = 10; // There is no reason to pick this value, but it seems to work fine.
        for (i0, i1) in (0..count).zip(1..=count) {
            let theta0 = (i0 as f64 / count as f64 * f) as f32;
            let theta1 = (i1 as f64 / count as f64 * f) as f32;
            let p0 = Vec2::new(theta0.sin(), -theta0.cos()) * 7.5 + pos.to_vec2();
            let p1 = Vec2::new(theta1.sin(), -theta1.cos()) * 7.5 + pos.to_vec2();
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

    if data.target_visible {
        if let Some(target_pos) = agent.get_target_pos(&data.game) {
            let line = [pos, to_point(target_pos)];

            painter.line_segment(line, (1., brush));
        }
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
                    painter.circle_stroke(circle, (point.radius * data.scale) as f32, (1., brush));
                }
            }
        }
        painter.add(PathShape::line(path, (1., brush)));
    }

    if data.entity_trace_visible {
        if let Some(deque) = agent.get_trace() {
            let iter = deque.iter().copied().map(to_point).collect();
            let path = PathShape::line(
                iter,
                (
                    0.5,
                    Color32::from_rgba_unmultiplied(brush.r(), brush.g(), brush.b(), 127),
                ),
            );
            painter.add(path);
        }
    }

    if data.entity_label_visible {
        let text = if let Some(target) = agent.get_target() {
            format!("{} ({})", agent.get_id(), target)
        } else {
            format!("{} (?)", agent.get_id())
        };
        painter.text(pos, Align2::CENTER_TOP, text, FontId::monospace(16.), brush);
    }

    if 5. < data.scale {
        let health = agent.get_health_rate() as f32;
        let view_pos_left = transform_point(view_transform, [agent_pos.x - 1., agent_pos.y - 1.]);
        let view_pos_right = transform_point(view_transform, [agent_pos.x + 1., agent_pos.y - 1.]);
        if matches!(agent.get_class(), Some(AgentClass::Fighter)) {
            let base = pos2(view_pos_left.x as f32, view_pos_left.y as f32) + Vec2::new(8., -5.);
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

fn paint_shadow_agents(
    painter: &Painter,
    app: &SwarmRsApp,
    team: usize,
    to_point: impl Fn([f64; 2]) -> Pos2,
) {
    for entity in &app.app_data.game.fog[team].entities {
        let agent_pos = entity.pos;
        let pos = to_point(agent_pos);
        let brush = if app.app_data.selected_entity == Some(entity.id) {
            SELECTED_COLOR
        } else {
            AGENT_COLORS[(team + 1) % AGENT_COLORS.len()]
        };
        painter.circle_filled(pos, 5., brush);

        painter.circle_stroke(
            pos,
            10.,
            Stroke {
                color: brush,
                width: 3.,
            },
        );
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

    let game = &data.game;

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

    let game = &data.game;

    let offset = Vec2::new(data.origin[0] as f32, data.origin[1] as f32);

    let to_point = |pos: [f64; 2]| {
        let pos = Vec2::new(pos[0] as f32, pos[1] as f32);
        to_screen.transform_pos(((pos + offset) * data.scale as f32).to_pos2())
    };

    match (data.fog_active[0], data.fog_active[1]) {
        (false, false) => {
            for resource in &game.resources {
                draw_resource(resource, to_point(resource.pos));
            }
        }
        (true, false) => {
            for resource in &game.fog[0].resources {
                draw_resource(resource, to_point(resource.pos));
            }
        }
        (false, true) => {
            for resource in &game.fog[1].resources {
                draw_resource(resource, to_point(resource.pos));
            }
        }
        _ => {
            for resource in game.fog[0]
                .resources
                .iter()
                .chain(game.fog[1].resources.iter())
            {
                draw_resource(resource, to_point(resource.pos));
            }
        }
    }
}

fn paint_big_message(_response: &Response, painter: &Painter, data: &AppData, size: Vec2) {
    if 0. < data.big_message_time {
        let color = Color32::from_rgba_unmultiplied(
            255,
            255,
            255,
            ((data.big_message_time / 1000.).min(1.) * 255.) as u8,
        );

        // Not sure if it is correct way to do this, but `clip_rect()` doesn't seem to consider side panel,
        // and `ui.available_size()` assigns 0 to y.
        let rect = painter.clip_rect();
        let mut pos = (rect.min.to_vec2() + rect.max.to_vec2()) / 2.;
        pos.x = rect.min.x + size.x / 2.;

        painter.text(
            pos.to_pos2(),
            Align2::CENTER_CENTER,
            &data.big_message,
            FontId::proportional(48.),
            color,
        );
    }
}
