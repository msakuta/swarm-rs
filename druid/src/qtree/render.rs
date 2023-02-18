use druid::{
    kurbo::{BezPath, Shape},
    Affine, Color, Env, PaintCtx, Rect, RenderContext,
};

use crate::{app_data::AppData, paint_board::to_point};

use ::swarm_rs::qtree::{cache_map::FRESH_TICKS, CellState, SearchTree};

pub(crate) fn paint_qtree(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
    let qtree_searcher = &data.game.borrow().qtree;

    let width = 1;
    const CELL_MARGIN: f64 = 0.1;

    for (&[x, y], &freshness) in &qtree_searcher.get_cache_map().fresh_cells {
        let rect = Rect::new(
            x as f64 + CELL_MARGIN,
            y as f64 + CELL_MARGIN,
            x as f64 + width as f64 - CELL_MARGIN,
            y as f64 + width as f64 - CELL_MARGIN,
        );
        let rect = rect.to_path(1.);
        let color = match qtree_searcher.get_cache_map().get([x, y]) {
            CellState::Obstacle => (255, 127, 127),
            CellState::Occupied(_) => (255, 127, 255),
            CellState::Free => (0, 255, 127),
            _ => (255, 0, 255),
        };
        let brush = Color::rgba8(
            color.0,
            color.1,
            color.2,
            (freshness * 127 / FRESH_TICKS) as u8,
        );
        ctx.fill(*view_transform * rect, &brush);
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
            let rect = Rect::new(
                x as f64 + CELL_MARGIN,
                y as f64 + CELL_MARGIN,
                x as f64 + width as f64 - CELL_MARGIN,
                y as f64 + width as f64 - CELL_MARGIN,
            );
            let rect = rect.to_path(1.);
            ctx.stroke(
                *view_transform * rect,
                &match state {
                    CellState::Obstacle => Color::rgb8(255, 127, 127),
                    CellState::Occupied(_) => Color::rgb8(255, 127, 255),
                    CellState::Free => Color::rgb8(0, 255, 127),
                    _ => Color::rgb8(255, 0, 255),
                },
                1.,
            );
        }
    }
}

pub trait DruidRender {
    fn render(
        &self,
        ctx: &mut PaintCtx,
        _env: &Env,
        view_transform: &Affine,
        _brush: &Color,
        _scale: f64,
    );
}

impl DruidRender for SearchTree {
    fn render(
        &self,
        ctx: &mut PaintCtx,
        _env: &Env,
        view_transform: &Affine,
        _brush: &Color,
        _scale: f64,
    ) {
        let brush = Color::WHITE;
        let mut path = BezPath::new();
        for [start, end] in &self.edges {
            path.move_to(to_point(self.nodes[*start]));
            path.line_to(to_point(self.nodes[*end]));
        }
        ctx.stroke(*view_transform * path, &brush, 0.5);
    }
}
