mod sampler;
mod search;

use std::{cell::RefCell, collections::HashSet};

use cgmath::{MetricSpace, Vector2};

use self::{sampler::Sampler, search::search};
use super::{
    interpolation::interpolate, wrap_angle, Agent, GameEnv, AGENT_HALFLENGTH, AGENT_HALFWIDTH,
};
use crate::{
    collision::{CollisionShape, Obb},
    entity::Entity,
    game::Game,
    measure_time,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct AgentState {
    pub x: f64,
    pub y: f64,
    pub heading: f64,
}

impl AgentState {
    pub fn new(x: f64, y: f64, heading: f64) -> Self {
        Self { x, y, heading }
    }

    pub(crate) fn collision_shape(&self) -> CollisionShape {
        CollisionShape::BBox(Obb {
            center: Vector2::new(self.x, self.y),
            xs: AGENT_HALFLENGTH,
            ys: AGENT_HALFWIDTH,
            orient: self.heading,
        })
    }

    pub(crate) fn with_orient(&self, orient: f64) -> Self {
        let mut copy = *self;
        copy.heading = orient;
        copy
    }
}

impl From<AgentState> for [f64; 2] {
    fn from(s: AgentState) -> Self {
        [s.x, s.y]
    }
}

impl From<AgentState> for Vector2<f64> {
    fn from(s: AgentState) -> Self {
        Self::new(s.x, s.y)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PathNode {
    pub x: f64,
    pub y: f64,
    pub backward: bool,
}

impl From<[f64; 2]> for PathNode {
    fn from(a: [f64; 2]) -> Self {
        Self {
            x: a[0],
            y: a[1],
            backward: false,
        }
    }
}

impl From<PathNode> for [f64; 2] {
    fn from(a: PathNode) -> Self {
        [a.x, a.y]
    }
}

impl From<&StateWithCost> for PathNode {
    fn from(node: &StateWithCost) -> Self {
        PathNode {
            x: node.state.x,
            y: node.state.y,
            backward: node.speed < 0.,
        }
    }
}

impl From<PathNode> for Vector2<f64> {
    fn from(node: PathNode) -> Self {
        Self::new(node.x, node.y)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StateWithCost {
    state: AgentState,
    cost: f64,
    speed: f64,
    id: usize,
    steer: f64,
    /// The maximum recursion level to determine collision. Used for debugging
    max_level: usize,
    from: Option<usize>,
    to: Vec<usize>,
    pruned: bool,
    blocked: bool,
}

impl StateWithCost {
    pub(crate) fn new(state: AgentState, cost: f64, steer: f64, speed: f64) -> Self {
        Self {
            state,
            cost,
            steer,
            speed,
            id: 0,
            max_level: 0,
            from: None,
            to: vec![],
            pruned: false,
            blocked: false,
        }
    }

    fn is_passable(&self) -> bool {
        !self.blocked && !self.pruned
    }
}

pub const DIST_RADIUS: f64 = 0.5 * 3.;
const DIST_THRESHOLD: f64 = DIST_RADIUS * DIST_RADIUS;

fn compare_state(s1: &AgentState, s2: &AgentState) -> bool {
    let delta_angle = wrap_angle(s1.heading - s2.heading);
    // println!("compareState deltaAngle: {}", deltaAngle);
    compare_distance(s1, s2, DIST_THRESHOLD) && delta_angle.abs() < std::f64::consts::PI / 6.
}

fn compare_distance(s1: &AgentState, s2: &AgentState, threshold: f64) -> bool {
    let delta_x = s1.x - s2.x;
    let delta_y = s1.y - s2.y;
    delta_x * delta_x + delta_y * delta_y < threshold
}

const MAX_STEER: f64 = std::f64::consts::PI / 3.;

#[derive(Debug)]
pub struct SearchState {
    search_tree: Vec<StateWithCost>,
    start_set: HashSet<usize>,
    goal: AgentState,
    pub(super) found_path: Option<Vec<usize>>,
}

impl SearchState {
    pub(crate) fn avoidance_path(&self) -> Option<impl Iterator<Item = PathNode> + '_> {
        self.found_path
            .as_ref()
            .map(|path| path.iter().map(|node| (&self.search_tree[*node]).into()))
    }
}

impl Agent {
    pub(super) fn step_move(px: f64, py: f64, heading: f64, steer: f64, motion: f64) -> AgentState {
        let [x, y] = [motion, 0.];
        let heading = heading + steer.min(1.).max(-1.) * x * 0.2 * MAX_STEER;
        let dx = heading.cos() * x - heading.sin() * y + px;
        let dy = heading.sin() * x + heading.cos() * y + py;
        AgentState {
            x: dx,
            y: dy,
            heading: wrap_angle(heading),
        }
    }

    pub(super) fn to_state(&self) -> AgentState {
        AgentState {
            x: self.pos[0],
            y: self.pos[1],
            heading: self.orient,
        }
    }

    pub(super) fn follow_avoidance_path(
        &mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
    ) -> bool {
        let Some(ref mut ss) = self.search_state else { return false };
        let Some(ref mut found_path) = ss.found_path else { return false };
        if let Some(target) = found_path.last() {
            let target_state = &ss.search_tree[*target];
            let state = target_state.state;
            let speed = target_state.speed;
            if DIST_RADIUS.powf(2.) < Vector2::from(state).distance2(Vector2::from(self.pos)) {
                self.move_to(game, state.into(), speed < 0., entities)
                    .into()
            } else {
                if let Some(prev_start) = found_path.pop() {
                    ss.start_set.remove(&prev_start);
                    if let Some(node) = found_path.last() {
                        ss.start_set.insert(*node);
                        // println!("follow_avoidance_path Start set to {:?}", ss.start);
                    }
                    let (_, time) = measure_time(|| self.prune_unreachable());
                    println!("prune_unreachable: {time:?}");
                }
                true
            }
        } else {
            false
        }
    }
}

struct SearchEnv<'a> {
    game: &'a Game,
    switch_back: bool,
    expand_states: usize,
    skipped_nodes: usize,
    tree_size: usize,
    entities: &'a [RefCell<Entity>],
}

impl Agent {
    /// RRT* search
    ///
    /// Returns true if the path is found
    pub(super) fn avoidance_search(
        &mut self,
        game: &Game,
        entities: &[RefCell<Entity>],
        callback: impl Fn(&StateWithCost, &StateWithCost),
        backward: bool,
        switch_back: bool,
    ) -> bool {
        // println!(
        //     "search invoked: state: {} goal: {:?}",
        //     self.search_state.is_some(),
        //     self.goal
        // );

        // Restart search if the target has diverged
        if let Some((search_state, goal)) = self.search_state.as_ref().zip(self.goal) {
            if !compare_state(&search_state.goal, &goal) {
                self.search_state = None;
            }
        }

        // if self
        //     .search_state
        //     .as_ref()
        //     .map(|ss| ss.found_path.is_some())
        //     .unwrap_or(false)
        // {
        //     return true;
        // }

        let mut env = SearchEnv {
            game,
            switch_back,
            expand_states: 1,
            skipped_nodes: 0,
            tree_size: 0,
            entities,
        };

        let searched_path =
            if let Some((mut search_state, goal)) = self.search_state.take().zip(self.goal) {
                // let start_state = &search_state.search_tree[search_state.start].state;
                if
                //compare_distance(&self.to_state(), start_state, DIST_THRESHOLD * 100.) &&
                compare_distance(&goal, &search_state.goal, DIST_THRESHOLD) {
                    // for root in &search_state.searchTree {
                    //     enumTree(root, &mut nodes);
                    // }

                    let nodes = &mut search_state.search_tree;

                    // println!(
                    //     "Using existing tree with {} nodes start from {:?}",
                    //     nodes.len(),
                    //     search_state.start
                    // );

                    const SEARCH_NODES: usize = 10;

                    if 0 < nodes.len() && nodes.len() < 10000 {
                        // Descending the tree is not a good way to sample a random node in a tree, since
                        // the chances are much higher on shallow nodes. We want to give chances uniformly
                        // among all nodes in the tree, so we randomly pick one from a linear list of all nodes.
                        for _i in 0..SEARCH_NODES {
                            let path =
                                search::<Sampler>(self, &search_state.start_set, &mut env, nodes);

                            env.tree_size += 1;

                            if let Some(path) = path {
                                // println!("Materialized found path: {:?}", self.path);
                                search_state.found_path = Some(path);
                                self.search_state = Some(search_state);
                                return true;
                            }
                        }
                    }

                    // let treeSize = env.tree_size;
                    search_state.goal = goal;
                    self.search_state = Some(search_state);
                    true
                } else {
                    false
                }
            } else {
                false
            };

        if !searched_path {
            if let Some(goal) = self.goal {
                // println!("Rebuilding tree with {} nodes should be 0", nodes.len());
                let mut nodes: Vec<StateWithCost> = vec![];
                let mut root_set = HashSet::new();
                if backward || -0.1 < self.speed {
                    let root = StateWithCost::new(self.to_state(), 0., 0., 1.);
                    let root_id = nodes.len();
                    nodes.push(root.clone());
                    root_set.insert(root_id);
                };
                if backward || self.speed < 0.1 {
                    let root = StateWithCost::new(self.to_state(), 0., 0., -1.);
                    let root_id = nodes.len();
                    nodes.push(root.clone());
                    root_set.insert(root_id);
                }

                if !nodes.is_empty() {
                    let found_path = search::<Sampler>(self, &root_set, &mut env, &mut nodes);

                    let search_state = SearchState {
                        search_tree: nodes,
                        start_set: root_set,
                        goal: goal,
                        found_path,
                    };
                    // else{
                    //     *search_state = SearchState{
                    //         searchTree: roots,
                    //         treeSize: 0,
                    //         start: State{x: this.x, y: this.y, heading: this.angle},
                    //         goal: this.goal,
                    //     };
                    // }
                    // println!("Search state: {search_state:?}");
                    self.search_state = Some(search_state);
                }
            }

            // We add path nodes after connections are built, because path nodes may come from non-tree nodes and
            // path nodes do not need to be connected.
            // for node in &path {
            //     if *node < nodes.len() {
            //         let mut new_node = StateWithCost::new(
            //             State {
            //                 x: node[0],
            //                 y: node[1],
            //                 heading: node[2],
            //             },
            //             0.,
            //             0.,
            //             1.,
            //         );
            //         new_node.id = nodes.len();
            //         nodes.push(new_node);
            //     }
            // }
        }

        self.search_state
            .as_ref()
            .map(|ss| ss.found_path.is_some())
            .unwrap_or(false)
    }

    pub(super) fn prune_unreachable(&mut self) {
        let Some(ref mut ss) = self.search_state else {
            return;
        };

        let Some(ref mut found_path) = ss.found_path else {
            return;
        };

        let mut visited = vec![false; ss.search_tree.len()];

        let mut start = found_path.clone();

        // Mark initial nodes as visited
        for node in &start {
            visited[*node] = true;
        }

        while let Some(node) = start.pop() {
            // println!("node {node} to-s: {:?}", ss.search_tree[node].to);
            for to in &ss.search_tree[node].to {
                if !visited[*to] {
                    start.push(*to);
                    visited[*to] = true;
                }
            }
        }

        let mut num_pruned = 0;
        for (visited, state) in visited.into_iter().zip(&mut ss.search_tree) {
            if !visited {
                state.pruned = true;
                num_pruned += 1;
            }
        }
        println!(
            "prune_unreachable pruned: {num_pruned} / {}",
            ss.search_tree.len()
        );
    }

    /// Check existing avoidance search state with actual entity positions, and
    /// prune those states that has new collisions.
    pub(super) fn check_avoidance_collision(&mut self, env: &GameEnv) -> Option<()> {
        let ss = self.search_state.as_mut()?;

        let collision_checker =
            |state: AgentState| Agent::collision_check(Some(self.id), state, env.entities, true);

        for i in 0..ss.search_tree.len() {
            let Some(from) = ss.search_tree[i].from else { continue };
            let start_state = ss.search_tree[from].state;
            let next_state = ss.search_tree[i].state;
            ss.search_tree[i].blocked = false;
            if interpolate(
                start_state,
                next_state,
                DIST_RADIUS * 0.5,
                collision_checker,
            ) {
                ss.search_tree[i].blocked = true;
                if ss
                    .found_path
                    .as_ref()
                    .map(|path| path.iter().any(|j| *j == i))
                    .unwrap_or(false)
                {
                    ss.found_path = None;
                }
            }
        }

        let mut visited = vec![false; ss.search_tree.len()];

        let mut start = ss.start_set.clone();

        // Mark initial nodes as visited
        for node in &start {
            visited[*node] = true;
        }

        while let Some(node) = start.iter().copied().next() {
            start.remove(&node);
            // println!("node {node} to-s: {:?}", ss.search_tree[node].to);
            for to in &ss.search_tree[node].to {
                if !visited[*to] {
                    start.insert(*to);
                    visited[*to] = true;
                }
            }
        }

        let mut num_detached = 0;
        // Assign infinite cost to those "detached" nodes by the obstacle
        for (_, state) in visited
            .iter()
            .zip(ss.search_tree.iter_mut())
            .filter(|(visited, _)| !**visited)
        {
            state.cost = 1e8;
            num_detached += 1;
        }

        println!(
            "check_avoidance_collision detached: {num_detached} / {}",
            ss.search_tree.len()
        );

        Some(())
    }
}

mod render {
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
                                    let angle =
                                        (point.y - from_state.y).atan2(point.x - from_state.x);
                                    let rot =
                                        Affine::translate((*view_transform * point).to_vec2())
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
                                    if let Some(vertices) =
                                        state.state.collision_shape().to_vertices()
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
}
