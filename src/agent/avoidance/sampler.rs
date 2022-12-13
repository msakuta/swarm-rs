use cgmath::{InnerSpace, MetricSpace, Vector2};
use rand::{distributions::Uniform, prelude::Distribution};

use crate::agent::{wrap_angle, Agent};

use super::{compare_distance, AgentState, SearchEnv, StateWithCost, DIST_RADIUS, DIST_THRESHOLD};

pub(in super::super) trait StateSampler {
    fn new(env: &SearchEnv) -> Self;
    fn compare_state(s1: &AgentState, s2: &AgentState) -> bool;
    fn initial_search(agent: &Agent, backward: bool) -> Vec<StateWithCost>;
    /// Sample a new state. Shall return an index to the starting node and the new state as a tuple.
    fn sample(
        &mut self,
        nodes: &[StateWithCost],
        env: &SearchEnv,
    ) -> Option<(usize, StateWithCost)>;
    fn calculate_cost(&self, distance: f64) -> f64;
    fn rewire(
        &self,
        nodes: &mut [StateWithCost],
        node: &StateWithCost,
        start: usize,
        collision_check: impl FnMut(AgentState, f64, f64, f64, f64) -> (bool, usize),
    );
}

/// Control space sampler. It is very cheap and can explore feasible path in kinematic model,
/// but it suffers from very slow space coverage rate.
pub(super) struct ForwardKinematicSampler {
    change_direction: bool,
    start_cost: Option<f64>,
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

    /// Changing direction costs
    fn calculate_cost(&self, distance: f64) -> f64 {
        self.start_cost.unwrap() + distance + if self.change_direction { 10000. } else { 0. }
    }

    fn rewire(
        &self,
        nodes: &mut [StateWithCost],
        node: &StateWithCost,
        start: usize,
        mut collision_check: impl FnMut(AgentState, f64, f64, f64, f64) -> (bool, usize),
    ) {
        let next_direction = node.speed.signum();
        let start_state = node.state;
        for i in 0..nodes.len() {
            if !Self::compare_state(&nodes[i].state, &node.state) {
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
                    return;
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
        }
    }
}

/// Spatially random sampler. It is closer to pure RRT, which guarantees asymptotically filling space coverage
pub(super) struct SpaceSampler(f64);

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
    ) -> Option<(usize, StateWithCost)> {
        let position = Vector2::new(
            rand::random::<f64>() * env.game.xs as f64,
            rand::random::<f64>() * env.game.ys as f64,
        );

        let (i, closest_node) = nodes.iter().enumerate().fold(
            None,
            |acc: Option<(usize, &StateWithCost)>, (ib, b)| {
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
            },
        )?;

        const STEER_DISTANCE: f64 = DIST_RADIUS * 2.5;

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

    fn calculate_cost(&self, distance: f64) -> f64 {
        self.0 + distance
    }

    fn rewire(
        &self,
        nodes: &mut [StateWithCost],
        node: &StateWithCost,
        start: usize,
        mut collision_check: impl FnMut(AgentState, f64, f64, f64, f64) -> (bool, usize),
    ) {
        const REWIRE_DISTANCE: f64 = DIST_RADIUS * 3.;
        let next_direction = node.speed.signum();
        let start_state = node.state;
        let lowest_cost = nodes.iter().enumerate().fold(None, |acc, (i, node)| {
            if REWIRE_DISTANCE.powf(2.) < Vector2::from(nodes[i].state).distance2(node.state.into())
            {
                return acc;
            }
            let existing_node = &nodes[i];
            let Some(existing_from) = existing_node.from else {
                return acc;
            };
            let existing_cost = existing_node.cost;
            let Some((to_index, _)) = nodes[existing_from].to
                .iter().copied().enumerate().find(|(_, j)| *j == i) else
            {
                return acc;
            };
            let distance = Vector2::from(nodes[i].state).distance(Vector2::from(start_state));
            let shortcut_cost = self.calculate_cost(distance);
            if existing_cost > shortcut_cost {
                if let Some((_, acc_cost)) = acc {
                    if shortcut_cost < acc_cost {
                        Some((i, shortcut_cost))
                    } else {
                        acc
                    }
                } else {
                    acc
                }
            } else {
                acc
            }
        });

        if let Some((i, shortcut_cost)) = lowest_cost {
            // If this is a "shortcut", i.e. has a lower cost than existing node, "graft" the branch
            let delta = Vector2::from(nodes[i].state) - Vector2::from(start_state);
            let distance = delta.magnitude();
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
                return;
            }
            let Some(existing_from) = nodes[i].from else {
                return;
            };
            let Some((to_index, _)) = nodes[existing_from].to
                .iter().copied().enumerate().find(|(_, j)| *j == i) else
            {
                return;
            };
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
    }
}
