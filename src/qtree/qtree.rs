use std::{
    collections::{BinaryHeap, HashMap},
    fmt::Display,
};

use super::{CellState, QTreePath, QTreePathNode, Rect, SearchTree};

/// A quad tree to divide space for navigation.
///
/// It is not actually a quad tree data structure. The algorithm is.
#[derive(Debug)]
pub(super) struct QTree {
    pub toplevel: usize,
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

    pub(super) fn recurse_update(
        &mut self,
        level: usize,
        parent: [i32; 2],
        f: &impl Fn(Rect) -> CellState,
    ) {
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

    pub(super) fn try_merge(&mut self, level: usize, cell_pos: [i32; 2]) {
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
    pub(super) fn idx_to_center(&self, idx: QTreeIdx) -> [f64; 2] {
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
            let neighbor_idx = [idx[0] + offset[0], idx[1] + offset[1]];
            let substates = self.recurse_find(level, neighbor_idx, side);
            if !substates.is_empty() {
                ret.extend_from_slice(&substates);
                continue;
            }

            // If we do not find cells in lower levels, it's likely that a super cell exists.
            // We don't need recursion unlike substates, because it's a linear search.
            let mut supidx = idx;
            let mut neighbor_supidx = neighbor_idx;
            let ancestor = 'ancestor: {
                for suplevel in (0..level).rev() {
                    supidx[0] /= 2;
                    supidx[1] /= 2;
                    neighbor_supidx[0] /= 2;
                    neighbor_supidx[1] /= 2;
                    if supidx == neighbor_supidx {
                        break 'ancestor None;
                    }
                    if let Some(found_parent) = self.levels[suplevel].get(&neighbor_supidx) {
                        break 'ancestor Some((suplevel, neighbor_supidx, found_parent));
                    }
                }
                None
            };
            if let Some(ancestor) = ancestor {
                // dbg_println!("    Searching up: {level}, {idx:?}, {side:?}");
                ret.extend_from_slice(&self.recurse_find(ancestor.0, ancestor.1, side));
            }
        }
        ret
    }
}

impl QTree {
    pub fn path_find(
        &self,
        ignore_id: &[usize],
        start: [f64; 2],
        end: [f64; 2],
        goal_radius: f64,
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
                    // The last node should directly connect to the goal
                    // path.push(QTreePathNode::new_with_qtree(end_idx, self));
                    path.push(QTreePathNode::new(end, goal_radius));
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

        (None, self.build_search_tree(closed_set))
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
