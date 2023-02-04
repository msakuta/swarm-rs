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
pub mod render;

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
        goal_radius: f64,
    ) -> (Option<QTreePath>, SearchTree) {
        self.qtree.path_find(ignore_id, start, end, goal_radius)
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
    pub fn new(pos: [f64; 2], radius: f64) -> Self {
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
