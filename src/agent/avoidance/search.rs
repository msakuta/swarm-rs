use std::collections::HashSet;

use cgmath::{MetricSpace, Vector2, Zero};

use crate::{
    agent::{
        avoidance::DIST_RADIUS,
        interpolation::{interpolate, interpolate_steer, AsPoint},
        wrap_angle, Agent,
    },
    collision::bsearch_collision,
};

use super::{
    compare_distance, sampler::StateSampler, AgentState, SearchEnv, StateWithCost,
};

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

pub(super) fn search<S: StateSampler>(
    this: &Agent,
    start_set: &HashSet<usize>,
    env: &mut SearchEnv,
    nodes: &mut Vec<StateWithCost>,
) -> Option<Vec<usize>> {
    'skip: for _i in 0..env.expand_states {
        let mut sampler = S::new(env);
        let (start, mut node) = sampler.sample(nodes, env)?;
        let next_direction = node.speed.signum();
        let start_state = nodes[start].state;

        // println!(
        //     "Searching {} states from {start}/{}",
        //     env.expandStates,
        //     nodes.len()
        // );

        // let start_state = nodes[start].state;
        let this_shape = this.get_shape();

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
                    interpolate(start_state, next, DIST_RADIUS, &collision_checker),
                    0,
                )
            }
        };

        // let AgentState { x, y, heading } = start_state;

        // First, check if there is already a "samey" node exists
        for i in 0..nodes.len() {
            if !S::compare_state(&nodes[i].state, &node.state) {
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
            let distance = Vector2::from(nodes[i].state).distance(Vector2::from(start_state));
            let shortcut_cost = sampler.calculate_cost(distance);

            // If this is a "shortcut", i.e. has a lower cost than existing node, "graft" the branch
            if existing_cost > shortcut_cost {
                let delta = Vector2::from(nodes[i].state) - Vector2::from(start_state);
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

        if let Some(path) = check_goal(start_set, new_node_id, &this.goal, &nodes) {
            return Some(path);
        }

        // callback(start, node);
    }
    None
}
