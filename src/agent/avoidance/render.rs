use crate::{agent::interpolation::lerp, paint_board::to_point};

use super::{SearchState, DIST_RADIUS};
use druid::{
    kurbo::Circle, piet::kurbo::BezPath, Affine, Color, Env, PaintCtx, Point, RenderContext,
};

impl SearchState {
    pub fn render(
        &self,
        ctx: &mut PaintCtx,
        _env: &Env,
        view_transform: &Affine,
        _brush: &Color,
        circle_visible: bool,
        shape_visible: bool,
        scale: f64,
    ) {
        // let rgba = brush.as_rgba8();
        // let brush = Color::rgba8(rgba.0 / 2, rgba.1 / 2, rgba.2 / 2, rgba.3);
        for (direction, brush) in [Color::WHITE, Color::rgb8(255, 127, 127)]
            .iter()
            .enumerate()
        {
            for pruned in [false, true] {
                let brush = if pruned {
                    let rgba = brush.as_rgba8();
                    Color::rgba8(rgba.0, rgba.1, rgba.2, rgba.3 / 4)
                } else {
                    brush.clone()
                };
                for level in 0..=3 {
                    let level_width = level as f64 * 0.5;
                    let mut bez_path = BezPath::new();
                    for state in &self.search_tree {
                        if state.max_level != level {
                            continue;
                        }
                        if 0. < (direction as f64 * 2. - 1.) * state.speed {
                            continue;
                        }
                        if (state.pruned || state.blocked) ^ pruned {
                            continue;
                        }
                        let point = Point::new(state.state.x, state.state.y);
                        if let Some(from) = state.from {
                            let from_state = self.search_tree[from].state;
                            bez_path.move_to(Point::new(from_state.x, from_state.y));
                            bez_path.line_to(point);

                            if 20. < scale * DIST_RADIUS {
                                let mut arrow_head = BezPath::new();
                                let angle = (point.y - from_state.y).atan2(point.x - from_state.x);
                                let rot = Affine::translate((*view_transform * point).to_vec2())
                                    * Affine::rotate(angle);
                                arrow_head.move_to(rot * Point::ZERO);
                                arrow_head.line_to(rot * Point::new(-7., 3.));
                                arrow_head.line_to(rot * Point::new(-7., -3.));
                                arrow_head.close_path();
                                ctx.fill(arrow_head, &brush);
                            }

                            if circle_visible && 0 < level {
                                let interpolates = 1 << level;
                                for i in 1..interpolates {
                                    let pos = lerp(
                                        &state.state.into(),
                                        &from_state.into(),
                                        i as f64 / interpolates as f64,
                                    );
                                    let circle = Circle::new(
                                        *view_transform * to_point(pos),
                                        2. + level_width,
                                    );
                                    ctx.fill(circle, &brush);
                                }
                            }
                            if shape_visible && 0 < level {
                                if let Some(vertices) = state.state.collision_shape().to_vertices()
                                {
                                    if let Some((first, rest)) = vertices.split_first() {
                                        let mut path = BezPath::new();
                                        path.move_to(to_point(*first));
                                        for v in rest {
                                            path.line_to(to_point(*v));
                                        }
                                        path.close_path();
                                        ctx.stroke(*view_transform * path, &brush, 0.5);
                                    }
                                }
                            }
                        }
                        if circle_visible {
                            let circle = Circle::new(*view_transform * point, 2. + level_width);
                            ctx.fill(circle, &brush);
                            let circle = Circle::new(point, DIST_RADIUS);
                            ctx.stroke(*view_transform * circle, &brush, 0.5);
                        }
                    }
                    ctx.stroke(*view_transform * bez_path, &brush, 0.5 + level_width);
                }
            }
        }
    }
}
