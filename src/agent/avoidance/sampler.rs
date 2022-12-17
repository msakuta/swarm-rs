use std::sync::atomic::{AtomicUsize, Ordering};

use cgmath::{InnerSpace, MetricSpace, Vector2};
use rand::{distributions::Uniform, prelude::Distribution};

use crate::agent::{wrap_angle, Agent};

use super::{
    compare_distance, AgentState, GridMap, SearchEnv, StateWithCost, CELL_SIZE, DIST_RADIUS,
    DIST_THRESHOLD,
};

pub(in super::super) trait StateSampler {
    fn new(env: &SearchEnv) -> Self;
    fn compare_state(s1: &AgentState, s2: &AgentState) -> bool;
    fn initial_search(agent: &Agent, backward: bool) -> Vec<StateWithCost>;

    /// Sample a new state. Shall return an index to the starting node and the new state as a tuple.
    fn sample(
        &mut self,
        nodes: &[StateWithCost],
        env: &SearchEnv,
        grid_map: &GridMap,
        collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) -> Option<(usize, StateWithCost)>;

    /// Attempt to merge a new candidate node to one of the existing ones and returns true if successful.
    ///
    /// It is only really useful with ForwardKinematicSampler.
    #[allow(unused_variables)]
    fn merge_same_nodes(
        &self,
        node: &StateWithCost,
        start: usize,
        nodes: &mut [StateWithCost],
        env: &mut SearchEnv,
    ) -> bool {
        false
    }

    /// Rewire the tree edges to optimize the search. The default does nothing.
    #[allow(unused_variables)]
    fn rewire(
        &self,
        nodes: &mut [StateWithCost],
        new_node: usize,
        start: usize,
        collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) {
    }
}

/// Control space sampler. It is very cheap and can explore feasible path in kinematic model,
/// but it suffers from very slow space coverage rate.
pub(super) struct ForwardKinematicSampler {
    change_direction: bool,
    start_cost: Option<f64>,
}

impl ForwardKinematicSampler {
    /// Changing direction costs
    fn calculate_cost(&self, distance: f64) -> f64 {
        self.start_cost.unwrap() + distance + if self.change_direction { 10000. } else { 0. }
    }
}

impl StateSampler for ForwardKinematicSampler {
    fn new(_env: &SearchEnv) -> Self {
        Self {
            change_direction: false,
            start_cost: None,
        }
    }

    fn compare_state(s1: &AgentState, s2: &AgentState) -> bool {
        let delta_angle = wrap_angle(s1.heading - s2.heading);
        // println!("compareState deltaAngle: {}", deltaAngle);
        compare_distance(s1, s2, DIST_THRESHOLD) && delta_angle.abs() < std::f64::consts::PI / 6.
    }

    fn initial_search(agent: &Agent, backward: bool) -> Vec<StateWithCost> {
        let mut nodes = vec![];
        if backward || -0.1 < agent.speed {
            let root = StateWithCost::new(agent.to_state(), 0., 0., 1.);
            nodes.push(root.clone());
        };
        if backward || agent.speed < 0.1 {
            let root = StateWithCost::new(agent.to_state(), 0., 0., -1.);
            nodes.push(root.clone());
        }
        nodes
    }

    fn sample(
        &mut self,
        nodes: &[StateWithCost],
        env: &SearchEnv,
        _grid_map: &GridMap,
        _collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) -> Option<(usize, StateWithCost)> {
        let (start, start_node) = {
            let total_passables = nodes.iter().filter(|node| node.is_passable()).count();
            if total_passables == 0 {
                return None;
            }
            let candidate = Uniform::from(0..total_passables).sample(&mut rand::thread_rng());
            nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.is_passable())
                .nth(candidate)?
        };

        let direction = start_node.speed.signum();

        self.change_direction = env.switch_back && rand::random::<f64>() < 0.2;

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

        Some((
            start,
            StateWithCost::new(next, self.calculate_cost(distance), steer, next_direction),
        ))
    }

    fn merge_same_nodes(
        &self,
        node: &StateWithCost,
        start: usize,
        nodes: &mut [StateWithCost],
        env: &mut SearchEnv,
    ) -> bool {
        // Check if there is already a "samey" node exists
        for i in 0..nodes.len() {
            if !Self::compare_state(&nodes[i].state, &node.state) {
                continue;
            }
            let existing_node = &nodes[i];
            let Some(existing_from) = existing_node.from else {
                continue;
            };
            if i == start || existing_from == start {
                nodes[i].blocked = false;
                if nodes[i].blocked {
                    println!("Reviving blocked node {i}");
                }
                return true;
            }
            env.skipped_nodes += 1;
            return true;
        }
        false
    }

    fn rewire(
        &self,
        nodes: &mut [StateWithCost],
        new_node_id: usize,
        start: usize,
        mut collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) {
        let node = &nodes[new_node_id];
        let node_state = node.state;
        let next_direction = node.speed.signum();
        let node_steer = node.steer;
        let start_state = node.state;
        for i in 0..nodes.len() {
            if i == new_node_id {
                continue;
            }
            if !Self::compare_state(&nodes[i].state, &node_state) {
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
                return;
            }
            let Some((to_index, _)) = nodes[existing_from].to
                .iter().copied().enumerate().find(|(_, j)| *j == i) else
            {
                continue
            };
            let distance = Vector2::from(nodes[i].state).distance(Vector2::from(start_state));
            let shortcut_cost = self.calculate_cost(distance);
            // If this is a "shortcut", i.e. has a lower cost than existing node, "graft" the branch
            if existing_cost > shortcut_cost {
                let delta = Vector2::from(nodes[i].state) - Vector2::from(start_state);
                let (hit, _level) = collision_check(
                    start_state,
                    nodes[i].state,
                    next_direction,
                    distance,
                    node_steer,
                );
                if hit {
                    return;
                }
                nodes[i].state.heading = heading_from_delta(&delta, next_direction);
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
        }
    }
}

fn heading_from_delta(delta: &Vector2<f64>, next_direction: f64) -> f64 {
    let heading = delta.y.atan2(delta.x);
    if next_direction < 0. {
        wrap_angle(heading + std::f64::consts::PI)
    } else {
        heading
    }
}

/// Spatially random sampler. It is closer to pure RRT, which guarantees asymptotically filling space coverage
pub(super) struct SpaceSampler(f64);

impl SpaceSampler {
    fn calculate_cost(&self, distance: f64) -> f64 {
        self.0 + distance
    }
}

const STEER_DISTANCE: f64 = DIST_RADIUS * 2.5;

impl StateSampler for SpaceSampler {
    fn new(_env: &SearchEnv) -> Self {
        Self(0.)
    }

    fn compare_state(s1: &AgentState, s2: &AgentState) -> bool {
        compare_distance(s1, s2, DIST_THRESHOLD)
    }

    fn initial_search(agent: &Agent, _backward: bool) -> Vec<StateWithCost> {
        let root = StateWithCost::new(agent.to_state(), 0., 0., 1.);
        vec![root]
    }

    fn sample(
        &mut self,
        nodes: &[StateWithCost],
        env: &SearchEnv,
        grid_map: &GridMap,
        _collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) -> Option<(usize, StateWithCost)> {
        let position = Vector2::new(
            rand::random::<f64>() * env.game.xs as f64,
            rand::random::<f64>() * env.game.ys as f64,
        );

        let (i, closest_node) = find_closest_node(nodes, position, grid_map)?;

        self.0 = closest_node.cost;
        let closest_point = Vector2::from(closest_node.state);
        let distance = closest_point.distance(position).min(STEER_DISTANCE);
        let steer = (position - closest_point).normalize() * distance;
        let position = closest_point + steer;

        let state = AgentState::new(position.x, position.y, closest_node.state.heading);
        let direction = closest_node.speed.signum();

        Some((
            i,
            StateWithCost::new(state, self.calculate_cost(distance), 0., direction),
        ))
    }
}

/// Use grid map to quickly find the closest node
fn find_closest_node<'a, 'b>(
    nodes: &'a [StateWithCost],
    position: Vector2<f64>,
    grid_map: &'b GridMap,
) -> Option<(usize, &'a StateWithCost)> {
    let center = (
        (position.x / CELL_SIZE) as i32,
        (position.y / CELL_SIZE) as i32,
    );
    const MAX_SEARCH_CELL_RADIUS: i32 = 10;
    // Gradually expand cells to scan
    for cell_radius in 0..MAX_SEARCH_CELL_RADIUS {
        let mut closest = None;
        for iy in -cell_radius..=cell_radius {
            for ix in -cell_radius..=cell_radius {
                let Some(cell_nodes) = grid_map.get(&[center.0 + ix, center.1 + iy]) else { continue };
                closest =
                    cell_nodes
                        .iter()
                        .fold(closest, |acc: Option<(usize, &StateWithCost)>, &ib| {
                            let b = &nodes[ib];
                            if let Some((ia, a)) = acc {
                                let distance_a = Vector2::from(a.state).distance(position);
                                let distance_b = Vector2::from(b.state).distance(position);
                                if distance_a < distance_b {
                                    Some((ia, a))
                                } else {
                                    Some((ib, b))
                                }
                            } else {
                                Some((ib, b))
                            }
                        });
            }
        }
        if let Some((i, closest_node)) = closest {
            return Some((i, closest_node));
        }
    }
    None
}

/// Rewiring distance has to be long enough to make RRT* effective
pub(super) const REWIRE_DISTANCE: f64 = DIST_RADIUS * 5.;

/// RRT* sampler, awkward capitalization in Rust convention
pub(super) struct RrtStarSampler;

impl StateSampler for RrtStarSampler {
    fn new(_env: &SearchEnv) -> Self {
        Self
    }

    fn compare_state(s1: &AgentState, s2: &AgentState) -> bool {
        compare_distance(s1, s2, DIST_THRESHOLD)
    }

    fn initial_search(agent: &Agent, _backward: bool) -> Vec<StateWithCost> {
        let root = StateWithCost::new(agent.to_state(), 0., 0., 1.);
        vec![root]
    }

    fn sample(
        &mut self,
        nodes: &[StateWithCost],
        env: &SearchEnv,
        grid_map: &GridMap,
        mut collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) -> Option<(usize, StateWithCost)> {
        let position = Vector2::new(
            rand::random::<f64>() * env.game.xs as f64,
            rand::random::<f64>() * env.game.ys as f64,
        );

        let (closest_id, closest_node) = find_closest_node(nodes, position, grid_map)?;

        let closest_point = Vector2::from(nodes[closest_id].state);
        let distance = closest_point.distance(position).min(STEER_DISTANCE);
        let position = if distance < STEER_DISTANCE {
            position
        } else {
            let steer = (position - closest_point).normalize() * distance;
            closest_point + steer
        };

        let mut state = AgentState::new(position.x, position.y, closest_node.state.heading);
        let direction = closest_node.speed.signum();

        let next_direction = direction;
        let start_state = closest_node.state;
        let lowest_cost = nodes
            .iter()
            .enumerate()
            .fold(None, |acc, (i, existing_node)| {
                let delta = Vector2::from(state) - Vector2::from(existing_node.state);
                if REWIRE_DISTANCE.powf(2.) < delta.magnitude2() {
                    return acc;
                }
                let distance = delta.magnitude();
                let (hit, _level) = collision_check(
                    existing_node.state,
                    start_state,
                    next_direction,
                    distance,
                    0.,
                );
                if hit {
                    return acc;
                }
                let this_cost = existing_node.cost + distance;
                if let Some((_, acc_cost, _)) = acc {
                    if this_cost < acc_cost {
                        Some((i, this_cost, delta))
                    } else {
                        acc
                    }
                } else {
                    Some((i, this_cost, delta))
                }
            });

        // If this is a "shortcut", i.e. has a lower cost than existing node, "graft" the branch
        if let Some((i, lowest_cost, delta)) = lowest_cost {
            state.heading = heading_from_delta(&delta, next_direction);
            Some((i, StateWithCost::new(state, lowest_cost, 0., direction)))
        } else {
            None
        }
    }

    fn rewire(
        &self,
        nodes: &mut [StateWithCost],
        new_node: usize,
        _start: usize,
        mut collision_check: impl FnMut(AgentState, AgentState, f64, f64, f64) -> (bool, usize),
    ) {
        static TOTAL_INVOKES: AtomicUsize = AtomicUsize::new(0);
        static REWIRE_COUNT: AtomicUsize = AtomicUsize::new(0);
        let new_node_state = nodes[new_node].state;
        let new_node_cost = nodes[new_node].cost;
        let next_direction = nodes[new_node].speed.signum();

        let invokes = TOTAL_INVOKES.fetch_add(1, Ordering::Relaxed);

        for i in 0..nodes.len() {
            if i == new_node || !nodes[i].is_passable() {
                continue;
            }
            let existing_node = &nodes[i];
            let delta = Vector2::from(new_node_state) - Vector2::from(existing_node.state);
            let dist2 = delta.magnitude2();
            if REWIRE_DISTANCE.powf(2.) < dist2 {
                continue;
            }
            let Some(existing_from) = existing_node.from else { continue };
            let distance = dist2.sqrt();
            let (hit, _level) = collision_check(
                new_node_state,
                existing_node.state,
                next_direction,
                distance,
                0.,
            );
            if hit {
                continue;
            }
            let new_cost = new_node_cost + distance;
            if new_cost < existing_node.cost {
                if let Some((to_index, _)) = nodes[existing_from]
                    .to
                    .iter()
                    .copied()
                    .enumerate()
                    .find(|(_, j)| *j == i)
                {
                    nodes[existing_from].to.remove(to_index);
                };
                let existing_node = &mut nodes[i];
                let existing_cost = existing_node.cost;
                existing_node.cost = new_cost;
                existing_node.state.heading = heading_from_delta(&delta, next_direction);
                existing_node.from = Some(new_node);

                // Changing a cost of a middle node will cause cascading effect to the nodes.
                update_cost(nodes, i);

                let rewire_count = REWIRE_COUNT.fetch_add(1, Ordering::Relaxed);
                if rewire_count % 100 == 0 {
                    println!(
                        "rewired cost {existing_cost} -> {new_cost}: {rewire_count}/{invokes}"
                    );
                }
            }
        }
    }
}

fn update_cost(nodes: &mut [StateWithCost], cur: usize) {
    let parent_cost = nodes[cur].cost;
    let parent_state = nodes[cur].state;
    // Work around borrow checker by temporarily taking the children
    let children = std::mem::take(&mut nodes[cur].to);
    for node in &children {
        let child = &mut nodes[*node];
        child.cost = parent_cost + Vector2::from(child.state).distance(parent_state.into());
        update_cost(nodes, *node);
    }
    nodes[cur].to = children;
}
