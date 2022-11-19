use rand::{distributions::Uniform, prelude::Distribution};

use super::Agent;
use crate::game::Game;

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
    steer: f64,
    from: Option<usize>,
    to: Vec<usize>,
}

impl StateWithCost {
    pub(crate) fn new(state: State, cost: f64, steer: f64, speed: f64) -> Self {
        Self {
            state,
            cost,
            steer,
            speed,
            id: 0,
            from: None,
            to: vec![],
        }
    }
}

const distRadius: f64 = 0.5;
const distThreshold: f64 = distRadius * distRadius;

/// Wrap the angle value in [0, 2pi)
fn wrapAngle(x: f64) -> f64 {
    use std::f64::consts::PI;
    const TWOPI: f64 = PI * 2.;
    // ((x + PI) - ((x + PI) / TWOPI).floor() * TWOPI) - PI
    x - (x + PI).div_euclid(TWOPI)
}

fn compareState(s1: &State, s2: &State) -> bool {
    let deltaAngle = wrapAngle(s1.heading - s2.heading);
    // println!("compareState deltaAngle: {}", deltaAngle);
    compareDistance(s1, s2, distThreshold) && deltaAngle.abs() < std::f64::consts::PI / 4.
}

fn compareDistance(s1: &State, s2: &State, threshold: f64) -> bool {
    let delta_x = s1.x - s2.x;
    let delta_y = s1.y - s2.y;
    delta_x * delta_x + delta_y * delta_y < threshold
}

const MAX_STEER: f64 = std::f64::consts::PI;

#[derive(Debug)]
pub struct SearchState {
    searchTree: Vec<StateWithCost>,
    treeSize: usize,
    start: State,
    goal: State,
}

impl Agent {
    fn stepMove(px: f64, py: f64, heading: f64, steer: f64, speed: f64, deltaTime: f64) -> State {
        let [x, y] = [speed * deltaTime, 0.];
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
        callback: impl Fn(&StateWithCost, &StateWithCost),
        switchBack: bool,
    ) -> Option<(Vec<StateWithCost>, Vec<usize>)> {
        println!(
            "search invoked: state: {} goal: {:?}",
            self.search_state.is_some(),
            self.goal
        );
        fn interpolate(
            start: &State,
            steer: f64,
            distance: f64,
            f: impl Fn(State) -> bool,
        ) -> bool {
            const INTERPOLATE_INTERVAL: f64 = 10.;
            let interpolates = (distance.abs() / INTERPOLATE_INTERVAL).floor() as usize;
            for i in 0..interpolates {
                let sign = if distance < 0. { -1. } else { 1. };
                let next = Agent::stepMove(
                    start.x,
                    start.y,
                    start.heading,
                    steer,
                    1.,
                    sign * i as f64 * INTERPOLATE_INTERVAL,
                );
                if f(next) {
                    return true;
                }
            }
            return false;
        }

        let mut nodes: Vec<StateWithCost> = vec![];
        // let mut edges: Vec<[StateWithCost; 2]> = vec![];

        fn checkGoal(
            start: usize,
            goal: &Option<State>,
            nodes: &[StateWithCost],
        ) -> Option<Vec<usize>> {
            if let Some(goal) = goal.as_ref() {
                if !compareState(&nodes[start].state, &goal) {
                    return None;
                }
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
            expandStates: usize,
            skipped_nodes: usize,
            tree_size: usize,
        }

        let mut env = SearchEnv {
            game,
            switch_back: switchBack,
            expandStates: 1,
            skipped_nodes: 0,
            tree_size: 0,
        };

        fn search(
            this: &Agent,
            start: usize,
            depth: usize,
            direction: f64,
            env: &mut SearchEnv,
            nodes: &mut Vec<StateWithCost>,
        ) -> Option<Vec<usize>> {
            if depth < 1 || 10000 < nodes.len() {
                return None;
            }
            if let Some(path) = checkGoal(start, &this.goal, &nodes) {
                return Some(path);
            }

            println!(
                "Searching {} states from {start}/{}",
                env.expandStates,
                nodes.len()
            );

            for _i in 0..env.expandStates {
                let State { x, y, heading } = nodes[start].state;
                let steer = rand::random::<f64>() - 0.5;
                let changeDirection = env.switch_back && rand::random::<f64>() < 0.2;
                let nextDirection = if changeDirection {
                    -direction
                } else {
                    direction
                };
                let distance: f64 = distRadius + rand::random::<f64>() * distRadius;
                let next = Agent::stepMove(x, y, heading, steer, 1., nextDirection * distance);
                println!("stepMove: {:?} -> {:?}", nodes[start], next);
                let hit = interpolate(
                    &nodes[start].state,
                    steer,
                    nextDirection * distance,
                    |state| env.game.check_hit([state.x, state.y]),
                );
                if hit {
                    continue;
                }
                // Changing direction costs
                let mut node = StateWithCost::new(
                    next,
                    nodes[start].cost + distance + if changeDirection { 1000. } else { 0. },
                    steer,
                    1.,
                );
                let mut foundNode = None;
                let mut skip = false;
                for i in 0..nodes.len() {
                    if compareState(&nodes[i].state, &node.state) {
                        let mut existingNode = nodes[i].clone();
                        if let Some(existing_from) = existingNode.from {
                            if i != start
                                && existing_from != start
                                && nodes[start].to.iter().any(|j| *j == i)
                            {
                                if existingNode.cost > node.cost {
                                    existingNode.cost = node.cost;
                                    if let Some(&to_index) = existingNode
                                        .from
                                        .and_then(|from| nodes.get(from))
                                        .and_then(|from| from.to.iter().find(|j| **j == i))
                                    {
                                        nodes[existing_from].to.remove(to_index);
                                    } else {
                                        return None;
                                        // throw "Shouldn't happen";
                                    }
                                    existingNode.from = Some(start);
                                    nodes[start].to.push(i);
                                    existingNode.state = node.state;
                                }
                                foundNode = Some(i);
                                break;
                            } else {
                                skip = true;
                            }
                        }
                    }
                }
                if skip {
                    continue;
                }
                if foundNode.is_none() {
                    node.from = Some(start);
                    let new_node_id = nodes.len();
                    nodes[start].to.push(new_node_id);
                    node.id = new_node_id;
                    nodes.push(node);
                    // callback(start, node);
                    search(this, new_node_id, depth - 1, nextDirection, env, nodes);
                } else {
                    env.skipped_nodes += 1;
                }
            }
            None
        }

        // fn enumTree(root: &StateWithCost, nodes: &mut Vec<StateWithCost>) {
        //     nodes.push(root.clone());
        //     for node in &root.to {
        //         enumTree(&nodes[*node], nodes);
        //     }
        // }

        fn traceTree(
            this: &Agent,
            root: usize,
            depth: usize,
            expandDepth: usize,
            env: &mut SearchEnv,
            nodes: &mut Vec<StateWithCost>,
        ) -> Option<Vec<usize>> {
            if depth < 1 {
                return None;
            }
            // if
            /* !root || */
            // checkGoal(root, &this.goal, &nodes).is_some() {
            //     println!("Reached goal! {:?} -> {:?}", nodes[root], this.goal);
            //     return None;
            // }
            let root_node = &nodes[root];
            if env.switch_back || -0.1 < root_node.speed {
                if let Some(path) = search(this, root, expandDepth, 1., env, nodes) {
                    return Some(path);
                }
            }
            let root_node = &nodes[root];
            if env.switch_back || root_node.speed < 0.1 {
                if let Some(path) = search(this, root, expandDepth, -1., env, nodes) {
                    return Some(path);
                }
            }
            let root_node_to = nodes[root].to.clone();
            if 0 < root_node_to.len() {
                for _ in 0..2 {
                    let idx = Uniform::from(0..root_node_to.len()).sample(&mut rand::thread_rng());
                    if let Some(path) =
                        traceTree(this, root_node_to[idx], depth - 1, expandDepth, env, nodes)
                    {
                        return Some(path);
                    }
                }
            }
            env.tree_size += 1;
            None
        }

        let mut path = vec![];

        let searched_path =
            if let Some((mut search_state, goal)) = self.search_state.take().zip(self.goal) {
                if compareDistance(&self.to_state(), &search_state.start, distThreshold * 100.)
                    && compareDistance(&goal, &search_state.goal, distThreshold)
                {
                    // for root in &search_state.searchTree {
                    //     enumTree(root, &mut nodes);
                    // }

                    let nodes = &mut search_state.searchTree;

                    println!("Using existing tree with {} nodes", nodes.len());

                    const SEARCH_NODES: usize = 10;

                    if 0 < nodes.len() && nodes.len() < 10000 {
                        // Descending the tree is not a good way to sample a random node in a tree, since
                        // the chances are much higher on shallow nodes. We want to give chances uniformly
                        // among all nodes in the tree, so we randomly pick one from a linear list of all nodes.
                        for i in 0..SEARCH_NODES {
                            let idx = Uniform::from(0..nodes.len()).sample(&mut rand::thread_rng());
                            traceTree(self, idx, 1, 1, &mut env, nodes);
                        }
                    }

                    // let treeSize = env.tree_size;
                    search_state.treeSize = 0;
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
                println!("Rebuilding tree with {} nodes should be 0", nodes.len());
                let mut roots = vec![];
                if switchBack || true
                /* || -0.1 < self.velocity.magnitude()*/
                {
                    let root = StateWithCost::new(self.to_state(), 0., 0., 1.);
                    let root_id = nodes.len();
                    println!("Pushing the first node: {:?}", root);
                    nodes.push(root.clone());
                    if let Some(found_path) = search(self, root_id, depth, 1., &mut env, &mut nodes)
                    {
                        path = found_path;
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
                    let treeSize = roots.len();
                    let search_state = SearchState {
                        searchTree: roots,
                        start: self.to_state(),
                        treeSize,
                        goal: goal,
                    };
                    // else{
                    //     *search_state = SearchState{
                    //         searchTree: roots,
                    //         treeSize: 0,
                    //         start: State{x: this.x, y: this.y, heading: this.angle},
                    //         goal: this.goal,
                    //     };
                    // }
                    println!("Search state: {search_state:?}");
                    self.search_state = Some(search_state);
                }
            }

            nodes
                .iter_mut()
                .enumerate()
                .for_each(|(index, node)| node.id = index);
            let mut connections: Vec<(usize, usize)> = vec![];
            for (index, node) in nodes.iter().enumerate() {
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
            Some((nodes, path))
        } else {
            None
        }
    }
}

mod render {
    use super::SearchState;
    use druid::{
        kurbo::Circle, piet::kurbo::BezPath, Affine, Color, Env, PaintCtx, Point, RenderContext,
    };

    impl SearchState {
        pub fn render(
            &self,
            ctx: &mut PaintCtx,
            env: &Env,
            view_transform: &Affine,
            brush: &Color,
        ) {
            let mut bez_path = BezPath::new();
            for state in &self.searchTree {
                let point = Point::new(state.state.x, state.state.y);
                if let Some(from) = state.from {
                    let from_state = self.searchTree[from].state;
                    bez_path.move_to(Point::new(from_state.x, from_state.y));
                    bez_path.line_to(point);
                }
                let circle = Circle::new(*view_transform * point, 2.);
                ctx.fill(circle, brush);
            }
            ctx.stroke(*view_transform * bez_path, brush, 0.5);
        }
    }
}
