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

pub mod cache_map;
pub mod qtree;
// pub mod render;

pub use self::cache_map::FRESH_TICKS;

use std::{error::Error, fmt::Display};

use crate::collision::Aabb;

use self::{
    cache_map::CacheMap,
    qtree::{PathFindError, QTree},
};

const DEBUG: bool = false;

pub(crate) type Rect = [i32; 4];

/// A navigation mesh that can find a path and update quickly using quad tree.
///
/// It consists of two parts; actual quad tree and bitmap cache to allow updating
/// existing quad tree quickly.
#[derive(Debug)]
pub struct QTreeSearcher {
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

    pub fn get_cache_map(&self) -> &CacheMap {
        &self.cache_map
    }

    pub fn get_qtree(&self) -> &QTree {
        &self.qtree
    }

    pub(crate) fn initialize(
        &mut self,
        shape: (usize, usize),
        f: &impl Fn(Rect) -> CellState,
    ) -> Result<(), Box<dyn Error>> {
        let max_size = shape.0.max(shape.1);
        let topbit = log2ceil(max_size)?;

        self.qtree.toplevel = topbit;

        self.cache_map.cache(topbit, shape, f);

        dbg_println!("maxlevel: {topbit}");

        self.qtree
            .recurse_update(0, [0, 0], &|rect| self.cache_map.query(rect));

        for (i, cell) in self.qtree.levels.iter().enumerate() {
            dbg_println!("level {i}: {}", cell.len());
        }

        Ok(())
    }

    pub(crate) fn find(&self, pos: [f64; 2]) -> Option<(usize, CellState)> {
        self.qtree.find(pos)
    }

    pub fn start_update(&mut self) {
        self.cache_map.start_update();
    }

    pub(crate) fn update(&mut self, pos: [i32; 2], pix: CellState) -> Result<(), String> {
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

    pub(crate) fn path_find(
        &self,
        ignore_id: &[usize],
        start: [f64; 2],
        end: [f64; 2],
        goal_radius: f64,
    ) -> (Result<QTreePath, PathFindError>, SearchTree) {
        self.qtree.path_find(ignore_id, start, end, goal_radius)
    }

    pub fn check_collision(&self, aabb: &Aabb) -> bool {
        for x in aabb[0].floor() as i32..aabb[2].ceil() as i32 {
            for y in aabb[1].floor() as i32..aabb[3].ceil() as i32 {
                if let Some((_, cell)) = self.qtree.find_by_idx([x, y]) {
                    if matches!(cell, CellState::Obstacle) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellState {
    Obstacle,
    Occupied(usize),
    Free,
    Mixed,
}

#[derive(Debug, Clone, Copy)]
pub struct QTreePathNode {
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
pub struct SearchTree {
    pub nodes: Vec<[f64; 2]>,
    pub edges: Vec<[usize; 2]>,
}

impl SearchTree {
    fn new() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
        }
    }

    pub fn get_nodes(&self) -> &[[f64; 2]] {
        &self.nodes
    }

    pub fn get_edges(&self) -> &[[usize; 2]] {
        &self.edges
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Log2CeilError;

impl Display for Log2CeilError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Log2 cannot receive 0")
    }
}

impl Error for Log2CeilError {}

fn log2ceil(u: usize) -> Result<usize, Log2CeilError> {
    if u < 2 {
        Err(Log2CeilError)
    } else {
        Ok(((u - 1).ilog2() + 1) as usize)
    }
}

#[test]
fn log2test() {
    assert!(log2ceil(0usize).is_err());
    assert!(log2ceil(1usize).is_err());
    assert_eq!(log2ceil(2usize), Ok(1));
    assert_eq!(log2ceil(3usize), Ok(2));
    assert_eq!(log2ceil(4usize), Ok(2));
    assert_eq!(log2ceil(5usize), Ok(3));
}
