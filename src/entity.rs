use cgmath::Vector2;

use crate::{
    agent::Agent,
    agent::{Bullet, PathNode},
    collision::{CollisionShape, Obb},
    game::Game,
    qtree::QTreePathNode,
    spawner::Spawner,
};
use std::{cell::RefCell, collections::VecDeque};

#[derive(Debug)]
pub(crate) enum Entity {
    Agent(Agent),
    Spawner(Spawner),
}

pub(crate) enum GameEvent {
    SpawnAgent { pos: [f64; 2], team: usize },
}

const SPAWNER_RADIUS: f64 = 0.5;

impl Entity {
    pub(crate) fn get_id(&self) -> usize {
        match self {
            Entity::Agent(agent) => agent.id,
            Entity::Spawner(spawner) => spawner.id,
        }
    }

    pub(crate) fn get_team(&self) -> usize {
        match self {
            Entity::Agent(agent) => agent.team,
            Entity::Spawner(spawner) => spawner.team,
        }
    }

    pub(crate) fn get_pos(&self) -> [f64; 2] {
        match self {
            Entity::Agent(agent) => agent.pos,
            Entity::Spawner(spawner) => spawner.pos,
        }
    }

    pub(crate) fn get_shape(&self) -> CollisionShape {
        match self {
            Entity::Agent(agent) => agent.get_shape(),
            Entity::Spawner(spawner) => {
                let spawner_pos = Vector2::from(spawner.pos);
                CollisionShape::BBox(Obb {
                    center: spawner_pos,
                    xs: SPAWNER_RADIUS,
                    ys: SPAWNER_RADIUS,
                    orient: 0.,
                })
            }
        }
    }

    pub(crate) fn get_last_state(&self) -> Option<CollisionShape> {
        match self {
            Entity::Agent(agent) => agent.get_last_state().map(|state| state.collision_shape()),
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

    pub(crate) fn get_target(&self) -> Option<usize> {
        match self {
            Entity::Agent(agent) => agent.target,
            Entity::Spawner(_) => None,
        }
    }

    pub(crate) fn get_path(&self) -> Option<&[QTreePathNode]> {
        match self {
            Entity::Agent(agent) => Some(&agent.path),
            Entity::Spawner(_) => None,
        }
    }

    pub(crate) fn get_avoidance_path(&self) -> Option<Vec<PathNode>> {
        match self {
            Entity::Agent(agent) => agent
                .search_state
                .as_ref()
                .and_then(|ss| ss.avoidance_path().map(|path| path.collect())),
            _ => None,
        }
    }

    pub(crate) fn is_agent(&self) -> bool {
        matches!(self, Entity::Agent(_))
    }

    pub(crate) fn get_orient(&self) -> Option<f64> {
        match self {
            Entity::Agent(agent) => Some(agent.orient),
            _ => None,
        }
    }

    pub(crate) fn get_health_rate(&self) -> f64 {
        match self {
            Entity::Agent(agent) => agent.get_health_rate(),
            Entity::Spawner(spawner) => spawner.get_health_rate(),
        }
    }

    pub(crate) fn get_trace(&self) -> Option<&VecDeque<[f64; 2]>> {
        match self {
            Entity::Agent(agent) => Some(&agent.trace),
            _ => None,
        }
    }

    pub(crate) fn get_goal(&self) -> Option<crate::agent::AgentState> {
        match self {
            Entity::Agent(agent) => agent.goal,
            _ => None,
        }
    }

    pub(crate) fn get_search_state(&self) -> Option<&crate::agent::SearchState> {
        match self {
            Entity::Agent(agent) => agent.search_state.as_ref(),
            _ => None,
        }
    }

    pub(crate) fn get_search_tree(&self) -> Option<&crate::qtree::SearchTree> {
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

    pub(crate) fn damage(&mut self) -> bool {
        match self {
            Entity::Agent(agent) => {
                agent.health -= 1;
                agent.health == 0
            }
            Entity::Spawner(spawner) => {
                spawner.health -= 1;
                spawner.health == 0
            }
        }
    }

    pub(crate) fn update(
        &mut self,
        app_data: &mut Game,
        entities: &[RefCell<Entity>],
        bullets: &mut Vec<Bullet>,
    ) -> Vec<GameEvent> {
        let mut ret = vec![];
        match self {
            Entity::Agent(ref mut agent) => {
                agent.find_enemy(entities);
                agent.update(app_data, entities, bullets);
            }
            Entity::Spawner(ref mut spawner) => {
                ret.extend(spawner.update(app_data, entities));
            }
        }
        ret
    }
}
