use std::{
    collections::{BinaryHeap, HashMap},
    fmt::Display,
    sync::atomic::{AtomicUsize, Ordering},
};

const DEBUG: bool = false;

macro_rules! dbg_println {
    ($fmt:literal) => {
        if DEBUG {
            println!($fmt);
        }
    };
    ($fmt:literal, $($args:expr),*) => {
        if DEBUG {
            println!($fmt, $($args),*);
        }
    };
}

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

/// A bitmap to update quad tree using only changed parts of the map.
///
/// It has array index indirection to reduce the map size, since CellState tends to be
/// much larger than u32.
#[derive(Debug)]
struct CacheMap {
    /// An internal map having the size (2 ^ toplevel) ^ 2, indicating index into [`cache_buf`]
    map: Vec<u32>,
    /// An array of actual values in [`cache_map`], extracted to reduce the pixel size.
    buf: Vec<CellState>,
    topbit: usize,
    size: usize,
    /// A history of recently updated cells for visualization
    fresh_cells: HashMap<[i32; 2], usize>,
    prev_map: Option<Vec<u32>>,
}

const FRESH_TICKS: usize = 8;

static QUERY_CALLS: AtomicUsize = AtomicUsize::new(0);
static UNPASSABLES: AtomicUsize = AtomicUsize::new(0);

impl CacheMap {
    fn new() -> Self {
        Self {
            map: vec![],
            buf: vec![],
            topbit: 0,
            size: 0,
            fresh_cells: HashMap::new(),
            prev_map: None,
        }
    }

    fn get(&self, pos: [i32; 2]) -> CellState {
        self.buf[self.map[pos[0] as usize + pos[1] as usize * self.size] as usize]
    }

    fn cache(&mut self, topbit: usize, shape: (usize, usize), f: &impl Fn(Rect) -> CellState) {
        self.topbit = topbit;
        self.size = 1 << topbit;
        self.map = vec![0; self.size * self.size];
        for y in 0..shape.0 {
            for x in 0..shape.1 {
                let pix = f([x as i32, y as i32, x as i32 + 1, y as i32 + 1]);
                if let Some((idx, _)) = self.buf.iter().enumerate().find(|(_, b)| **b == pix) {
                    self.map[x + y * self.size] = idx as u32;
                } else {
                    self.map[x + y * self.size] = self.buf.len() as u32;
                    self.buf.push(pix);
                }
            }
        }
        dbg_println!(
            "cache_map size: {}, cache_buf: {:?}",
            std::mem::size_of::<u32>() * self.map.len(),
            self.buf
        );
    }

    fn start_update(&mut self) {
        self.prev_map = Some(self.map.clone());
    }

    fn update(&mut self, pos: [i32; 2], pix: CellState) -> Result<bool, String> {
        if pos[0] < 0 || self.size as i32 <= pos[0] || pos[1] < 0 || self.size as i32 <= pos[1] {
            return Err("Out of bounds!".to_string());
        }

        let existing = &mut self.map[pos[0] as usize + pos[1] as usize * self.size];

        if self.buf[*existing as usize] != pix {
            // dbg_println!("Updating cache_map {pos:?}: {:?} -> {:?}", *existing, pix);

            if let Some((idx, _)) = self.buf.iter().enumerate().find(|(_, buf)| **buf == pix) {
                *existing = idx as u32;
            } else {
                let idx = self.buf.len();
                self.buf.push(pix);
                *existing = idx as u32;
            }

            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn finish_update(&mut self) {
        if let Some(prev_map) = &self.prev_map {
            for i in
                self.map
                    .iter()
                    .zip(prev_map.iter())
                    .enumerate()
                    .filter_map(
                        |(i, (cur, prev))| {
                            if *cur != *prev {
                                Some(i)
                            } else {
                                None
                            }
                        },
                    )
            {
                self.fresh_cells.insert(
                    [(i % self.size) as i32, (i / self.size) as i32],
                    FRESH_TICKS,
                );
            }
        }
        let mut to_delete = vec![];
        for fresh_cell in &mut self.fresh_cells {
            if 1 < *fresh_cell.1 {
                *fresh_cell.1 -= 1;
            } else {
                to_delete.push(*fresh_cell.0);
            }
        }

        for to_delete in to_delete {
            self.fresh_cells.remove(&to_delete);
        }
    }

    fn query(&self, rect: Rect) -> CellState {
        let mut has_passable = false;
        let mut has_unpassable = None;
        for x in rect[0]..rect[2] {
            for y in rect[1]..rect[3] {
                QUERY_CALLS.fetch_add(1, Ordering::Relaxed);
                let mut has_unpassable_local = None;
                let pix = &self.buf[self.map[x as usize + y as usize * self.size] as usize];
                if !matches!(pix, CellState::Free) {
                    UNPASSABLES.fetch_add(1, Ordering::Relaxed);
                    has_unpassable_local = Some(*pix);
                }
                if has_unpassable_local.is_none() {
                    has_passable = true;
                }
                if has_unpassable.is_none() {
                    has_unpassable = has_unpassable_local;
                }
                if has_passable && has_unpassable.is_some() {
                    return CellState::Mixed;
                }
            }
        }
        if has_passable {
            CellState::Free
        } else if let Some(state) = has_unpassable {
            state
        } else {
            CellState::Obstacle
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CellState {
    Obstacle,
    Occupied(usize),
    Free,
    Mixed,
}

/// A quad tree to divide space for navigation.
///
/// It is not actually a quad tree data structure. The algorithm is.
#[derive(Debug)]
struct QTree {
    toplevel: usize,
    pub levels: Vec<HashMap<[i32; 2], CellState>>,
}

impl QTree {
    pub fn new() -> Self {
        Self {
            toplevel: 0,
            levels: vec![],
        }
    }

    pub fn width(&self, level: usize) -> usize {
        1 << (self.toplevel - level)
    }

    fn recurse_update(&mut self, level: usize, parent: [i32; 2], f: &impl Fn(Rect) -> CellState) {
        let width = self.width(level) as i32;
        let rect = [
            parent[0] * width,
            parent[1] * width,
            (parent[0] + 1) * width,
            (parent[1] + 1) * width,
        ];
        if level <= 2 {
            dbg_println!("level: {level}, rect: {rect:?}");
        }
        let cell_state = f(rect);
        if self.toplevel <= level || !matches!(cell_state, CellState::Mixed) {
            self.insert(level, parent, cell_state);
            return;
        }
        self.insert(level, parent, CellState::Mixed);
        for x in 0..2i32 {
            for y in 0..2i32 {
                self.recurse_update(level + 1, [parent[0] * 2 + x, parent[1] * 2 + y], f);
            }
        }
    }

    fn try_merge(&mut self, level: usize, cell_pos: [i32; 2]) {
        if 1 <= level {
            let super_pixels = || {
                (cell_pos[0] / 2 * 2..=cell_pos[0] / 2 * 2 + 1)
                    .map(|ix| {
                        (cell_pos[1] / 2 * 2..=cell_pos[1] / 2 * 2 + 1).map(move |iy| [ix, iy])
                    })
                    .flatten()
            };

            #[derive(Debug, PartialEq, Eq)]
            enum Homogeneity {
                Homogeneous(CellState),
                Heterogeneous,
            }

            let homogeneous = self.levels.get(level).and_then(|level| {
                super_pixels()
                    .map(|pos| {
                        level
                            .get(&pos)
                            .map(|state| Homogeneity::Homogeneous(*state))
                    })
                    .reduce(|acc, cur| {
                        if let Some((acc, cur)) = acc.zip(cur) {
                            if acc == cur {
                                Some(acc)
                            } else {
                                Some(Homogeneity::Heterogeneous)
                            }
                        } else {
                            None
                        }
                    })
                    .flatten()
            });
            dbg_println!(
                "homogeneous? {homogeneous:?} for {:?}: {:?}",
                super_pixels().collect::<Vec<_>>(),
                super_pixels()
                    .map(|pos| self.levels.get(level).and_then(|level| level.get(&pos)))
                    .collect::<Vec<_>>()
            );
            match homogeneous {
                Some(Homogeneity::Homogeneous(pix)) => {
                    for pos in super_pixels() {
                        if let Some(level) = self.levels.get_mut(level) {
                            level.remove(&pos);
                        }
                    }
                    self.levels
                        .get_mut(level - 1)
                        .and_then(|level| level.insert([cell_pos[0] / 2, cell_pos[1] / 2], pix));
                    self.try_merge(level - 1, [cell_pos[0] / 2, cell_pos[1] / 2])
                }
                Some(Homogeneity::Heterogeneous) => (),
                None => self.try_merge(level - 1, [cell_pos[0] / 2, cell_pos[1] / 2]),
            }
        }
    }

    fn insert(&mut self, level: usize, pos: [i32; 2], state: CellState) {
        if self.levels.len() <= level + 1 {
            self.levels.resize(level + 1, HashMap::new());
        }
        self.levels[level].insert(pos, state);
    }

    fn pos_to_idx(&self, pos: [f64; 2], level: usize) -> [i32; 2] {
        let scale = |f| (f / (1 << (self.toplevel - level)) as f64) as i32;
        [scale(pos[0]), scale(pos[1])]
    }

    fn to_idx(&self, pos: [i32; 2], level: usize) -> [i32; 2] {
        let scale = |f| f / (1 << (self.toplevel - level));
        [scale(pos[0]), scale(pos[1])]
    }

    pub fn find(&self, pos: [f64; 2]) -> Option<(usize, CellState)> {
        self.find_by_idx([pos[0] as i32, pos[1] as i32])
    }

    /// Find by index at the bottom level
    pub fn find_by_idx(&self, pos: [i32; 2]) -> Option<(usize, CellState)> {
        // let width = 1 << self.toplevel;
        for (level, cells) in self.levels.iter().enumerate().rev() {
            let cell_pos = self.to_idx(pos, level);
            let cell = cells.get(&cell_pos);
            // println!("find({cell_pos:?}): {cell:?}");
            match cell {
                Some(CellState::Obstacle) => return Some((level, CellState::Obstacle)),
                Some(CellState::Free) => return Some((level, CellState::Free)),
                Some(CellState::Occupied(id)) => return Some((level, CellState::Occupied(*id))),
                _ => continue,
            }
        }
        return None;
    }
}

#[derive(Debug, Clone, Copy)]
enum Side {
    Left,
    Top,
    Right,
    Bottom,
}

type QTreeIdx = (usize, [i32; 2]);

struct ClosedState {
    cost: f64,
    came_from: Option<QTreeIdx>,
}

impl QTree {
    /// Convert a tree index into coordinates of the center of the cell
    fn idx_to_center(&self, idx: QTreeIdx) -> [f64; 2] {
        let width = self.width(idx.0) as f64;
        [
            (idx.1[0] as f64 + 0.5) * width,
            (idx.1[1] as f64 + 0.5) * width,
        ]
    }

    fn recurse_find(&self, level: usize, idx: [i32; 2], side: Side) -> Vec<(usize, [i32; 2])> {
        if self.levels.len() <= level {
            return vec![];
        }
        let same_level = self.levels[level].get(&idx);
        if same_level
            .map(|cell| matches!(cell, CellState::Mixed))
            .unwrap_or(true)
        {
            let mut ret = vec![];
            let (x, y) = (idx[0] * 2, idx[1] * 2);
            let subcells = match &side {
                Side::Left => [[x, y], [x, y + 1]],
                Side::Top => [[x, y], [x + 1, y]],
                Side::Right => [[x + 1, y], [x + 1, y + 1]],
                Side::Bottom => [[x, y + 1], [x + 1, y + 1]],
            };
            for subcell in subcells {
                ret.extend_from_slice(&self.recurse_find(level + 1, subcell, side));
            }
            ret
        } else if same_level.is_some() {
            // dbg_println!("    Same level: {level}, {idx:?}, {side:?}");
            vec![(level, idx)]
        } else {
            vec![]
        }
    }

    fn find_neighbors(&self, level: usize, idx: [i32; 2]) -> Vec<(usize, [i32; 2])> {
        let mut ret = vec![];
        for (side, offset) in [
            (Side::Left, [1, 0]),
            (Side::Top, [0, 1]),
            (Side::Right, [-1, 0]),
            (Side::Bottom, [0, -1]),
        ] {
            let subidx = [idx[0] + offset[0], idx[1] + offset[1]];
            let substates = self.recurse_find(level, subidx, side);
            ret.extend_from_slice(&substates);
            let supidx = [subidx[0] / 2, subidx[1] / 2];
            if substates.is_empty() && supidx != [idx[0] / 2, idx[1] / 2] {
                let mut parent = (level - 1, supidx);
                let mut ancestor = None;
                while 0 < parent.0 {
                    if let Some(found_parent) = self.levels[parent.0].get(&parent.1) {
                        ancestor = Some((parent.0, parent.1, found_parent));
                        break;
                    }
                    parent = (level - 1, [idx[0] / 2, idx[1] / 2]);
                }
                if let Some(ancestor) = ancestor {
                    // dbg_println!("    Searching up: {level}, {idx:?}, {side:?}");
                    ret.extend_from_slice(&self.recurse_find(ancestor.0, ancestor.1, side));
                }
            }
        }
        ret
    }
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

impl QTree {
    pub fn path_find(
        &self,
        ignore_id: &[usize],
        start: [f64; 2],
        end: [f64; 2],
    ) -> (Option<QTreePath>, SearchTree) {
        let Some(start_found) = self.find(start) else {
            return (None, SearchTree::new())
        };
        let blocked = |state| match state {
            CellState::Obstacle => true,
            CellState::Occupied(id) => !ignore_id.iter().any(|i| *i == id),
            _ => false,
        };
        if blocked(start_found.1) {
            dbg_println!("Start position {start:?} was occupied!");
            return (None, SearchTree::new());
        }
        let Some(end_found) = self.find(end) else {
            return (None, SearchTree::new())
        };
        if blocked(end_found.1) {
            dbg_println!("End position {start:?} was occupied!");
            return (None, SearchTree::new());
        }
        let end_idx = (
            end_found.0,
            [
                end[0] as i32 / self.width(end_found.0) as i32,
                end[1] as i32 / self.width(end_found.0) as i32,
            ],
        );

        #[derive(Debug, Clone, Copy)]
        struct OpenState {
            level: usize,
            idx: [i32; 2],
            cost: f64,
        }

        impl Display for OpenState {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "[{}]{:?}", self.level, self.idx)
            }
        }

        impl PartialEq for OpenState {
            fn eq(&self, other: &Self) -> bool {
                self.level == other.level && self.idx == other.idx
            }
        }

        impl Eq for OpenState {}

        impl PartialOrd for OpenState {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                self.cost.partial_cmp(&other.cost).map(|o| o.reverse())
            }
        }

        impl Ord for OpenState {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                self.cost
                    .partial_cmp(&other.cost)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .reverse()
            }
        }

        dbg_println!("Start Searching from {start:?}");

        let mut open_set = BinaryHeap::new();

        let start_idx = self.pos_to_idx(start, start_found.0);
        let start_state = OpenState {
            level: start_found.0,
            idx: start_idx,
            cost: 0.,
        };
        open_set.push(start_state);

        let start_idx = (start_found.0, start_idx);

        if start_idx == end_idx {
            let mut path = vec![];
            path.push(QTreePathNode::new_with_qtree(end_idx, self));
            path.push(QTreePathNode::new_with_qtree(start_idx, self));
            return (Some(path), SearchTree::new());
        }

        let mut closed_set = HashMap::new();
        closed_set.insert(
            start_idx,
            ClosedState {
                cost: 0.,
                came_from: None,
            },
        );

        while let Some(state) = open_set.pop() {
            if state.cost < 10. {
                // dbg_println!("  Searching from {state} of {}", open_set.len());
            }
            for (nei_level, nei_idx) in self.find_neighbors(state.level, state.idx) {
                // let nei_idx = [state.idx[0] + neighbor[0], state.idx[1] + neighbor[1]];
                let nei_width = self.width(nei_level) as i32;
                let nei_bottom = [nei_idx[0] * nei_width, nei_idx[1] * nei_width];

                if (nei_level, nei_idx) == end_idx {
                    let mut path = vec![];
                    path.push(QTreePathNode::new_with_qtree(end_idx, self));
                    let mut node = Some((state.level, state.idx));
                    while let Some(anode) = node {
                        path.push(QTreePathNode::new_with_qtree(anode, self));
                        node = closed_set.get(&anode).and_then(|bnode| bnode.came_from);
                    }
                    return (Some(path), self.build_search_tree(closed_set));
                }
                let new_cost = state.cost + self.width(state.level) as f64;
                let cell = self.levels[nei_level].get(&nei_idx);
                if state.cost < 10. {
                    // dbg_println!(
                    //     "    Neighbor: [{nei_level}]{nei_idx:?}: {cell:?}, new_cost: {new_cost}"
                    // );
                }
                let Some(cell) = cell else {
                    continue
                };
                if !matches!(cell, CellState::Free) {
                    continue;
                }
                let cell_idx = [nei_bottom[0] / nei_width, nei_bottom[1] / nei_width];

                if closed_set
                    .get(&(nei_level, cell_idx))
                    .map(|state| state.cost <= new_cost)
                    .unwrap_or(false)
                {
                    continue;
                }
                open_set.push(OpenState {
                    level: nei_level,
                    idx: cell_idx,
                    cost: new_cost,
                });
                closed_set.insert(
                    (nei_level, cell_idx),
                    ClosedState {
                        cost: new_cost,
                        came_from: Some((state.level, state.idx)),
                    },
                );
                // } else if 0 < state.level {
                //     let sup_idx = [nei_idx[0] / 2, nei_idx[1] / 2];
                //     let new_cost = state.cost + self.width(state.level - 1) as f64;
                //     println!("Super Neighbor {nei_idx:?} -> {sup_idx:?}: new_cost: {new_cost}");
                //     if closed_set
                //         .get(&(state.level - 1, sup_idx))
                //         .map(|cost| *cost <= new_cost)
                //         .unwrap_or(false)
                //     {
                //         continue;
                //     }
                //     open_set.push(OpenState {
                //         level: state.level - 1,
                //         idx: sup_idx,
                //         cost: new_cost,
                //     });
                //     closed_set.insert((state.level - 1, sup_idx), new_cost);
            }
        }

        (Some(vec![]), self.build_search_tree(closed_set))
    }

    fn build_search_tree(&self, closed_set: HashMap<QTreeIdx, ClosedState>) -> SearchTree {
        let mut search_tree = SearchTree::new();
        for closed_state in &closed_set {
            if let Some(start) = closed_state.1.came_from {
                let start_node = search_tree.nodes.len();
                let start_width = self.width(start.0);
                search_tree.nodes.push([
                    (start.1[0] as f64 + 0.5) * start_width as f64,
                    (start.1[1] as f64 + 0.5) * start_width as f64,
                ]);
                let end_node = search_tree.nodes.len();
                let end_width = self.width(closed_state.0 .0);
                search_tree.nodes.push([
                    (closed_state.0 .1[0] as f64 + 0.5) * end_width as f64,
                    (closed_state.0 .1[1] as f64 + 0.5) * end_width as f64,
                ]);
                search_tree.edges.push([start_node, end_node]);
            }
        }
        search_tree
    }
}

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

    use super::{CellState, SearchTree, FRESH_TICKS};

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
