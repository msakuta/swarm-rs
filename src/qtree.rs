use std::{
    collections::{BinaryHeap, HashMap},
    fmt::Display,
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
            println!($fmt, $($args)*);
        }
    };
}

pub(crate) type Rect = [i32; 4];

#[derive(Debug)]
pub(crate) struct QTree {
    toplevel: usize,
    pub levels: Vec<HashMap<[i32; 2], CellState>>,
}

#[derive(Debug, Clone)]
pub(crate) enum CellState {
    Occupied,
    Free,
    Mixed,
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

    pub fn update(&mut self, shape: (usize, usize), f: &impl Fn(Rect) -> CellState) {
        let toplevel = shape.0.max(shape.1);
        let Some(topbit) = (0..64).rev().find(|bit| {
            toplevel & (1 << bit) != 0
        }) else { return };
        self.toplevel = topbit;
        dbg_println!("maxlevel: {topbit}");
        self.recurse_update(0, topbit, [0, 0], f);

        for (i, cell) in self.levels.iter().enumerate() {
            dbg_println!("level {i}: {}", cell.len());
        }
    }

    fn recurse_update(
        &mut self,
        level: usize,
        maxlevel: usize,
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
        if maxlevel <= level || matches!(cell_state, CellState::Occupied | CellState::Free) {
            self.insert(level, parent, cell_state);
            return;
        }
        for x in 0..2i32 {
            for y in 0..2i32 {
                self.insert(level, parent, CellState::Mixed);
                self.recurse_update(
                    level + 1,
                    maxlevel,
                    [parent[0] * 2 + x, parent[1] * 2 + y],
                    f,
                );
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
                Some(CellState::Free) => return Some((level, CellState::Free)),
                Some(CellState::Occupied) => return Some((level, CellState::Occupied)),
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

impl QTree {
    pub fn path_find(&self, start: [f64; 2], end: [f64; 2]) -> (Option<QTreePath>, SearchTree) {
        let Some(start_found) = self.find(start) else {
            return (None, SearchTree::new())
        };
        if matches!(start_found.1, CellState::Occupied) {
            dbg_println!("Start position {start:?} was occupied!");
            return (None, SearchTree::new());
        }
        let Some(end_found) = self.find(end) else {
            return (None, SearchTree::new())
        };
        if matches!(end_found.1, CellState::Occupied) {
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
                dbg_println!("  Searching from {state} of {}", open_set.len());
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
                    dbg_println!(
                        "    Neighbor: [{nei_level}]{nei_idx:?}: {cell:?}, new_cost: {new_cost}"
                    );
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

    use super::{CellState, SearchTree};

    pub(crate) fn paint_qtree(ctx: &mut PaintCtx, data: &AppData, view_transform: &Affine) {
        let qtree = &data.game.qtree;

        // dbg!(data.global_render_time);
        // let cur = (data.global_render_time / 1000.).rem_euclid(8.) as usize;

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
                    x as f64,
                    y as f64,
                    x as f64 + width as f64,
                    y as f64 + width as f64,
                );
                let rect = rect.to_path(1.);
                ctx.stroke(
                    *view_transform * rect,
                    &match state {
                        CellState::Occupied => Color::rgb8(255, 127, 127),
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
