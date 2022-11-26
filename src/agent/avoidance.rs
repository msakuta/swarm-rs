use std::cell::RefCell;

use cgmath::Vector2;
use rand::{distributions::Uniform, prelude::Distribution};

use super::{
    interpolation::{interpolate, interpolate_steer},
    Agent,
};
use crate::{
    entity::{CollisionShape, Entity},
    game::Game,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct State {
    pub x: f64,
    pub y: f64,
    pub heading: f64,
}

impl State {
    pub fn new(x: f64, y: f64, heading: f64) -> Self {
        Self { x, y, heading }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StateWithCost {
    state: State,
    cost: f64,
    speed: f64,
    id: usize,
    _steer: f64,
    from: Option<usize>,
    to: Vec<usize>,
}

impl StateWithCost {
    pub(crate) fn new(state: State, cost: f64, steer: f64, speed: f64) -> Self {
        Self {
            state,
            cost,
            _steer: steer,
            speed,
            id: 0,
            from: None,
            to: vec![],
        }
    }
}

pub const DIST_RADIUS: f64 = 0.5 * 5.;
const DIST_THRESHOLD: f64 = DIST_RADIUS * DIST_RADIUS;

/// Wrap the angle value in [0, 2pi)
fn wrap_angle(x: f64) -> f64 {
    use std::f64::consts::PI;
    const TWOPI: f64 = PI * 2.;
    // ((x + PI) - ((x + PI) / TWOPI).floor() * TWOPI) - PI
    x - (x + PI).div_euclid(TWOPI)
}

fn compare_state(s1: &State, s2: &State) -> bool {
    let delta_angle = wrap_angle(s1.heading - s2.heading);
    // println!("compareState deltaAngle: {}", deltaAngle);
    compare_distance(s1, s2, DIST_THRESHOLD) && delta_angle.abs() < std::f64::consts::PI / 4.
}

fn compare_distance(s1: &State, s2: &State, threshold: f64) -> bool {
    let delta_x = s1.x - s2.x;
    let delta_y = s1.y - s2.y;
    delta_x * delta_x + delta_y * delta_y < threshold
}

const MAX_STEER: f64 = std::f64::consts::PI / 3.;

#[derive(Debug)]
pub struct SearchState {
    search_tree: Vec<StateWithCost>,
    tree_size: usize,
    start: State,
    goal: State,
    found_path: Option<Vec<usize>>,
}

impl Agent {
    pub(super) fn step_move(
        px: f64,
        py: f64,
        heading: f64,
        steer: f64,
        speed: f64,
        delta_time: f64,
    ) -> State {
        let [x, y] = [speed * delta_time, 0.];
        let heading = heading + steer.min(1.).max(-1.) * x * 0.2 * MAX_STEER;
        let dx = heading.cos() * x - heading.sin() * y + px;
        let dy = heading.sin() * x + heading.cos() * y + py;
        State {
            x: dx,
            y: dy,
            heading,
        }
    }

    pub(super) fn to_state(&self) -> State {
        State {
            x: self.pos[0],
            y: self.pos[1],
            heading: self.orient,
        }
    }

    /// RRT* search
    pub(super) fn search(
        &mut self,
        depth: usize,
        game: &Game,
        entities: &[RefCell<Entity>],
        callback: impl Fn(&StateWithCost, &StateWithCost),
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

        if self
            .search_state
            .as_ref()
            .map(|ss| ss.found_path.is_some())
            .unwrap_or(false)
        {
            return true;
        }

        let mut nodes: Vec<StateWithCost> = vec![];
        // let mut edges: Vec<[StateWithCost; 2]> = vec![];

        /// Check if the goal is close enough to the added node, and if it was, return a built path
        fn check_goal(
            start: usize,
            goal: &Option<State>,
            nodes: &[StateWithCost],
        ) -> Option<Vec<usize>> {
            if let Some(goal) = goal.as_ref() {
                if !compare_distance(&nodes[start].state, &goal, (DIST_RADIUS * 2.).powf(2.)) {
                    return None;
                }
                println!("Found path! {goal:?}");
                let mut node = start;
                let mut path = vec![];
                while let Some(next_node) = nodes[node].from {
                    path.push(next_node);
                    node = next_node;
                }
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

        fn search(
            this: &Agent,
            start: usize,
            direction: f64,
            env: &mut SearchEnv,
            nodes: &mut Vec<StateWithCost>,
        ) -> Option<Vec<usize>> {
            if let Some(path) = check_goal(start, &this.goal, &nodes) {
                return Some(path);
            }

            // println!(
            //     "Searching {} states from {start}/{}",
            //     env.expandStates,
            //     nodes.len()
            // );

            impl From<State> for [f64; 2] {
                fn from(s: State) -> Self {
                    [s.x, s.y]
                }
            }

            impl From<State> for Vector2<f64> {
                fn from(s: State) -> Self {
                    Self::new(s.x, s.y)
                }
            }

            'skip: for _i in 0..env.expand_states {
                let State { x, y, heading } = nodes[start].state;
                let steer = rand::random::<f64>() - 0.5;
                let change_direction = env.switch_back && rand::random::<f64>() < 0.2;
                let next_direction = if change_direction {
                    -direction
                } else {
                    direction
                };
                let distance: f64 = DIST_RADIUS * 2. + rand::random::<f64>() * DIST_RADIUS * 3.;
                let next = Agent::step_move(x, y, heading, steer, 1., next_direction * distance);
                // println!("stepMove: {:?} -> {:?}", nodes[start], next);
                const USE_SEPAX: bool = true;
                const USE_STEER: bool = false;
                let collision_checker = |pos: [f64; 2]| {
                    if Agent::collision_check(Some(this.id), pos, env.entities) {
                        return false;
                    }
                    !env.game.check_hit(pos)
                };
                let hit = if USE_SEPAX {
                    env.entities
                        .iter()
                        .filter_map(|entity| entity.try_borrow().ok())
                        .map(|entity| entity.get_shape())
                        .any(|shape| {
                            let CollisionShape::BBox(bbox) = shape;
                            let pos = nodes[start].state.into();
                            let diff = Vector2::from(next) - pos;
                            let hit = crate::game::separating_axis(
                                &pos,
                                &diff,
                                bbox.into_iter().map(Vector2::from),
                            );
                            if hit {
                                println!("Entity hit");
                            }
                            hit
                        })
                        || interpolate(nodes[start].state, next, DIST_RADIUS * 0.5, |pos| {
                            !env.game.check_hit(pos)
                        })
                } else if USE_STEER {
                    interpolate_steer(
                        &nodes[start].state,
                        steer,
                        next_direction * distance,
                        DIST_RADIUS,
                        &collision_checker,
                    )
                } else {
                    interpolate(nodes[start].state, next, DIST_RADIUS, &collision_checker)
                };
                if hit {
                    // println!("Search hit something!, {nextDirection} * {distance}");
                    continue;
                }
                // Changing direction costs
                let mut node = StateWithCost::new(
                    next,
                    nodes[start].cost + distance + if change_direction { 1000. } else { 0. },
                    steer,
                    1.,
                );
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
                        continue 'skip;
                    }
                    let Some((to_index, _)) = nodes[existing_from].to
                        .iter().copied().enumerate().find(|(_, j)| *j == i) else
                    {
                        continue
                    };
                    if existing_cost > node.cost {
                        nodes[i].cost = node.cost;
                        nodes[existing_from].to.remove(to_index);
                        nodes[i].from = Some(start);
                        nodes[start].to.push(i);
                        nodes[i].state = node.state;
                    }
                    env.skipped_nodes += 1;
                    continue 'skip;
                }

                node.from = Some(start);
                let new_node_id = nodes.len();
                nodes[start].to.push(new_node_id);
                node.id = new_node_id;
                nodes.push(node);
                // callback(start, node);
            }
            None
        }

        // fn enumTree(root: &StateWithCost, nodes: &mut Vec<StateWithCost>) {
        //     nodes.push(root.clone());
        //     for node in &root.to {
        //         enumTree(&nodes[*node], nodes);
        //     }
        // }

        fn trace_tree(
            this: &Agent,
            root: usize,
            env: &mut SearchEnv,
            nodes: &mut Vec<StateWithCost>,
        ) -> Option<Vec<usize>> {
            let root_node = &nodes[root];
            if env.switch_back || -0.1 < root_node.speed {
                if let Some(path) = search(this, root, 1., env, nodes) {
                    return Some(path);
                }
            }
            let root_node = &nodes[root];
            if env.switch_back || root_node.speed < 0.1 {
                if let Some(path) = search(this, root, -1., env, nodes) {
                    return Some(path);
                }
            }
            env.tree_size += 1;
            None
        }

        let searched_path =
            if let Some((mut search_state, goal)) = self.search_state.take().zip(self.goal) {
                if compare_distance(&self.to_state(), &search_state.start, DIST_THRESHOLD * 100.)
                    && compare_distance(&goal, &search_state.goal, DIST_THRESHOLD)
                {
                    // for root in &search_state.searchTree {
                    //     enumTree(root, &mut nodes);
                    // }

                    let nodes = &mut search_state.search_tree;

                    // println!("Using existing tree with {} nodes", nodes.len());

                    const SEARCH_NODES: usize = 20;

                    if 0 < nodes.len() && nodes.len() < 10000 {
                        // Descending the tree is not a good way to sample a random node in a tree, since
                        // the chances are much higher on shallow nodes. We want to give chances uniformly
                        // among all nodes in the tree, so we randomly pick one from a linear list of all nodes.
                        for _i in 0..SEARCH_NODES {
                            let idx = Uniform::from(0..nodes.len()).sample(&mut rand::thread_rng());
                            if let Some(path) = trace_tree(self, idx, &mut env, nodes) {
                                self.avoidance_path = path
                                    .iter()
                                    .map(|i| {
                                        let node = nodes[*i].state;
                                        [node.x, node.y]
                                    })
                                    .collect();
                                // println!("Materialized found path: {:?}", self.path);
                                search_state.found_path = Some(path);
                                self.search_state = None; //Some(search_state);
                                return true;
                            }
                        }
                    }

                    // let treeSize = env.tree_size;
                    search_state.tree_size = 0;
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
                let mut roots = vec![];
                if switch_back || true
                /* || -0.1 < self.velocity.magnitude()*/
                {
                    let root = StateWithCost::new(self.to_state(), 0., 0., 1.);
                    let root_id = nodes.len();
                    // println!("Pushing the first node: {:?}", root);
                    nodes.push(root.clone());
                    if let Some(path) = search(self, root_id, 1., &mut env, &mut nodes) {
                        self.avoidance_path = path
                            .iter()
                            .map(|i| {
                                let node = nodes[*i].state;
                                [node.x, node.y]
                            })
                            .collect();
                        // let tree_size = nodes.len();
                        self.search_state = None;
                        //  Some(SearchState {
                        //     searchTree: nodes,
                        //     start: root.state,
                        //     treeSize: tree_size,
                        //     goal,
                        //     found_path: Some(path),
                        // });
                        return true;
                    }
                    roots.push(root);
                }
                // if(switchBack || this.speed < 0.1){
                //     let root = StateWithCost(State{x: this.x, y: this.y, heading: this.angle}, 0, 0, -1);
                //     nodes.push(root);
                //     search(root, depth, -1, 1);
                //     roots.push(root);
                // }
                if !roots.is_empty() {
                    let tree_size = roots.len();
                    let search_state = SearchState {
                        search_tree: roots,
                        start: self.to_state(),
                        tree_size,
                        goal: goal,
                        found_path: None,
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

            nodes
                .iter_mut()
                .enumerate()
                .for_each(|(index, node)| node.id = index);
            let mut connections: Vec<(usize, usize)> = vec![];
            for node in nodes.iter() {
                if let Some(from) = node.from {
                    callback(&nodes[from], node);
                    if !(node.id < nodes.len()) {
                        panic!("No node id for to: {}", node.id);
                    }
                    connections.push((from, node.id));
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

            // let nodeBuffer = new Float32Array(nodes.length * 5);
            // nodes.forEach((node, i) => {
            //     nodeBuffer[i * 5] = node.x;
            //     nodeBuffer[i * 5 + 1] = node.y;
            //     nodeBuffer[i * 5 + 2] = node.heading;
            //     nodeBuffer[i * 5 + 3] = node.cost;
            //     nodeBuffer[i * 5 + 4] = node.speed;
            // });

            // validate
            for con in connections {
                if !(con.0 < nodes.len()) || !(con.1 < nodes.len()) {
                    panic!("No node id for from: {:?}", con);
                }
            }
            // for node in &self.path {
            //     if(!(node.id in nodes)) throw `Path node not in nodes ${node.id}`;
            // }
        }
        false
    }
}

mod render {
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
            brush: &Color,
            circle_visible: bool,
        ) {
            let mut bez_path = BezPath::new();
            for state in &self.search_tree {
                let point = Point::new(state.state.x, state.state.y);
                if let Some(from) = state.from {
                    let from_state = self.search_tree[from].state;
                    bez_path.move_to(Point::new(from_state.x, from_state.y));
                    bez_path.line_to(point);
                }
                if circle_visible {
                    let circle = Circle::new(*view_transform * point, 2.);
                    ctx.fill(circle, brush);
                    let circle = Circle::new(point, DIST_RADIUS);
                    ctx.stroke(*view_transform * circle, brush, 0.5);
                }
            }
            ctx.stroke(*view_transform * bez_path, brush, 0.5);
        }
    }
}
