mod render;
mod sampler;
mod search;

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

use cgmath::{MetricSpace, Vector2};

pub(crate) use self::render::AvoidanceRenderParams;
use self::{
    sampler::{ForwardKinematicSampler, RrtStarSampler, SpaceSampler, StateSampler},
    search::{can_connect_goal, insert_to_grid_map, search, to_cell},
};
use super::{
    interpolation::interpolate, wrap_angle, Agent, GameEnv, AGENT_HALFLENGTH, AGENT_HALFWIDTH,
    AGENT_SCALE,
};
use crate::{
    collision::{CollisionShape, Obb},
    entity::Entity,
    game::{AvoidanceMode, Game},
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

pub const DIST_RADIUS: f64 = 0.5 * AGENT_SCALE;
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
const CELL_SIZE: f64 = 2.;
const MAX_CELL_COUNT: usize = 10;

/// We use a grid of cells with fixed sizes to query nodes in a search tree.
/// The benefit of grid over RTree is that RTree requires O(n log n) to build
/// the index, while grid is just O(n). We need to insert as many times as
/// query, so the insertion time needs to be small.
type GridMap = HashMap<[i32; 2], HashSet<usize>>;

#[derive(Debug)]
pub struct SearchState {
    search_tree: Vec<StateWithCost>,
    start_set: HashSet<usize>,
    goal: AgentState,
    last_solution: Option<usize>,
    pub(super) found_path: Option<Vec<usize>>,
    pub(super) grid_map: GridMap,
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

pub(super) struct SearchEnv<'a> {
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
        backward: bool,
        switch_back: bool,
        avoidance_mode: AvoidanceMode,
    ) -> bool {
        let mut env = SearchEnv {
            game,
            switch_back,
            expand_states: game.avoidance_expands as usize,
            skipped_nodes: 0,
            tree_size: 0,
            entities,
        };

        match avoidance_mode {
            AvoidanceMode::Kinematic => {
                self.avoidance_search_gen::<ForwardKinematicSampler>(&mut env, backward)
            }
            AvoidanceMode::Rrt => self.avoidance_search_gen::<SpaceSampler>(&mut env, backward),
            AvoidanceMode::RrtStar => {
                self.avoidance_search_gen::<RrtStarSampler>(&mut env, backward)
            }
        }
    }

    /// Templatized logic for searching avoidance path. The type argument speicfy how to
    /// sample a new node.
    pub(super) fn avoidance_search_gen<Sampler: StateSampler>(
        &mut self,
        env: &mut SearchEnv,
        backward: bool,
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

        let searched_path =
            if let Some((mut search_state, goal)) = self.search_state.take().zip(self.goal) {
                if let Some(goal) = search_state.last_solution {
                    if let Some(path) =
                        can_connect_goal(&search_state.start_set, &search_state.search_tree, goal)
                    {
                        // Restore previous solution
                        search_state.found_path = Some(path);
                    }
                }
                if compare_distance(&goal, &search_state.goal, DIST_THRESHOLD) {
                    // for root in &search_state.searchTree {
                    //     enumTree(root, &mut nodes);
                    // }

                    let nodes = &mut search_state.search_tree;

                    // println!(
                    //     "Using existing tree with {} nodes start from {:?}",
                    //     nodes.len(),
                    //     search_state.start
                    // );

                    if 0 < nodes.len() && nodes.len() < 10000 {
                        // Descending the tree is not a good way to sample a random node in a tree, since
                        // the chances are much higher on shallow nodes. We want to give chances uniformly
                        // among all nodes in the tree, so we randomly pick one from a linear list of all nodes.
                        let path = search::<Sampler>(
                            self,
                            &search_state.start_set,
                            env,
                            nodes,
                            &mut search_state.grid_map,
                        );

                        env.tree_size += 1;

                        if let Some(path) = path {
                            // println!("Materialized found path: {:?}", self.path);
                            search_state.last_solution = path.last().copied();
                            search_state.found_path = Some(path);
                            self.search_state = Some(search_state);
                            return true;
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
                let mut nodes: Vec<StateWithCost> = Sampler::initial_search(self, backward);

                if !nodes.is_empty() {
                    let root_set = (0..nodes.len()).collect();
                    let mut grid_map = HashMap::new();
                    for (i, node) in nodes.iter().enumerate() {
                        insert_to_grid_map(&mut grid_map, to_cell(node.state), i);
                    }
                    let found_path =
                        search::<Sampler>(self, &root_set, env, &mut nodes, &mut grid_map);

                    let search_state = SearchState {
                        search_tree: nodes,
                        start_set: root_set,
                        goal: goal,
                        last_solution: None,
                        found_path,
                        grid_map,
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

        // let mut num_detached = 0;
        // Assign infinite cost to those "detached" nodes by the obstacle
        // for (_, state) in visited
        //     .iter()
        //     .zip(ss.search_tree.iter_mut())
        //     .filter(|(visited, _)| !**visited)
        // {
        //     state.cost = 1e8;
        //     num_detached += 1;
        // }

        // println!(
        //     "check_avoidance_collision detached: {num_detached} / {}",
        //     ss.search_tree.len()
        // );

        Some(())
    }
}
