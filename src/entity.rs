use crate::{agent::Agent, agent::Bullet, game::Game, spawner::Spawner};
use std::{cell::RefCell, collections::VecDeque};

#[derive(Clone, Debug)]
pub(crate) enum Entity {
    Agent(Agent),
    Spawner(Spawner),
}

pub(crate) enum GameEvent {
    SpawnAgent { pos: [f64; 2], team: usize },
}

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

    pub(crate) fn get_path(&self) -> Option<&[[f64; 2]]> {
        match self {
            Entity::Agent(agent) => Some(&agent.path),
            Entity::Spawner(_) => None,
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

    pub(crate) fn get_trace(&self) -> Option<&VecDeque<[f64; 2]>> {
        match self {
            Entity::Agent(agent) => Some(&agent.trace),
            _ => None,
        }
    }

    pub(crate) fn damage(&mut self) -> bool {
        match self {
            Entity::Agent(_) => false,
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
