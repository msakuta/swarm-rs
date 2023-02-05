use crate::{
    agent::{AgentClass, Bullet, AGENT_HALFLENGTH, AGENT_HALFWIDTH, BULLET_RADIUS},
    app_data::{AppData, LineMode},
    entity::Entity,
    game::Resource,
    marching_squares::{cell_lines, cell_polygon_index, pick_bits, BoolField, CELL_POLYGON_BUFFER},
    perlin_noise::Xor128,
    qtree::render::paint_qtree,
    triangle_utils::center_of_triangle_obj,
};

use cgmath::{InnerSpace, Vector2};
use druid::{
    kurbo::{Arc, Shape},
    piet::kurbo::{BezPath, Circle, Line},
    piet::{FontFamily, ImageFormat, InterpolationMode},
    Affine, Color, FontDescriptor, Point, Rect, TextLayout,
};
use druid::{widget::prelude::*, Vec2};

const OBSTACLE_COLOR: u8 = 63u8;
const BACKGROUND_COLOR: u8 = 127u8;

pub(crate) fn paint_game(ctx: &mut PaintCtx, data: &AppData, env: &Env) {
    let view_transform = data.view_transform();

    let contours = paint_board(ctx, data, env, &view_transform);

    if data.qtree_visible {
        paint_qtree(ctx, data, &view_transform);
    }

    paint_resources(ctx, data, &view_transform);

    paint_agents(ctx, data, env, &view_transform);

    paint_bullets(ctx, data, &view_transform);

    paint_temp_ents(ctx, data, &view_transform);

    paint_big_message(ctx, env, data);

    *data.render_stats.borrow_mut() = format!(
        "Drawn {} contours, {} triangles",
        contours,
        data.game.borrow().mesh.triangulation.triangles.len()
    );
}

pub(crate) fn paint_board(
    ctx: &mut PaintCtx,
    data: &AppData,
    env: &Env,
    view_transform: &Affine,
) -> usize {
    let game = data.game.borrow();
    let (xs, ys) = (game.xs, game.ys);

    let mut contours = 0;

    const RED_COLOR: Color = Color::rgb8(255, 0, 0);

    let shape = (xs as isize, ys as isize);

    let field = BoolField::new(game.board.as_ref(), shape);

    ctx.with_save(|ctx| {
        ctx.transform(*view_transform);

        let render_image = |ctx: &mut PaintCtx, image: Result<_, druid::piet::Error>| match image {
            Ok(res) => {
                ctx.draw_image(
                    &res,
                    (
                        Point::new(0., 0.),
                        Point::new(xs as f64 - 0., ys as f64 - 0.),
                    ),
                    InterpolationMode::NearestNeighbor,
                );
            }
            Err(e) => println!("Make image error: {}", e.to_string()),
        };

        if data.show_label_image {
            let mut rng = Xor128::new(616516);
            let max_label = *game.mesh.labeled_image.iter().max().unwrap_or(&0) as usize + 1;
            let label_colors = (0..max_label)
                .map(|label| {
                    if label == 0 {
                        [OBSTACLE_COLOR, OBSTACLE_COLOR, OBSTACLE_COLOR]
                    } else {
                        [
                            (rng.nexti() % 0x80) as u8,
                            (rng.nexti() % 0x80) as u8,
                            (rng.nexti() % 0x80) as u8,
                        ]
                    }
                })
                .collect::<Vec<_>>();

            let img = ctx.make_image(
                xs,
                ys,
                &game
                    .mesh
                    .labeled_image
                    .iter()
                    .map(|p| label_colors[*p as usize].into_iter())
                    .flatten()
                    .collect::<Vec<_>>(),
                ImageFormat::Rgb,
            );
            render_image(ctx, img);
        } else {
            let img = ctx.make_image(
                xs,
                ys,
                &game
                    .board
                    .iter()
                    .map(|p| if *p { BACKGROUND_COLOR } else { OBSTACLE_COLOR })
                    .collect::<Vec<_>>(),
                ImageFormat::Grayscale,
            );
            render_image(ctx, img);
        }

        if let LineMode::Polygon = data.line_mode {
            for y in 0..ys - 1 {
                for x in 0..xs - 1 {
                    let bits = pick_bits(&field, (x as isize, y as isize));

                    if bits == 0 || bits == 15 {
                        continue;
                    }

                    let idx = (cell_polygon_index(bits) / 4) as usize;

                    let poly = CELL_POLYGON_BUFFER[idx as usize];
                    let mut path = BezPath::new();
                    let to_point = |i: usize| {
                        Point::new(
                            (poly[i * 2] as f64) / 2. + x as f64,
                            (poly[i * 2 + 1] as f64) / 2. + y as f64,
                        )
                    };
                    path.move_to(to_point(0));
                    path.line_to(to_point(1));
                    path.line_to(to_point(2));
                    path.line_to(to_point(3));
                    path.close_path();
                    ctx.fill(path, &RED_COLOR);
                    contours += 1;
                }
            }
        }
    });

    if let LineMode::Line = data.line_mode {
        for y in 0..ys - 1 {
            for x in 0..xs - 1 {
                let bits = pick_bits(&field, (x as isize, y as isize));

                if bits == 0 || bits == 15 {
                    continue;
                }

                let lines = cell_lines(bits);
                let to_point = |p: [f32; 2]| {
                    Point::new(p[0] as f64 + x as f64 - 0.5, p[1] as f64 + y as f64 - 0.5)
                };
                for line in lines {
                    let line = *view_transform * Line::new(to_point(line[0]), to_point(line[1]));
                    ctx.stroke(line, &RED_COLOR, 2.0);
                }
                contours += 1;
            }
        }
    }

    let scale_transform = *view_transform;

    fn delaunator_to_druid_point(p: &delaunator::Point) -> Point {
        Point { x: p.x, y: p.y }
    }

    if data.triangulation_visible {
        let mut rng = Xor128::new(616516);

        let points = &game.mesh.points;
        let triangles = &game.mesh.triangulation.triangles;
        let triangle_labels = &game.mesh.triangle_labels;
        let triangle_passable = &game.mesh.triangle_passable;

        let max_label = *triangle_labels.iter().max().unwrap_or(&0) as usize + 1;
        let label_colors = (0..max_label)
            .map(|_| {
                Color::rgb8(
                    (rng.nexti() % 0x80 + 0x7f) as u8,
                    (rng.nexti() % 0x80 + 0x7f) as u8,
                    (rng.nexti() % 0x80 + 0x7f) as u8,
                )
            })
            .collect::<Vec<_>>();

        for (i, triangle) in triangles.chunks(3).enumerate() {
            use ::delaunator::EMPTY;
            let label = triangle_labels[i];
            if triangles[i * 3] == EMPTY
                || triangles[i * 3 + 1] == EMPTY
                || triangles[i * 3 + 2] == EMPTY
            {
                continue;
            }
            let color = if triangle_passable[i] && label >= 0 {
                label_colors[label as usize].clone()
            } else {
                Color::RED
            };

            if triangle_passable[i] && label >= 0 || data.unpassable_visible {
                let vertices: [usize; 4] = [triangle[0], triangle[1], triangle[2], triangle[0]];
                for (start, end) in vertices.iter().zip(vertices.iter().skip(1)) {
                    let line = Line::new(
                        delaunator_to_druid_point(&points[*start]),
                        delaunator_to_druid_point(&points[*end]),
                    );
                    ctx.stroke(scale_transform * line, &color, 1.0);
                }
            }

            if data.triangle_label_visible && (triangle_passable[i] || data.unpassable_visible) {
                let mut layout = TextLayout::<String>::from_text(format!("{}", i));
                layout.set_font(FontDescriptor::new(FontFamily::SANS_SERIF).with_size(16.0));
                layout.set_text_color(color);
                layout.rebuild_if_needed(ctx.text(), env);
                layout.draw(
                    ctx,
                    scale_transform
                        * delaunator_to_druid_point(&center_of_triangle_obj(
                            &game.mesh.triangulation,
                            &points,
                            i,
                        )),
                );
            }
        }
    }

    if data.simplified_visible {
        let mut rng = Xor128::new(32132);

        for bez_path in &game.mesh.simplified_border {
            let stroke_color = Color::rgb8(
                (rng.nexti() % 0x80 + 0x7f) as u8,
                (rng.nexti() % 0x80 + 0x7f) as u8,
                (rng.nexti() % 0x80 + 0x7f) as u8,
            );

            ctx.stroke(scale_transform * bez_path, &stroke_color, 2.0);
        }
    }

    contours
}

pub(crate) fn to_point(pos: [f64; 2]) -> Point {
    Point {
        x: pos[0],
        y: pos[1],
    }
}

fn to_vec2(pos: [f64; 2]) -> Vec2 {
    Vec2 {
        x: pos[0],
        y: pos[1],
    }
}

fn paint_agents(ctx: &mut PaintCtx, data: &AppData, env: &Env, view_transform: &Affine) {
    const AGENT_COLORS: [Color; 2] = [Color::rgb8(0, 255, 127), Color::rgb8(255, 0, 63)];

    let game = data.game.borrow();
    let draw_rectangle = 1. / AGENT_HALFLENGTH < data.scale;
    let entities = &game.entities;

    for agent in entities.iter() {
        let agent = agent.borrow();
        let pos = to_point(agent.get_pos());
        let circle = Circle::new(*view_transform * pos, 5.);
        let brush = &AGENT_COLORS[agent.get_team() % AGENT_COLORS.len()];
        ctx.fill(circle, brush);

        if !agent.is_agent() {
            let big_circle = Circle::new(*view_transform * pos, 10.);
            ctx.stroke(big_circle, brush, 3.);
        }

        let resource = agent.resource();
        if 0 < resource {
            use std::f64::consts::PI;
            let arc = Arc {
                center: *view_transform * pos,
                radii: Vec2::new(7.5, 7.5),
                start_angle: 0.,
                sweep_angle: resource as f64 * 2. * PI / agent.max_resource() as f64,
                x_rotation: -PI / 2.,
            };
            ctx.stroke(arc, &Color::YELLOW, 2.5);
        }

        // let bcirc = agent.bounding_circle();
        // let shape = Circle::new(
        //     *view_transform * to_point(bcirc.center.into()),
        //     bcirc.radius * data.scale,
        // );
        // ctx.stroke(shape, brush, 1.);

        if let Some(orient) = agent.get_orient() {
            let class = agent.get_class().unwrap_or(AgentClass::Worker);
            let length = if matches!(class, AgentClass::Fighter) {
                20.
            } else {
                10.
            };
            let view_pos = *view_transform * pos;
            let dest = Point::new(
                view_pos.x + orient.cos() * length,
                view_pos.y + orient.sin() * length,
            );
            let orient_line = Line::new(view_pos, dest);
            ctx.stroke(orient_line, brush, 3.);

            if draw_rectangle {
                let rot_transform =
                    *view_transform * Affine::translate(pos.to_vec2()) * Affine::rotate(orient);
                let mut path = BezPath::new();
                let mut first = true;
                class.vertices(|v| {
                    if first {
                        path.move_to(Point::new(v[0], v[1]));
                        first = false;
                    } else {
                        path.line_to(Point::new(v[0], v[1]));
                    }
                });
                path.close_path();
                ctx.stroke(rot_transform * path, brush, 1.);
            }
        } else {
            let aabb = agent.get_aabb();
            let rect = Rect::new(aabb[0], aabb[1], aabb[2], aabb[3]);
            ctx.stroke(*view_transform * rect.to_path(0.01), brush, 1.);
        }

        if data.target_visible {
            if let Some(target) = agent.get_target() {
                if let Some(target) = game
                    .entities
                    .iter()
                    .find(|agent| agent.borrow().get_id() == target)
                {
                    let target_pos = target.borrow().get_pos();
                    let line = Line::new(pos, to_point(target_pos));

                    ctx.stroke(*view_transform * line, brush, 1.);
                }
            }
        }

        if data.avoidance_render_params.visible {
            if let Some(goal) = agent.get_goal() {
                const CROSS_SIZE: f64 = 5.;
                let goal = *view_transform * Point::new(goal.x, goal.y);
                let mut bez_path = BezPath::new();
                bez_path.move_to(goal + Vec2::new(-CROSS_SIZE, CROSS_SIZE));
                bez_path.line_to(goal + Vec2::new(CROSS_SIZE, -CROSS_SIZE));
                bez_path.move_to(goal + Vec2::new(-CROSS_SIZE, -CROSS_SIZE));
                bez_path.line_to(goal + Vec2::new(CROSS_SIZE, CROSS_SIZE));
                ctx.stroke(bez_path, brush, 2.);
            }

            if let Some(search_state) = agent.get_search_state() {
                search_state.render(
                    ctx,
                    env,
                    view_transform,
                    &data.avoidance_render_params,
                    brush,
                    data.scale,
                );
            }

            if let Entity::Agent(agent) = &*agent {
                if let Some(avoidance_plan) = &agent.avoidance_plan {
                    for &(drive, steer) in avoidance_plan {
                        let target = agent.get_avoidance_state((drive, steer));
                        let circle = Circle::new(to_point(target.into()), AGENT_HALFWIDTH);
                        ctx.fill(*view_transform * circle, &Color::rgba8(255, 255, 255, 127));
                    }
                }
            }
        }

        if data.qtree_search_visible {
            if let Some(search_tree) = agent.get_search_tree() {
                search_tree.render(ctx, env, view_transform, brush, data.scale);
            }
        }

        if data.path_visible {
            let avoidance_drawn = 'breaky: {
                let Some(path) = agent
                    .get_avoidance_path() else {
                        break 'breaky None;
                    };
                if path.len() == 0 {
                    break 'breaky None;
                }
                let mut bez_path = BezPath::new();
                let (rest, last) = if let Some(goal) = agent.get_goal() {
                    bez_path.move_to(to_point(goal.into()));
                    let goal: [f64; 2] = goal.into();
                    (&path[..], path.last().copied().or(Some(goal.into())))
                } else if let Some((first, rest)) = path.split_first() {
                    bez_path.move_to(to_point((*first).into()));
                    (rest, rest.last().copied().or_else(|| Some(*first)))
                } else {
                    break 'breaky None;
                };
                for point in rest {
                    bez_path.line_to(to_point((*point).into()));
                }
                bez_path.line_to(to_point(agent.get_pos()));
                ctx.stroke(*view_transform * bez_path, brush, 1.);
                last.map(|p| p.into())
            };
            if let Some((first, rest)) = agent.get_path().and_then(|path| path.split_first()) {
                let mut bez_path = BezPath::new();
                if let Some(first) = avoidance_drawn {
                    bez_path.move_to(to_point(first));
                } else {
                    bez_path.move_to(to_point(first.pos));
                }
                for point in rest {
                    let circle = Circle::new(to_point(point.pos), point.radius);
                    ctx.stroke(*view_transform * circle, brush, 1.);
                    bez_path.line_to(to_point(point.pos));
                }
                bez_path.line_to(to_point(agent.get_pos()));
                ctx.stroke(*view_transform * bez_path, brush, 1.);
            }
        }

        if data.entity_trace_visible {
            if let Some(deque) = agent.get_trace() {
                let mut iter = deque.iter();
                if let Some(first) = iter.next() {
                    let mut bez_path = BezPath::new();
                    bez_path.move_to(to_point(*first));
                    for point in iter {
                        bez_path.line_to(to_point(*point));
                    }
                    bez_path.line_to(to_point(agent.get_pos()));
                    ctx.stroke(*view_transform * bez_path, brush, 0.5);
                }
            }
        }

        if data.entity_label_visible {
            let mut layout =
                TextLayout::<String>::from_text(if let Some(target) = agent.get_target() {
                    format!("{} ({})", agent.get_id(), target)
                } else {
                    format!("{} (?)", agent.get_id())
                });
            layout.set_font(FontDescriptor::new(FontFamily::SANS_SERIF).with_size(16.0));
            layout.set_text_color(brush.clone());
            layout.rebuild_if_needed(ctx.text(), env);
            layout.draw(ctx, *view_transform * pos);
        }

        if 5. < data.scale {
            let health = agent.get_health_rate();
            let view_pos_left = *view_transform * Point::new(pos.x - 1., pos.y - 1.);
            let view_pos_right = *view_transform * Point::new(pos.x + 1., pos.y - 1.);
            if matches!(agent.get_class(), Some(AgentClass::Fighter)) {
                let mut cross = BezPath::new();
                let base = view_pos_left + Vec2::new(8., -5.);
                cross.move_to(base + Vec2::new(-8., -25.));
                cross.line_to(base + Vec2::new(0., -20.));
                cross.line_to(base + Vec2::new(8., -25.));
                cross.line_to(base + Vec2::new(8., -20.));
                cross.line_to(base + Vec2::new(0., -15.));
                cross.line_to(base + Vec2::new(-8., -20.));
                cross.close_path();
                ctx.fill(&cross, &Color::YELLOW);
                ctx.stroke(cross, brush, 1.);
            }
            let l = view_pos_left.x;
            let r = view_pos_right.x;
            let t = view_pos_left.y - 15.;
            let b = view_pos_left.y - 10.;
            let rect = Rect::new(l, t, r, b);
            ctx.fill(rect.to_path(0.1), &Color::RED);
            let health_rect = Rect::new(l, t, l + health * (r - l), b);
            ctx.fill(health_rect.to_path(0.1), &Color::rgb8(0, 191, 0));
        }
    }
}

fn paint_bullets(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
    let draw_bullet = |ctx: &mut PaintCtx, bullet: &Bullet, pos: Point, radius: f64| {
        let circle = Circle::new(pos, radius);
        ctx.fill(
            circle,
            if bullet.team == 0 {
                &Color::WHITE
            } else {
                &Color::FUCHSIA
            },
        );
        ctx.stroke(circle, &Color::YELLOW, radius / 10.);
    };

    const TARGET_PIXELS: f64 = 3.;

    let game = data.game.borrow();

    let draw_small = data.scale < TARGET_PIXELS / BULLET_RADIUS;

    ctx.with_save(|ctx| {
        ctx.transform(*view_transform);
        for bullet in game.bullets.iter() {
            let pos = to_vec2(bullet.pos);
            let velo = to_vec2(bullet.velo).normalize();
            let length = bullet
                .traveled
                .min(2. * Vector2::from(bullet.velo).magnitude());
            let tail = pos - velo * length;
            if matches!(bullet.shooter_class, AgentClass::Fighter) {
                let perp = Vec2::new(velo.y, -velo.x) * BULLET_RADIUS;
                let mut trail = BezPath::new();
                trail.move_to((pos + perp).to_point());
                trail.line_to((pos - perp).to_point());
                trail.line_to(tail.to_point());
                trail.close_path();
                ctx.fill(trail, &Color::rgb8(255, 191, 63));
                if !draw_small {
                    draw_bullet(ctx, bullet, to_point(bullet.pos), BULLET_RADIUS);
                }
            } else {
                let mut trail = BezPath::new();
                trail.move_to((pos + velo).to_point());
                trail.line_to((pos - velo).to_point());
                ctx.stroke(trail, &Color::rgb8(255, 191, 63), 0.075);
            };
        }
    });

    if draw_small {
        for bullet in game.bullets.iter() {
            if matches!(bullet.shooter_class, AgentClass::Fighter) {
                let view_pos = *view_transform * to_point(bullet.pos);
                draw_bullet(ctx, bullet, view_pos, TARGET_PIXELS);
            }
        }
    }
}

fn paint_resources(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
    const TARGET_PIXELS: f64 = 10.;

    let draw_resource = |ctx: &mut PaintCtx, resource: &Resource, pos: Point| {
        let radius = (resource.amount as f64).sqrt() / TARGET_PIXELS;
        let circle = Circle::new(pos, radius);
        ctx.fill(circle, &Color::YELLOW);
        ctx.stroke(circle, &Color::YELLOW, radius / 30.);
    };

    let game = data.game.borrow();

    ctx.with_save(|ctx| {
        ctx.transform(*view_transform);
        for resource in &game.resources {
            draw_resource(ctx, resource, to_point(resource.pos));
        }
    });
}

fn paint_temp_ents(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
    for temp_ent in data.game.borrow().temp_ents.iter() {
        let pos = to_point(temp_ent.pos);
        let max_ttl = temp_ent.max_ttl;
        let circle = Circle::new(
            pos,
            temp_ent.max_radius * (max_ttl - temp_ent.ttl) / max_ttl,
        );
        let alpha = (temp_ent.ttl * 512. / max_ttl).min(255.) as u8;
        ctx.fill(*view_transform * circle, &Color::rgba8(255, 127, 0, alpha));
    }
}

fn paint_big_message(ctx: &mut PaintCtx, env: &Env, data: &AppData) {
    if 0. < data.big_message_time {
        let mut layout = TextLayout::<String>::from_text(&data.big_message);
        layout.set_font(FontDescriptor::new(FontFamily::SANS_SERIF).with_size(48.0));
        layout.set_text_color(Color::rgba(
            1.,
            1.,
            1.,
            (data.big_message_time / 1000.).min(1.),
        ));
        layout.rebuild_if_needed(ctx.text(), env);
        let metrics = layout.layout_metrics();
        let size = ctx.size();
        layout.draw(
            ctx,
            Point::new(
                (size.width - metrics.size.width) / 2.,
                (size.height - metrics.size.height) / 2.,
            ),
        );
    }
}
