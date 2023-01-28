//! Quad tree based pathfinder

macro_rules! dbg_println {
    ($fmt:literal) => {
        if crate::qtree::DEBUG {
            println!($fmt);
        }
    };
    ($fmt:literal, $($args:expr),*) => {
        if crate::qtree::DEBUG {
            println!($fmt, $($args),*);
        }
    };
}

mod cache_map;
mod qtree;

use self::{cache_map::CacheMap, qtree::QTree};

const DEBUG: bool = false;

pub(crate) type Rect = [i32; 4];

/// A navigation mesh that can find a path and update quickly using quad tree.
///
/// It consists of two parts; actual quad tree and bitmap cache to allow updating
/// existing quad tree quickly.
#[derive(Debug)]
pub(crate) struct QTreeSearcher {
    qtree: QTree,
    cache_map: CacheMap,
}

impl QTreeSearcher {
    pub(crate) fn new() -> Self {
        Self {
            qtree: QTree::new(),
            cache_map: CacheMap::new(),
        }
    }

    pub fn initialize(&mut self, shape: (usize, usize), f: &impl Fn(Rect) -> CellState) {
        let toplevel = shape.0.max(shape.1);
        let Some(topbit) = (0..std::mem::size_of::<usize>() * 8).rev().find(|bit| {
            toplevel & (1 << bit) != 0
        }) else { return };
        self.qtree.toplevel = topbit;

        self.cache_map.cache(topbit, shape, f);

        dbg_println!("maxlevel: {topbit}");

        self.qtree
            .recurse_update(0, [0, 0], &|rect| self.cache_map.query(rect));

        for (i, cell) in self.qtree.levels.iter().enumerate() {
            dbg_println!("level {i}: {}", cell.len());
        }
    }

    pub fn start_update(&mut self) {
        self.cache_map.start_update();
    }

    pub fn update(&mut self, pos: [i32; 2], pix: CellState) -> Result<(), String> {
        let res = self.cache_map.update(pos, pix)?;
        if res {
            let mut level = self.qtree.toplevel;
            let mut cell_pos = pos;
            loop {
                if let Some(tile) = self
                    .qtree
                    .levels
                    .get_mut(level)
                    .and_then(|level| level.get_mut(&cell_pos))
                {
                    if !matches!(tile, CellState::Mixed) {
                        if level == self.qtree.toplevel {
                            *tile = pix;
                        } else {
                            *tile = CellState::Mixed;
                            self.qtree.recurse_update(level, cell_pos, &|rect| {
                                let res = self.cache_map.query(rect);
                                // dbg_println!("Updating query({level}): {rect:?} -> {res:?}");
                                res
                            });
                        }
                        self.qtree.try_merge(self.qtree.toplevel, pos);
                        return Ok(());
                    }
                }
                if level == 0 {
                    return Err("Could not find a cell to update".to_string());
                }
                level -= 1;
                cell_pos = [cell_pos[0] / 2, cell_pos[1] / 2];
            }
        }
        Ok(())
    }

    pub fn finish_update(&mut self) {
        self.cache_map.finish_update();
    }

    pub fn path_find(
        &self,
        ignore_id: &[usize],
        start: [f64; 2],
        end: [f64; 2],
    ) -> (Option<QTreePath>, SearchTree) {
        self.qtree.path_find(ignore_id, start, end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellState {
    Obstacle,
    Occupied(usize),
    Free,
    Mixed,
}

#[derive(Debug)]
pub(crate) struct QTreePathNode {
    pub pos: [f64; 2],
    pub radius: f64,
}

impl QTreePathNode {
    pub fn _new(pos: [f64; 2], radius: f64) -> Self {
        Self { pos, radius }
    }

    fn new_with_qtree(idx: (usize, [i32; 2]), qtree: &QTree) -> Self {
        Self {
            pos: qtree.idx_to_center(idx),
            radius: qtree.width(idx.0) as f64 / 2.,
        }
    }
}

pub(crate) type QTreePath = Vec<QTreePathNode>;

#[derive(Debug)]
pub(crate) struct SearchTree {
    nodes: Vec<[f64; 2]>,
    edges: Vec<[usize; 2]>,
}

impl SearchTree {
    fn new() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }
}

pub mod render {
    use druid::{
        kurbo::{BezPath, Shape},
        Affine, Color, Env, PaintCtx, Rect, RenderContext,
    };

    use crate::{app_data::AppData, paint_board::to_point};

    use super::{cache_map::FRESH_TICKS, CellState, SearchTree};

    pub(crate) fn paint_qtree(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
        let qtree = &data.game.qtree;

        // dbg!(data.global_render_time);
        // let cur = (data.global_render_time / 1000.).rem_euclid(8.) as usize;

        let qtree_searcher = qtree.borrow();
        let width = 1;
        const CELL_MARGIN: f64 = 0.1;

        for (&[x, y], &freshness) in &qtree_searcher.cache_map.fresh_cells {
            let rect = Rect::new(
                x as f64 + CELL_MARGIN,
                y as f64 + CELL_MARGIN,
                x as f64 + width as f64 - CELL_MARGIN,
                y as f64 + width as f64 - CELL_MARGIN,
            );
            let rect = rect.to_path(1.);
            let color = match qtree_searcher.cache_map.get([x, y]) {
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

        let qtree = &qtree_searcher.qtree;

        for (level, cells) in qtree.levels.iter().enumerate().take(8) {
            // let level = cur;
            // let cells = &qtree.levels[level];
            // {
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

    impl SearchTree {
        pub(crate) fn render(
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
}
