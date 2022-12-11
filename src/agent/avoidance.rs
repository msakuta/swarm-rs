use std::{cell::RefCell, collections::HashSet};

use cgmath::{MetricSpace, Vector2, Zero};
use rand::{distributions::Uniform, prelude::Distribution};

use super::{
    interpolation::{interpolate, interpolate_steer},
    wrap_angle, Agent, GameEnv, AGENT_HALFLENGTH, AGENT_HALFWIDTH,
};
use crate::{
    agent::interpolation::AsPoint,
    collision::{bsearch_collision, CollisionShape, Obb},
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
            heading,
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

        /// Check if the goal is close enough to the added node, and if it was, return a built path
        fn check_goal(
            start_set: &HashSet<usize>,
            start: usize,
            goal: &Option<AgentState>,
            nodes: &[StateWithCost],
        ) -> Option<Vec<usize>> {
            if let Some(goal) = goal.as_ref() {
                if !compare_distance(&nodes[start].state, &goal, (DIST_RADIUS * 2.).powf(2.)) {
                    return None;
                }
                let mut node = start;
                let mut path = vec![];
                while let Some(next_node) = nodes[node].from {
                    if !nodes[next_node].is_passable() {
                        return None;
                    }
                    path.push(next_node);
                    if start_set.contains(&next_node) {
                        break;
                    }
                    node = next_node;
                }
                println!(
                    "Found path to {goal:?}: {:?}",
                    path.iter()
                        .map(|node| nodes[*node].speed)
                        .collect::<Vec<_>>()
                );
                return Some(path);
            }
            None
        }

        struct SearchEnv<'a> {
            game: &'a Game,
            switch_back: bool,
            expand_states: usize,
            skipped_nodes: usize,
            tree_size: usize,
            entities: &'a [RefCell<Entity>],
        }

        let mut env = SearchEnv {
            game,
            switch_back,
            expand_states: 1,
            skipped_nodes: 0,
            tree_size: 0,
            entities,
        };

        trait StateSampler {
            fn new(env: &SearchEnv) -> Self;
            fn sample(
                &mut self,
                start_node: &StateWithCost,
                direction: f64,
                env: &SearchEnv,
            ) -> StateWithCost;
            fn calculate_cost(&self, distance: f64) -> f64;
            fn next_direction(&self, direction: f64) -> f64;
        }

        /// Control space sampler.
        struct ForwardKinematicSampler {
            change_direction: bool,
            start_cost: Option<f64>,
        }

        impl StateSampler for ForwardKinematicSampler {
            fn new(env: &SearchEnv) -> Self {
                Self {
                    change_direction: env.switch_back && rand::random::<f64>() < 0.2,
                    start_cost: None,
                }
            }

            fn sample(
                &mut self,
                start_node: &StateWithCost,
                direction: f64,
                _env: &SearchEnv,
            ) -> StateWithCost {
                let steer = rand::random::<f64>() - 0.5;
                let next_direction = if self.change_direction {
                    -direction
                } else {
                    direction
                };
                let distance: f64 = DIST_RADIUS * 2. + rand::random::<f64>() * DIST_RADIUS * 3.;
                let AgentState { x, y, heading } = start_node.state;
                let next = Agent::step_move(x, y, heading, steer, next_direction * distance);

                self.start_cost = Some(start_node.cost);

                StateWithCost::new(next, self.calculate_cost(distance), steer, next_direction)
            }

            /// Changing direction costs
            fn calculate_cost(&self, distance: f64) -> f64 {
                self.start_cost.unwrap()
                    + distance
                    + if self.change_direction { 10000. } else { 0. }
            }

            fn next_direction(&self, direction: f64) -> f64 {
                if self.change_direction {
                    -direction
                } else {
                    direction
                }
            }
        }

        struct SpaceSampler(f64);

        impl StateSampler for SpaceSampler {
            fn new(_env: &SearchEnv) -> Self {
                Self(0.)
            }

            fn sample(
                &mut self,
                start_node: &StateWithCost,
                direction: f64,
                env: &SearchEnv,
            ) -> StateWithCost {
                let distance: f64 = DIST_RADIUS * 2. + rand::random::<f64>() * DIST_RADIUS * 3.;
                self.0 = start_node.cost;
                let state = AgentState {
                    x: rand::random::<f64>() * env.game.xs as f64,
                    y: rand::random::<f64>() * env.game.ys as f64,
                    heading: start_node.state.heading,
                };
                StateWithCost::new(state, self.calculate_cost(distance), 0., direction)
            }

            fn calculate_cost(&self, distance: f64) -> f64 {
                self.0 + distance
            }

            /// Always the same direction
            fn next_direction(&self, direction: f64) -> f64 {
                direction
            }
        }

        type Sampler = SpaceSampler;

        fn search<S: StateSampler>(
            this: &Agent,
            start: usize,
            start_set: &HashSet<usize>,
            direction: f64,
            env: &mut SearchEnv,
            nodes: &mut Vec<StateWithCost>,
        ) -> Option<Vec<usize>> {
            if let Some(path) = check_goal(start_set, start, &this.goal, &nodes) {
                return Some(path);
            }

            // println!(
            //     "Searching {} states from {start}/{}",
            //     env.expandStates,
            //     nodes.len()
            // );

            let start_state = nodes[start].state;
            let this_shape = start_state.collision_shape();

            let collision_check = |next: AgentState,
                                   next_direction: f64,
                                   distance: f64,
                                   heading: f64,
                                   steer: f64|
             -> (bool, usize) {
                const USE_SEPAX: bool = true;
                const USE_STEER: bool = false;
                let collision_checker = |state: AgentState| {
                    if Agent::collision_check(Some(this.id), state, env.entities, true) {
                        return false;
                    }
                    !env.game.check_hit(
                        &start_state
                            .collision_shape()
                            .with_position(state.as_point().into()),
                    )
                };
                if USE_SEPAX {
                    let (hit, level) = env
                        .entities
                        .iter()
                        .filter_map(|entity| entity.try_borrow().ok())
                        .fold((false, 0usize), |acc, entity| {
                            let shape = entity.get_shape();
                            let pos = Vector2::from(start_state);
                            let diff = Vector2::from(next) - pos;
                            let (hit, level) =
                                bsearch_collision(&this_shape, &diff, &shape, &Vector2::zero());
                            (acc.0 || hit, acc.1.max(level))
                        });

                    if hit {
                        (hit, level)
                    } else {
                        (
                            interpolate(start_state, next, DIST_RADIUS * 0.5, |pos| {
                                !env.game.check_hit(
                                    &start_state.collision_shape().with_position(pos.into()),
                                )
                            }),
                            level,
                        )
                    }
                } else if USE_STEER {
                    (
                        interpolate_steer(
                            &start_state,
                            steer,
                            next_direction * distance,
                            DIST_RADIUS,
                            &collision_checker,
                        ),
                        0,
                    )
                } else {
                    (
                        interpolate(start_state, next, DIST_RADIUS, &collision_checker),
                        0,
                    )
                }
            };

            'skip: for _i in 0..env.expand_states {
                // let AgentState { x, y, heading } = start_state;
                let mut sampler = S::new(env);
                let mut node = sampler.sample(&nodes[start], direction, env);
                let mut next_direction = sampler.next_direction(direction);

                // First, check if there is already a "samey" node exists
                for i in 0..nodes.len() {
                    if !compare_state(&nodes[i].state, &node.state) {
                        continue;
                    }
                    let existing_node = &nodes[i];
                    let Some(existing_from) = existing_node.from else {
                        continue;
                    };
                    let existing_cost = existing_node.cost;
                    if i == start || existing_from == start {
                        nodes[i].blocked = false;
                        if nodes[i].blocked {
                            println!("Reviving blocked node {i}");
                        }
                        continue 'skip;
                    }
                    let Some((to_index, _)) = nodes[existing_from].to
                        .iter().copied().enumerate().find(|(_, j)| *j == i) else
                    {
                        continue
                    };
                    let distance =
                        Vector2::from(nodes[i].state).distance(Vector2::from(start_state));
                    let shortcut_cost = sampler.calculate_cost(distance);

                    // If this is a "shortcut", i.e. has a lower cost than existing node, "graft" the branch
                    if existing_cost > shortcut_cost {
                        let delta = Vector2::from(node.state) - Vector2::from(start_state);
                        let heading = delta.y.atan2(delta.x);
                        let heading = if next_direction < 0. {
                            wrap_angle(heading + std::f64::consts::PI)
                        } else {
                            heading
                        };
                        let (hit, _level) = collision_check(
                            nodes[i].state,
                            next_direction,
                            distance,
                            heading,
                            node.steer,
                        );
                        if hit {
                            continue 'skip;
                        }
                        nodes[i].state.heading = heading;
                        nodes[i].cost = shortcut_cost;
                        nodes[existing_from].to.remove(to_index);
                        nodes[i].from = Some(start);
                        if nodes[i].blocked {
                            println!("Reviving blocked node {i}");
                        }
                        nodes[i].blocked = false;
                        nodes[start].to.push(i);
                        // nodes[i].state = node.state;
                    }
                    env.skipped_nodes += 1;
                    continue 'skip;
                }
                // println!("stepMove: {:?} -> {:?}", nodes[start], next);

                let distance = Vector2::from(node.state).distance(start_state.into());

                let (hit, level) = collision_check(
                    node.state,
                    next_direction,
                    distance,
                    node.state.heading,
                    node.steer,
                );

                if hit {
                    // println!("Search hit something!, {nextDirection} * {distance}");
                    continue;
                }

                node.from = Some(start);
                let new_node_id = nodes.len();
                nodes[start].to.push(new_node_id);
                node.id = new_node_id;
                node.max_level = level;
                nodes.push(node);
                // callback(start, node);
            }
            None
        }

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

                    const SEARCH_NODES: usize = 100;

                    if 0 < nodes.len() && nodes.len() < 10000 {
                        // Descending the tree is not a good way to sample a random node in a tree, since
                        // the chances are much higher on shallow nodes. We want to give chances uniformly
                        // among all nodes in the tree, so we randomly pick one from a linear list of all nodes.
                        'roll_dice: for _i in 0..SEARCH_NODES {
                            let path = 'trace_tree: {
                                let root =
                                    Uniform::from(0..nodes.len()).sample(&mut rand::thread_rng());
                                let root_node = &nodes[root];
                                if !root_node.is_passable() {
                                    continue 'roll_dice;
                                }
                                if env.switch_back || 0. < root_node.speed {
                                    if let Some(path) = search::<Sampler>(
                                        self,
                                        root,
                                        &search_state.start_set,
                                        1.,
                                        &mut env,
                                        nodes,
                                    ) {
                                        break 'trace_tree Some(path);
                                    }
                                }
                                let root_node = &nodes[root];
                                if env.switch_back || root_node.speed < 0. {
                                    if let Some(path) = search::<Sampler>(
                                        self,
                                        root,
                                        &search_state.start_set,
                                        -1.,
                                        &mut env,
                                        nodes,
                                    ) {
                                        break 'trace_tree Some(path);
                                    }
                                }
                                env.tree_size += 1;
                                None
                            };

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
                let found_path = 'find_path: {
                    if backward || -0.1 < self.speed {
                        let root = StateWithCost::new(self.to_state(), 0., 0., 1.);
                        let root_id = nodes.len();
                        nodes.push(root.clone());
                        root_set.insert(root_id);
                        if let Some(path) =
                            search::<Sampler>(self, root_id, &root_set, 1., &mut env, &mut nodes)
                        {
                            break 'find_path Some(path);
                        }
                    };
                    if backward || self.speed < 0.1 {
                        let root = StateWithCost::new(self.to_state(), 0., 0., -1.);
                        let root_id = nodes.len();
                        nodes.push(root.clone());
                        root_set.insert(root_id);
                        if let Some(path) =
                            search::<Sampler>(self, root_id, &root_set, -1., &mut env, &mut nodes)
                        {
                            break 'find_path Some(path);
                        }
                    }
                    None
                };
                if !nodes.is_empty() {
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
