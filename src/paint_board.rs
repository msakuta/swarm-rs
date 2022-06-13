use crate::{
    app_data::{AppData, LineMode},
    marching_squares::{cell_lines, cell_polygon_index, pick_bits, BoolField, CELL_POLYGON_BUFFER},
};
use druid::widget::prelude::*;
use druid::{
    piet::kurbo::{BezPath, Line},
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

    let (xs, ys) = (data.xs, data.ys);
    for (i, cell) in data.board.iter().enumerate() {
        let xi = i % xs;
        let yi = i / ys;
        let point = Point {
            x: w0 * xi as f64,
            y: h0 * yi as f64,
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

    let scale_transform = /*view_transform*/ Affine::scale(w0);

    const RED_COLOR: Color = Color::rgb8(255, 0, 0);

    let field = BoolField::new(data.board.as_ref(), shape);

    let mut contours = 0;
    match data.line_mode {
        LineMode::Line => {
            ctx.restore().unwrap();
            for y in 0..ys - 1 {
                for x in 0..xs - 1 {
                    let bits = pick_bits(field, (x as isize, y as isize));

                    if bits == 0 || bits == 15 {
                        continue;
                    }

                    let lines = cell_lines(bits);
                    let to_point = |p: [f32; 2]| {
                        Point::new(
                            (p[0] as f64 + x as f64 + 0.5) * w0,
                            (p[1] as f64 + y as f64 + 0.5) * h0,
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
            for y in 0..ys - 1 {
                for x in 0..xs - 1 {
                    let bits = pick_bits(field, (x as isize, y as isize));

                    if bits == 0 || bits == 15 {
                        continue;
                    }

                    let idx = (cell_polygon_index(bits) / 4) as usize;

                    let poly = CELL_POLYGON_BUFFER[idx as usize];
                    let mut path = BezPath::new();
                    let to_point = |i: usize| {
                        Point::new(
                            (poly[i * 2] as f64) / 2. + 1. + x as f64,
                            (poly[i * 2 + 1] as f64) / 2. + 1. + y as f64,
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

    *data.render_stats.borrow_mut() = format!("Drawn {} contours", contours);
}
