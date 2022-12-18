use std::collections::HashSet;

use cgmath::{MetricSpace, Vector2, Zero};

use crate::{
    agent::{
        avoidance::{DIST_RADIUS, MAX_CELL_COUNT},
        interpolation::{interpolate, interpolate_steer, AsPoint},
        Agent,
    },
    collision::bsearch_collision,
};

use super::{
    compare_distance, sampler::StateSampler, AgentState, GridMap, SearchEnv, StateWithCost,
    CELL_SIZE,
};

pub(super) fn to_cell(state: AgentState) -> [i32; 2] {
    [
        state.x.div_euclid(CELL_SIZE) as i32,
        state.y.div_euclid(CELL_SIZE) as i32,
    ]
}

pub(super) fn insert_to_grid_map(grid_map: &mut GridMap, idx: [i32; 2], value: usize) {
    let cell = grid_map.entry(idx).or_insert_with(|| HashSet::new());
    cell.insert(value);
}

pub(super) fn count_from_grid_map(grid_map: &mut GridMap, idx: [i32; 2]) -> usize {
    grid_map.get(&idx).map(|cell| cell.len()).unwrap_or(0)
}

pub(super) fn can_connect_goal(
    start_set: &HashSet<usize>,
    nodes: &[StateWithCost],
    mut node: usize,
) -> Option<Vec<usize>> {
    let mut path = vec![];
    while let Some(next_node) = nodes[node].from {
        if !nodes[next_node].is_passable() {
            return None;
        }
        path.push(next_node);
        if start_set.contains(&next_node) {
            return Some(path);
        }
        node = next_node;
    }
    None
}

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

/// Perform the search using sampling strategy given by `S`.
/// Returns path node ids or None if the path is not found yet.
pub(super) fn search<S: StateSampler>(
    this: &Agent,
    start_set: &HashSet<usize>,
    env: &mut SearchEnv,
    nodes: &mut Vec<StateWithCost>,
    grid_map: &mut GridMap,
) -> Option<Vec<usize>> {
    'skip: for _i in 0..env.expand_states {
        let mut sampler = S::new(env);

        // println!(
        //     "Searching {} states from {start}/{}",
        //     env.expandStates,
        //     nodes.len()
        // );

        // let start_state = nodes[start].state;
        // let this_shape = this.get_shape();

        let collision_check = |start_state: AgentState,
                               next_state: AgentState,
                               next_direction: f64,
                               distance: f64,
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
                let start_shape = start_state.collision_shape();
                let (hit, level) = env
                    .entities
                    .iter()
                    .filter_map(|entity| entity.try_borrow().ok())
                    .fold((false, 0usize), |acc, entity| {
                        let shape = entity.get_shape();
                        let pos = Vector2::from(start_state);
                        let diff = Vector2::from(next_state) - pos;
                        let (hit, level) =
                            bsearch_collision(&start_shape, &diff, &shape, &Vector2::zero());
                        (acc.0 || hit, acc.1.max(level))
                    });

                if hit {
                    (hit, level)
                } else {
                    (
                        interpolate(start_state, next_state, DIST_RADIUS * 0.5, |pos| {
                            !env.game
                                .check_hit(&start_state.collision_shape().with_position(pos.into()))
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
                    interpolate(start_state, next_state, DIST_RADIUS, &collision_checker),
                    0,
                )
            }
        };

        let found = 'found: {
            for _ in 0..100 {
                let (start, node) = sampler.sample(nodes, env, grid_map, collision_check)?;
                let cell_count = count_from_grid_map(grid_map, to_cell(node.state));
                if MAX_CELL_COUNT < cell_count {
                    continue;
                }
                break 'found Some((start, node));
            }
            None
        };

        let Some((start, mut node)) = found else {
            continue;
        };

        let next_direction = node.speed.signum();
        let start_state = nodes[start].state;

        // let AgentState { x, y, heading } = start_state;
        if sampler.merge_same_nodes(&node, start, nodes, env) {
            continue 'skip;
        }
        // println!("stepMove: {:?} -> {:?}", nodes[start], next);

        let distance = Vector2::from(node.state).distance(start_state.into());

        let (hit, level) = collision_check(
            start_state,
            node.state,
            next_direction,
            distance,
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

        insert_to_grid_map(grid_map, to_cell(node.state), new_node_id);

        let mut hist = vec![];
        for cell in grid_map.values() {
            let bin = cell.len();
            if hist.len() <= bin {
                hist.resize(bin + 1, 0);
            }
            hist[bin] += 1;
        }

        nodes.push(node);

        sampler.rewire(nodes, new_node_id, start, collision_check);

        if let Some(path) = check_goal(start_set, new_node_id, &this.goal, &nodes) {
            return Some(path);
        }

        // callback(start, node);
    }
    None
}
