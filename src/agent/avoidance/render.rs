use crate::{
    agent::{interpolation::lerp, AgentClass},
    paint_board::to_point,
};

use super::{sampler::REWIRE_DISTANCE, SearchState, CELL_SIZE, DIST_RADIUS};
use druid::{
    kurbo::{Circle, Shape},
    piet::kurbo::BezPath,
    widget::{Checkbox, Flex},
    Affine, Color, Data, Env, FontDescriptor, FontFamily, Lens, PaintCtx, Point, Rect,
    RenderContext, TextLayout, Widget, WidgetExt,
};

#[derive(Clone, Lens, Data)]
pub(crate) struct AvoidanceRenderParams {
    pub visible: bool,
    pub circle_visible: bool,
    pub shape_visible: bool,
    pub cost_visible: bool,
    pub grid_visible: bool,
}

impl AvoidanceRenderParams {
    pub fn new() -> Self {
        Self {
            visible: true,
            circle_visible: false,
            shape_visible: false,
            cost_visible: false,
            grid_visible: false,
        }
    }

    pub fn gen_widgets() -> impl Widget<Self> {
        Flex::row()
            .with_child(Checkbox::new("Avoidance").lens(Self::visible))
            .with_child(Checkbox::new("Circle").lens(Self::circle_visible))
            .with_child(Checkbox::new("Shape").lens(Self::shape_visible))
            .with_child(Checkbox::new("Cost").lens(Self::cost_visible))
            .with_child(Checkbox::new("Grid").lens(Self::grid_visible))
    }
}

impl SearchState {
    pub(crate) fn render(
        &self,
        ctx: &mut PaintCtx,
        env: &Env,
        view_transform: &Affine,
        params: &AvoidanceRenderParams,
        _brush: &Color,
        scale: f64,
    ) {
        if params.grid_visible {
            for (cell, _count) in self.grid_map.iter() {
                let (x, y) = (cell[0] as f64, cell[1] as f64);
                let rect = Rect::new(
                    x * CELL_SIZE,
                    y * CELL_SIZE,
                    (x + 1.) * CELL_SIZE,
                    (y + 1.) * CELL_SIZE,
                );
                let rect = rect.to_path(0.);
                ctx.stroke(*view_transform * rect, &Color::PURPLE, 1.);
            }
        }

        // let rgba = brush.as_rgba8();
        // let brush = Color::rgba8(rgba.0 / 2, rgba.1 / 2, rgba.2 / 2, rgba.3);
        for (direction, brush) in [Color::WHITE, Color::rgb8(255, 127, 127)]
            .iter()
            .enumerate()
        {
            for mode in 0..=2 {
                let (brush, pruned, cycle) = match mode {
                    0 => (brush.clone(), false, false),
                    1 => {
                        let rgba = brush.as_rgba8();
                        (
                            Color::rgba8(rgba.0, rgba.1, rgba.2, rgba.3 / 4),
                            true,
                            false,
                        )
                    }
                    2 => (Color::rgb8(255, 0, 255), false, true),
                    _ => unreachable!(),
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
                        if state.cycle ^ cycle {
                            continue;
                        }
                        if !cycle {
                            if (state.pruned || state.blocked) ^ pruned {
                                continue;
                            }
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

                            if params.circle_visible && 0 < level {
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
                            if params.shape_visible && 0 < level {
                                if let Some(vertices) = state
                                    .state
                                    .collision_shape(AgentClass::Worker)
                                    .to_vertices()
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
                        if 20. < scale * DIST_RADIUS && params.cost_visible {
                            let mut layout = TextLayout::<String>::from_text(format!(
                                "{}: {:.01}",
                                state.id, state.cost
                            ));
                            layout.set_font(
                                FontDescriptor::new(FontFamily::SANS_SERIF).with_size(10.0),
                            );
                            layout.set_text_color(brush.clone());
                            layout.rebuild_if_needed(ctx.text(), env);
                            layout.draw(ctx, *view_transform * to_point(state.state.into()));
                        }
                        if state.from.is_none() {
                            let circle = Circle::new(*view_transform * point, 2.);
                            ctx.fill(circle, &brush);
                        } else if params.circle_visible {
                            let circle = Circle::new(*view_transform * point, 2. + level_width);
                            ctx.fill(circle, &brush);
                            let circle = Circle::new(point, DIST_RADIUS);
                            ctx.stroke(*view_transform * circle, &brush, 0.5);
                            let circle = Circle::new(point, REWIRE_DISTANCE);
                            ctx.stroke(*view_transform * circle, &brush, 0.3);
                        }
                    }
                    ctx.stroke(*view_transform * bez_path, &brush, 0.5 + level_width);
                }
            }
        }
    }
}
