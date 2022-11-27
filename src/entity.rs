use cgmath::Vector2;

use crate::{agent::Agent, agent::Bullet, game::Game, spawner::Spawner};
use std::{cell::RefCell, collections::VecDeque};

#[derive(Debug)]
pub(crate) enum Entity {
    Agent(Agent),
    Spawner(Spawner),
}

pub(crate) enum GameEvent {
    SpawnAgent { pos: [f64; 2], team: usize },
}

/// Oriented bounding box
#[derive(Debug, Clone, Copy)]
pub(crate) struct Obb {
    pub center: Vector2<f64>,
    pub xs: f64,
    pub ys: f64,
    pub orient: f64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CollisionShape {
    // Circle(f64),
    BBox(Obb),
}

impl CollisionShape {
    pub(crate) fn to_vertices(&self) -> Option<[[f64; 2]; 4]> {
        let Self::BBox(Obb {
            center,
            xs,
            ys,
            orient,
        }) = *self;
        let mut bbox = [[-xs, -ys], [-xs, ys], [xs, ys], [xs, -ys]];
        let rot = cgmath::Matrix2::from_angle(cgmath::Rad(orient));
        for vertex in &mut bbox {
            *vertex = (center + rot * Vector2::from(*vertex)).into();
        }
        Some(bbox)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct BoundingCircle {
    pub center: Vector2<f64>,
    pub radius: f64,
}

impl BoundingCircle {
    pub(crate) fn new(center: impl Into<Vector2<f64>>, radius: f64) -> Self {
        Self {
            center: center.into(),
            radius,
        }
    }
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

    pub(crate) fn bounding_circle(&self) -> BoundingCircle {
        match self {
            Self::Agent(agent) => agent.bounding_circle(),
            Self::Spawner(spawner) => BoundingCircle::new(spawner.pos, SPAWNER_RADIUS),
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

    pub(crate) fn get_avoidance_path(&self) -> Option<&[[f64; 2]]> {
        match self {
            Entity::Agent(agent) => Some(&agent.avoidance_path),
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

    pub(crate) fn get_goal(&self) -> Option<crate::agent::State> {
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
