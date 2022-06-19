use crate::{
    app_data::{AppData, LineMode},
    marching_squares::{cell_lines, cell_polygon_index, pick_bits, BoolField, CELL_POLYGON_BUFFER},
    perlin_noise::Xor128,
};
use druid::widget::prelude::*;
use druid::{
    piet::kurbo::{BezPath, Circle, Line},
    Affine, Color, Point, Rect,
};

const OBSTACLE_COLOR: Color = Color::rgb8(63, 63, 63);
const BACKGROUND_COLOR: Color = Color::rgb8(127, 127, 127);

pub(crate) fn paint_board(ctx: &mut PaintCtx, data: &AppData) {
    let (w0, h0) = (32., 32.);
    let cell_size = (28., 28.);

    let view_transform = data.view_transform();

    ctx.save().unwrap();
    ctx.transform(view_transform);

    let mut rng = Xor128::new(32132);

    let (xs, ys) = (data.xs, data.ys);
    for (i, cell) in data.board.iter().enumerate() {
        let xi = i % xs;
        let yi = i / ys;
        let point = Point {
            x: w0 * (xi as f64 - 1.) + (w0 - cell_size.0) * 0.5,
            y: h0 * (yi as f64 - 1.) + (h0 - cell_size.1) * 0.5,
        };
        let rect = Rect::from_origin_size(point, cell_size);
        ctx.fill(
            rect,
            if *cell {
                &BACKGROUND_COLOR
            } else {
                &OBSTACLE_COLOR
            },
        );
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

    if data.simplified_visible {
        for bez_path in data.simplified_border.as_ref() {
            let stroke_color = Color::rgb8(
                (rng.nexti() % 0x80 + 0x7f) as u8,
                (rng.nexti() % 0x80 + 0x7f) as u8,
                (rng.nexti() % 0x80 + 0x7f) as u8,
            );

            ctx.stroke(scale_transform * bez_path, &stroke_color, 2.0);
        }
    }

    fn delaunator_to_druid_point(p: &delaunator::Point) -> Point {
        Point { x: p.x, y: p.y }
    }

    const PURPLE_COLOR: Color = Color::rgb8(255, 0, 255);

    if data.triangulation_visible {
        let triangles = &data.triangulation.triangles;
        for triangle in triangles.chunks(3) {
            let vertices: [usize; 4] = [triangle[0], triangle[1], triangle[2], triangle[0]];
            for (start, end) in vertices.iter().zip(vertices.iter().skip(1)) {
                let line = Line::new(
                    delaunator_to_druid_point(&data.points[*start]),
                    delaunator_to_druid_point(&data.points[*end]),
                );
                ctx.stroke(scale_transform * line, &PURPLE_COLOR, 1.0);
            }
        }
    }

    const GREEN_COLOR: Color = Color::GREEN;

    for agent in data.agents.iter() {
        let pos = agent.pos;
        let pos = Point {
            x: pos[0] * w0,
            y: pos[1] * h0,
        };
        let circle = Circle::new(view_transform * pos, 5.);

        ctx.fill(circle, &GREEN_COLOR);
    }

    *data.render_stats.borrow_mut() = format!("Drawn {} contours", contours);
}
