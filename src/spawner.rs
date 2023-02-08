mod behavior_nodes;

use behavior_tree_lite::{error::LoadError, Blackboard, Context};

use self::behavior_nodes::{build_tree, SpawnFighter, SpawnWorker};
use crate::{
    agent::AgentClass,
    behavior_tree_adapt::{BehaviorTree, GetIdCommand, GetResource},
    collision::{aabb_intersects, CollisionShape, Obb},
    entity::{Entity, GameEvent},
    game::Game,
};
use std::cell::RefCell;

const SPAWNER_MAX_HEALTH: u32 = 1000;
pub(crate) const SPAWNER_MAX_RESOURCE: i32 = 1000;
pub(crate) const SPAWNER_RADIUS: f64 = 1.0;

pub(crate) type SpawnerState = [f64; 2];

#[derive(Debug)]
pub struct Spawner {
    pub id: usize,
    pub pos: [f64; 2],
    pub team: usize,
    pub active: bool,
    pub health: u32,
    pub resource: i32,
    behavior_tree: Option<BehaviorTree>,
    blackboard: Blackboard,
}

impl Spawner {
    pub(crate) fn new(
        id_gen: &mut usize,
        pos: [f64; 2],
        team: usize,
        behavior_source: &str,
    ) -> Result<Self, LoadError> {
        let id = *id_gen;
        *id_gen += 1;

        let tree = build_tree(behavior_source)?;

        Ok(Spawner {
            id,
            pos,
            team,
            active: true,
            health: SPAWNER_MAX_HEALTH,
            resource: 0,
            behavior_tree: Some(tree),
            blackboard: Blackboard::new(),
        })
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
        if self.resource < AgentClass::Worker.cost() {
            self.resource += 1;
        }

        let mut ret = vec![];

        let mut try_spawn = |class: AgentClass| -> Option<Box<dyn std::any::Any>> {
            if class.cost() <= self.resource {
                let rng = &mut game.rng;
                if entities
                    .iter()
                    .filter(|entity| {
                        entity
                            .try_borrow()
                            .map(|entity| {
                                entity.is_agent()
                                    && entity.get_team() == self.team
                                    && entity.get_class() == Some(class)
                            })
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
                        class,
                    });
                    return Some(Box::new(true));
                }
            }
            Some(Box::new(false))
        };

        if let Some(mut tree) = self.behavior_tree.take() {
            let mut ctx = Context::new(std::mem::take(&mut self.blackboard));
            let mut process = |f: &dyn std::any::Any| {
                if f.downcast_ref::<GetIdCommand>().is_some() {
                    return Some(Box::new(self.id) as Box<dyn std::any::Any>);
                } else if f.downcast_ref::<GetResource>().is_some() {
                    return Some(Box::new(self.resource));
                } else if f.downcast_ref::<SpawnFighter>().is_some() {
                    return try_spawn(AgentClass::Fighter);
                } else if f.downcast_ref::<SpawnWorker>().is_some() {
                    return try_spawn(AgentClass::Worker);
                }
                None
            };

            let _res = tree.0.tick(&mut process, &mut ctx);

            self.behavior_tree = Some(tree);
            self.blackboard = ctx.take_blackboard();
        }

        ret
    }
}
