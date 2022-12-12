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
}
