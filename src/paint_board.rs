use crate::{
    agent::{wrap_angle, Bullet, AGENT_HALFLENGTH, AGENT_HALFWIDTH, BULLET_RADIUS, BULLET_SPEED},
    app_data::{AppData, LineMode},
    entity::Entity,
    marching_squares::{cell_lines, cell_polygon_index, pick_bits, BoolField, CELL_POLYGON_BUFFER},
    perlin_noise::Xor128,
    temp_ents::MAX_TTL,
    triangle_utils::center_of_triangle_obj,
};
use cgmath::Vector2;
use druid::{
    kurbo::Shape,
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

    paint_agents(ctx, data, env, &view_transform);

    paint_bullets(ctx, data, &view_transform);

    paint_temp_ents(ctx, data, &view_transform);

    *data.render_stats.borrow_mut() = format!(
        "Drawn {} contours, {} triangles",
        contours,
        data.game.mesh.triangulation.triangles.len()
    );
}

pub(crate) fn paint_board(
    ctx: &mut PaintCtx,
    data: &AppData,
    env: &Env,
    view_transform: &Affine,
) -> usize {
    let (xs, ys) = (data.game.xs, data.game.ys);

    let mut contours = 0;

    const RED_COLOR: Color = Color::rgb8(255, 0, 0);

    let shape = (xs as isize, ys as isize);

    let field = BoolField::new(data.game.board.as_ref(), shape);

    ctx.with_save(|ctx| {
        ctx.transform(*view_transform);

        match ctx.make_image(
            xs,
            ys,
            &data
                .game
                .board
                .iter()
                .map(|p| if *p { BACKGROUND_COLOR } else { OBSTACLE_COLOR })
                .collect::<Vec<_>>(),
            ImageFormat::Grayscale,
        ) {
            Ok(res) => {
                ctx.draw_image(
                    &res,
                    (
                        Point::new(-1., -1.),
                        Point::new(xs as f64 - 1., ys as f64 - 1.),
                    ),
                    InterpolationMode::NearestNeighbor,
                );
            }
            Err(e) => println!("Make image error: {}", e.to_string()),
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

        let points = &data.game.mesh.points;
        let triangles = &data.game.mesh.triangulation.triangles;
        let triangle_labels = &data.game.mesh.triangle_labels;
        let triangle_passable = &data.game.mesh.triangle_passable;

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
                            &data.game.mesh.triangulation,
                            &points,
                            i,
                        )),
                );
            }
        }
    }

    if data.simplified_visible {
        let mut rng = Xor128::new(32132);

        for bez_path in &data.game.mesh.simplified_border {
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

    let draw_rectangle = 1. / AGENT_HALFLENGTH < data.scale;
    let entities = data.game.entities.borrow();

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

        // let bcirc = agent.bounding_circle();
        // let shape = Circle::new(
        //     *view_transform * to_point(bcirc.center.into()),
        //     bcirc.radius * data.scale,
        // );
        // ctx.stroke(shape, brush, 1.);

        if let Some(orient) = agent.get_orient() {
            let length = 10.;
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
                path.move_to(Point::new(-AGENT_HALFLENGTH, -AGENT_HALFWIDTH));
                path.line_to(Point::new(AGENT_HALFLENGTH, -AGENT_HALFWIDTH));
                path.line_to(Point::new(AGENT_HALFLENGTH, AGENT_HALFWIDTH));
                path.line_to(Point::new(-AGENT_HALFLENGTH, AGENT_HALFWIDTH));
                path.close_path();
                ctx.stroke(rot_transform * path, brush, 1.);
            }
        }

        if data.target_visible {
            if let Some(target) = agent.get_target() {
                if let Some(target) = data
                    .game
                    .entities
                    .borrow()
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
                    bez_path.move_to(to_point(*first));
                }
                for point in rest {
                    bez_path.line_to(to_point(*point));
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

    let draw_small = data.scale < TARGET_PIXELS / BULLET_RADIUS;

    ctx.with_save(|ctx| {
        ctx.transform(*view_transform);
        for bullet in data.game.bullets.iter() {
            let pos = to_vec2(bullet.pos);
            let velo = to_vec2(bullet.velo).normalize();
            let perp = Vec2::new(velo.y, -velo.x) * BULLET_RADIUS;
            let length = bullet.traveled.min(2. * BULLET_SPEED);
            let tail = pos - velo * length;
            let mut trail = BezPath::new();
            trail.move_to((pos + perp).to_point());
            trail.line_to((pos - perp).to_point());
            trail.line_to(tail.to_point());
            trail.close_path();
            ctx.fill(trail, &Color::rgb8(255, 191, 63));
            if !draw_small {
                draw_bullet(ctx, bullet, to_point(bullet.pos), BULLET_RADIUS);
            }
        }
    });

    if draw_small {
        for bullet in data.game.bullets.iter() {
            let view_pos = *view_transform * to_point(bullet.pos);
            draw_bullet(ctx, bullet, view_pos, TARGET_PIXELS);
        }
    }
}

fn paint_temp_ents(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
    for temp_ent in data.game.temp_ents.borrow().iter() {
        let pos = to_point(temp_ent.pos);
        let circle = Circle::new(pos, 2. * (MAX_TTL - temp_ent.ttl) / MAX_TTL);
        let alpha = (temp_ent.ttl * 512. / MAX_TTL).min(255.) as u8;
        ctx.fill(*view_transform * circle, &Color::rgba8(255, 127, 0, alpha));
    }
}
