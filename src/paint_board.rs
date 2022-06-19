use crate::{
    app_data::{AppData, LineMode},
    marching_squares::{cell_lines, cell_polygon_index, pick_bits, BoolField, CELL_POLYGON_BUFFER},
    perlin_noise::Xor128,
    triangle_utils::center_of_triangle_obj,
};
use druid::widget::prelude::*;
use druid::{
    piet::kurbo::{BezPath, Circle, Line},
    piet::{FontFamily, ImageFormat, InterpolationMode},
    Affine, Color, FontDescriptor, Point, TextLayout,
};

const OBSTACLE_COLOR: u8 = 63u8;
const BACKGROUND_COLOR: u8 = 127u8;

pub(crate) fn paint_board(ctx: &mut PaintCtx, data: &AppData, env: &Env) {
    let (w0, h0) = (32., 32.);

    let view_transform = data.view_transform();

    ctx.save().unwrap();
    ctx.transform(view_transform);

    let (xs, ys) = (data.xs, data.ys);

    match ctx.make_image(
        xs,
        ys,
        &data
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
                    Point::new(-w0, -h0),
                    Point::new(w0 * (xs as f64 - 1.), h0 * (ys as f64 - 1.)),
                ),
                InterpolationMode::NearestNeighbor,
            );
        }
        Err(e) => println!("Make image error: {}", e.to_string()),
    }

    let shape = (xs as isize, ys as isize);

    const RED_COLOR: Color = Color::rgb8(255, 0, 0);

    let field = BoolField::new(data.board.as_ref(), shape);

    let mut contours = 0;
    match data.line_mode {
        LineMode::None => ctx.restore().unwrap(),
        LineMode::Line => {
            ctx.restore().unwrap();
            for y in 0..ys - 1 {
                for x in 0..xs - 1 {
                    let bits = pick_bits(&field, (x as isize, y as isize));

                    if bits == 0 || bits == 15 {
                        continue;
                    }

                    let lines = cell_lines(bits);
                    let to_point = |p: [f32; 2]| {
                        Point::new(
                            (p[0] as f64 + x as f64 - 0.5) * w0,
                            (p[1] as f64 + y as f64 - 0.5) * h0,
                        )
                    };
                    for line in lines {
                        let line = view_transform * Line::new(to_point(line[0]), to_point(line[1]));
                        ctx.stroke(line, &RED_COLOR, 2.0);
                    }
                    contours += 1;
                }
            }
        }
        LineMode::Polygon => {
            let scale_transform = /*view_transform*/ Affine::scale(w0);

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
                    path.move_to(scale_transform * to_point(0));
                    path.line_to(scale_transform * to_point(1));
                    path.line_to(scale_transform * to_point(2));
                    path.line_to(scale_transform * to_point(3));
                    path.close_path();
                    ctx.fill(path, &RED_COLOR);
                    contours += 1;
                }
            }
            ctx.restore().unwrap();
        }
    }

    let scale_transform = view_transform * Affine::scale(w0);

    fn delaunator_to_druid_point(p: &delaunator::Point) -> Point {
        Point { x: p.x, y: p.y }
    }

    if data.triangulation_visible {
        let mut rng = Xor128::new(616516);

        let max_label = *data.triangle_labels.iter().max().unwrap_or(&0) as usize + 1;
        let label_colors = (0..max_label)
            .map(|_| {
                Color::rgb8(
                    (rng.nexti() % 0x80 + 0x7f) as u8,
                    (rng.nexti() % 0x80 + 0x7f) as u8,
                    (rng.nexti() % 0x80 + 0x7f) as u8,
                )
            })
            .collect::<Vec<_>>();

        let triangles = &data.triangulation.triangles;
        for (i, triangle) in triangles.chunks(3).enumerate() {
            use ::delaunator::EMPTY;
            let label = data.triangle_labels[i];
            if triangles[i * 3] == EMPTY
                || triangles[i * 3 + 1] == EMPTY
                || triangles[i * 3 + 2] == EMPTY
            {
                continue;
            }
            let color = if data.triangle_passable[i] && label >= 0 {
                label_colors[label as usize].clone()
            } else {
                Color::RED
            };

            if data.triangle_passable[i] && label >= 0 || data.unpassable_visible {
                let vertices: [usize; 4] = [triangle[0], triangle[1], triangle[2], triangle[0]];
                for (start, end) in vertices.iter().zip(vertices.iter().skip(1)) {
                    let line = Line::new(
                        delaunator_to_druid_point(&data.points[*start]),
                        delaunator_to_druid_point(&data.points[*end]),
                    );
                    ctx.stroke(scale_transform * line, &color, 1.0);
                }
            }

            if data.triangle_label_visible && (data.triangle_passable[i] || data.unpassable_visible)
            {
                let mut layout = TextLayout::<String>::from_text(format!("{}", i));
                layout.set_font(FontDescriptor::new(FontFamily::SANS_SERIF).with_size(16.0));
                layout.set_text_color(color);
                layout.rebuild_if_needed(ctx.text(), env);
                layout.draw(
                    ctx,
                    scale_transform
                        * delaunator_to_druid_point(&center_of_triangle_obj(
                            &data.triangulation,
                            &data.points,
                            i,
                        )),
                );
            }
        }
    }

    if data.simplified_visible {
        let mut rng = Xor128::new(32132);

        for bez_path in data.simplified_border.as_ref() {
            let stroke_color = Color::rgb8(
                (rng.nexti() % 0x80 + 0x7f) as u8,
                (rng.nexti() % 0x80 + 0x7f) as u8,
                (rng.nexti() % 0x80 + 0x7f) as u8,
            );

            ctx.stroke(scale_transform * bez_path, &stroke_color, 2.0);
        }
    }

    let to_point = |pos: [f64; 2]| Point {
        x: pos[0] * w0,
        y: pos[1] * h0,
    };

    const AGENT_COLORS: [Color; 2] = [Color::rgb8(63, 255, 63), Color::RED];

    for agent in data.entities.iter() {
        let agent = agent.borrow();
        let pos = to_point(agent.get_pos());
        let circle = Circle::new(view_transform * pos, 5.);
        let brush = &AGENT_COLORS[agent.get_team() % AGENT_COLORS.len()];
        ctx.fill(circle, brush);

        if !agent.is_agent() {
            let big_circle = Circle::new(view_transform * pos, 10.);
            ctx.stroke(big_circle, brush, 3.);
        }

        if data.target_visible {
            if let Some(target) = agent.get_target() {
                if let Some(target) = data
                    .entities
                    .iter()
                    .find(|agent| agent.borrow().get_id() == target)
                {
                    let target_pos = target.borrow().get_pos();
                    let line = Line::new(pos, to_point(target_pos));

                    ctx.stroke(view_transform * line, brush, 1.);
                }
            }
        }

        if data.path_visible {
            if let Some((first, rest)) = agent.get_path().and_then(|path| path.split_first()) {
                let mut bez_path = BezPath::new();
                bez_path.move_to(to_point(*first));
                for point in rest {
                    bez_path.line_to(to_point(*point));
                }
                bez_path.line_to(to_point(agent.get_pos()));
                ctx.stroke(view_transform * bez_path, brush, 1.);
            }
        }
    }

    for bullet in data.bullets.iter() {
        let circle = Circle::new(view_transform * to_point(bullet.pos), 3.);
        ctx.fill(
            circle,
            if bullet.team == 0 {
                &Color::WHITE
            } else {
                &Color::PURPLE
            },
        );
        ctx.stroke(circle, &Color::YELLOW, 1.);
    }

    *data.render_stats.borrow_mut() = format!("Drawn {} contours", contours);
}
