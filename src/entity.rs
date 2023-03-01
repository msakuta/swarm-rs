use cgmath::{InnerSpace, Vector2};

use crate::{
    agent::Agent,
    agent::{interpolation::interpolate_i, AgentClass, Bullet, PathNode, AGENT_MAX_RESOURCE},
    collision::CollisionShape,
    game::Game,
    measure_time,
    qtree::QTreePathNode,
    spawner::{Spawner, SPAWNER_MAX_HEALTH, SPAWNER_MAX_RESOURCE},
};
use std::{cell::RefCell, collections::VecDeque};

pub(crate) const MAX_LOG_ENTRIES: usize = 100;

#[derive(Debug)]
pub enum Entity {
    Agent(Agent),
    Spawner(Spawner),
}

pub(crate) enum GameEvent {
    SpawnAgent {
        pos: [f64; 2],
        team: usize,
        class: AgentClass,
        spawner: usize,
    },
}

impl Entity {
    pub fn get_id(&self) -> usize {
        match self {
            Entity::Agent(agent) => agent.id,
            Entity::Spawner(spawner) => spawner.id,
        }
    }

    pub fn get_team(&self) -> usize {
        match self {
            Entity::Agent(agent) => agent.team,
            Entity::Spawner(spawner) => spawner.team,
        }
    }

    pub fn get_class(&self) -> Option<AgentClass> {
        match self {
            Entity::Agent(agent) => Some(agent.class),
            Entity::Spawner(_) => None,
        }
    }

    pub fn get_pos(&self) -> [f64; 2] {
        match self {
            Entity::Agent(agent) => agent.pos,
            Entity::Spawner(spawner) => spawner.pos,
        }
    }

    pub(crate) fn get_shape(&self) -> CollisionShape {
        match self {
            Entity::Agent(agent) => agent.get_shape(),
            Entity::Spawner(spawner) => spawner.get_shape(),
        }
    }

    pub(crate) fn get_last_state(&self) -> Option<CollisionShape> {
        match self {
            Entity::Agent(agent) => agent
                .get_last_state()
                .map(|state| state.collision_shape(agent.class)),
            Entity::Spawner(_spawner) => None, // Spawner never moves
        }
    }

    pub(crate) fn get_active(&self) -> bool {
        match self {
            Entity::Agent(agent) => agent.active,
            Entity::Spawner(spawner) => spawner.active,
        }
    }

    pub(crate) fn set_active(&mut self, active: bool) {
        match self {
            Entity::Agent(agent) => agent.active = active,
            Entity::Spawner(spawner) => spawner.active = active,
        }
    }

    pub fn get_target(&self) -> Option<usize> {
        match self {
            Entity::Agent(agent) => agent.get_target(),
            Entity::Spawner(_) => None,
        }
    }

    pub fn get_target_pos(&self, game: &Game) -> Option<[f64; 2]> {
        match self {
            Entity::Agent(agent) => agent.get_target_pos(game),
            Entity::Spawner(_) => None,
        }
    }

    pub fn get_path(&self) -> Option<&[QTreePathNode]> {
        match self {
            Entity::Agent(agent) => Some(&agent.path),
            Entity::Spawner(_) => None,
        }
    }

    pub fn get_avoidance_path(&self) -> Option<Vec<PathNode>> {
        match self {
            Entity::Agent(agent) => agent
                .search_state
                .as_ref()
                .and_then(|ss| ss.avoidance_path().map(|path| path.collect())),
            _ => None,
        }
    }

    pub fn get_avoidance_path_array(&self) -> Option<Vec<[f64; 2]>> {
        match self {
            Entity::Agent(agent) => agent.search_state.as_ref().and_then(|ss| {
                ss.avoidance_path()
                    .map(|path| path.map(|node| [node.x, node.y]).collect())
            }),
            _ => None,
        }
    }

    pub fn is_agent(&self) -> bool {
        matches!(self, Entity::Agent(_))
    }

    pub fn get_orient(&self) -> Option<f64> {
        match self {
            Entity::Agent(agent) => Some(agent.orient),
            _ => None,
        }
    }

    pub fn get_aabb(&self) -> [f64; 4] {
        match self {
            Entity::Agent(agent) => agent.get_shape().to_aabb(),
            Entity::Spawner(spawner) => Spawner::collision_shape(spawner.pos).to_aabb(),
        }
    }

    pub fn get_health(&self) -> u32 {
        match self {
            Entity::Agent(agent) => agent.health,
            Entity::Spawner(spawner) => spawner.health,
        }
    }

    pub fn get_max_health(&self) -> u32 {
        match self {
            Entity::Agent(agent) => agent.get_max_health(),
            Entity::Spawner(_) => SPAWNER_MAX_HEALTH,
        }
    }

    pub fn get_health_rate(&self) -> f64 {
        match self {
            Entity::Agent(agent) => agent.get_health_rate(),
            Entity::Spawner(spawner) => spawner.get_health_rate(),
        }
    }

    pub fn get_trace(&self) -> Option<&VecDeque<[f64; 2]>> {
        match self {
            Entity::Agent(agent) => Some(&agent.trace),
            _ => None,
        }
    }

    pub fn get_goal(&self) -> Option<crate::agent::AgentState> {
        match self {
            Entity::Agent(agent) => agent.goal,
            _ => None,
        }
    }

    pub fn get_search_state(&self) -> Option<&crate::agent::SearchState> {
        match self {
            Entity::Agent(agent) => agent.search_state.as_ref(),
            _ => None,
        }
    }

    pub fn get_search_tree(&self) -> Option<&crate::qtree::SearchTree> {
        match self {
            Entity::Agent(agent) => agent.search_tree.as_ref(),
            _ => None,
        }
    }

    pub(crate) fn get_speed(&self) -> f64 {
        match self {
            Entity::Agent(agent) => agent.speed,
            _ => 0.,
        }
    }

    pub fn resource(&self) -> i32 {
        match self {
            Entity::Agent(agent) => agent.resource,
            Entity::Spawner(spawner) => spawner.resource,
        }
    }

    pub fn max_resource(&self) -> i32 {
        match self {
            Entity::Agent(_) => AGENT_MAX_RESOURCE,
            Entity::Spawner(_) => SPAWNER_MAX_RESOURCE,
        }
    }

    pub(crate) fn remove_resource(&mut self, resource: i32) {
        match self {
            Entity::Agent(agent) => agent.resource = (agent.resource - resource).max(0),
            Entity::Spawner(spawner) => spawner.resource = (spawner.resource - resource).max(0),
        }
    }

    pub fn log_buffer(&self) -> &VecDeque<String> {
        match self {
            Entity::Agent(agent) => agent.log_buffer(),
            Entity::Spawner(spawner) => spawner.log_buffer(),
        }
    }

    pub(crate) fn damage(&mut self, damage: u32) -> bool {
        match self {
            Entity::Agent(agent) => {
                agent.health = agent.health.saturating_sub(damage);
                agent.health == 0
            }
            Entity::Spawner(spawner) => {
                spawner.health = spawner.health.saturating_sub(damage);
                spawner.health == 0
            }
        }
    }

    pub(crate) fn update(
        &mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
        bullets: &mut Vec<Bullet>,
    ) -> Vec<GameEvent> {
        let mut ret = vec![];
        match self {
            Entity::Agent(ref mut agent) => {
                agent.update(game, entities, bullets);
            }
            Entity::Spawner(ref mut spawner) => {
                ret.extend(spawner.update(game, entities));
            }
        }

        if game.params.fow_raycasting {
            let (_, time) = measure_time(|| self.fow_raycast(game));
            game.fow_raycast_profiler.borrow_mut().add(time);
        } else {
            self.defog(game);
        }
        ret
    }

    fn fow_raycast(&mut self, game: &mut Game) {
        // The circumference of the range circle
        const VISION_RANGE_CIRC: f64 = VISION_RANGE * 2. * std::f64::consts::PI;

        let pos = Vector2::from(self.get_pos());

        for i in 0..VISION_RANGE_CIRC as usize {
            let theta = i as f64 * 2. * std::f64::consts::PI / VISION_RANGE_CIRC;
            let end = pos + Vector2::new(theta.cos(), theta.sin()) * VISION_RANGE;
            if let Some((pos, end)) = pos.cast().zip(end.cast()) {
                interpolate_i(pos, end, |pos| {
                    let res = !pos
                        .cast()
                        .map(|pos| game.is_passable_at(pos.into()))
                        .unwrap_or(true);
                    if !res {
                        game.fog[self.get_team()][pos.x as usize + pos.y as usize * game.xs] = true;
                    }
                    res
                });
            }
        }
    }

    /// Erase fog unconditionally within the radius
    fn defog(&mut self, game: &mut Game) {
        let pos = Vector2::from(self.get_pos());
        let fog = &mut game.fog[self.get_team()];
        let iy0 = (pos.y - VISION_RANGE).max(0.) as usize;
        let iy1 = (pos.y + VISION_RANGE).min(game.ys as f64) as usize;
        let ix0 = (pos.x - VISION_RANGE).max(0.) as usize;
        let ix1 = (pos.x + VISION_RANGE).min(game.xs as f64) as usize;
        for iy in iy0..iy1 {
            for ix in ix0..ix1 {
                let delta = Vector2::from(pos) - Vector2::new(ix as f64, iy as f64);
                if delta.magnitude2() < VISION_RANGE.powf(2.) {
                    let p = &mut fog[ix + iy * game.xs];
                    *p = true;
                }
            }
        }
    }
}

const VISION_RANGE: f64 = 10.;
