use crate::{
    collision::{aabb_intersects, CollisionShape, Obb},
    entity::{Entity, GameEvent},
    game::Game,
};
use std::cell::RefCell;

const SPAWNER_MAX_HEALTH: u32 = 10;
pub(crate) const SPAWNER_MAX_RESOURCE: i32 = 300;
pub(crate) const SPAWNER_RADIUS: f64 = 1.0;

pub(crate) type SpawnerState = [f64; 2];

#[derive(Clone, Debug)]
pub(crate) struct Spawner {
    pub id: usize,
    pub pos: [f64; 2],
    pub team: usize,
    pub active: bool,
    pub health: u32,
    pub resource: i32,
}

impl Spawner {
    pub(crate) fn new(id_gen: &mut usize, pos: [f64; 2], team: usize) -> Self {
        let id = *id_gen;
        *id_gen += 1;
        Spawner {
            id,
            pos,
            team,
            active: true,
            health: SPAWNER_MAX_HEALTH,
            resource: 0,
        }
    }

    pub(crate) fn get_health_rate(&self) -> f64 {
        self.health as f64 / SPAWNER_MAX_HEALTH as f64
    }

    pub(crate) fn get_shape(&self) -> CollisionShape {
        Self::collision_shape(self.pos)
    }

    pub(crate) fn collision_shape(pos: SpawnerState) -> CollisionShape {
        CollisionShape::BBox(Obb {
            center: pos.into(),
            xs: SPAWNER_RADIUS,
            ys: SPAWNER_RADIUS,
            orient: 0.,
        })
    }

    pub(crate) fn qtree_collision(
        ignore: Option<usize>,
        newpos: SpawnerState,
        others: &[RefCell<Entity>],
    ) -> bool {
        let aabb = Self::collision_shape(newpos).buffer(1.).to_aabb();
        for other in others {
            let other = other.borrow();
            if Some(other.get_id()) == ignore {
                continue;
            }
            let other_aabb = other.get_shape().to_aabb();
            if aabb_intersects(&aabb, &other_aabb) {
                return true;
            }
        }
        false
    }

    pub(crate) fn update(
        &mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
    ) -> Vec<GameEvent> {
        if self.resource < 100 {
            self.resource += 1;
        }

        if 100 <= self.resource {
            let mut ret = vec![];
            let rng = &mut game.rng;
            if entities
                .iter()
                .filter(|entity| {
                    entity
                        .try_borrow()
                        .map(|entity| entity.is_agent() && entity.get_team() == self.team)
                        .unwrap_or(false)
                })
                .count()
                < 3
                && rng.next() < 0.1
            {
                ret.push(GameEvent::SpawnAgent {
                    pos: self.pos,
                    team: self.team,
                    spawner: self.id,
                })
            }
            ret
        } else {
            vec![]
        }
    }
}
