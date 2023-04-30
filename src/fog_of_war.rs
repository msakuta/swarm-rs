use std::cell::RefCell;

use crate::{
    agent::interpolation::interpolate_raycast,
    entity::Entity,
    game::{Board, Game, Resource},
};

pub(crate) const FOG_MAX_AGE: i32 = 10000;

/// A struct representing the subjective knowledge about the world state.
#[derive(Debug, Clone)]
pub struct FogOfWar {
    pub fow: Vec<i32>,
    pub resources: Vec<Resource>,
    pub entities: Vec<EntityShadow>,
}

impl FogOfWar {
    pub(crate) fn new(board: &Board) -> Self {
        Self {
            fow: vec![i32::MIN; board.len()],
            resources: vec![],
            entities: vec![],
        }
    }
}

/// A struct representing a memory of enemy spawner
#[derive(Debug, Clone)]
pub struct EntityShadow {
    pub id: usize,
    pub pos: [f64; 2],
    pub health: u32,
}

impl From<&Entity> for EntityShadow {
    fn from(entity: &Entity) -> Self {
        Self {
            id: entity.get_id(),
            pos: entity.get_pos(),
            health: entity.get_health(),
        }
    }
}

/// A map from a pixel and a collection of pixels that would be obscured if the given pixel was an obstacle.
pub(crate) type FogRaycastMap = Vec<Vec<[i32; 2]>>;

impl Game {
    pub(crate) fn fog_resource(&mut self, team: usize) {
        // Clean up stale memory in visible area
        let resources = std::mem::take(&mut self.fog[team].resources);
        let mut resources: Vec<Resource> = resources
            .into_iter()
            .filter(|res| !self.is_clear_fog_at(team, res.pos))
            .collect();

        for resource in &self.resources {
            if !self.is_clear_fog_at(team, resource.pos) {
                continue;
            }
            if resources.iter().all(|res| res.pos != resource.pos) {
                resources.push(resource.clone());
            }
        }

        let fog = &mut self.fog[team];
        fog.resources = resources;
    }

    pub(crate) fn fog_entities(&mut self, team: usize, entities: &[RefCell<Entity>]) {
        let shadow_entities = std::mem::take(&mut self.fog[team].entities);
        let mut shadow_entities: Vec<_> = shadow_entities
            .into_iter()
            .filter(|res| !self.is_clear_fog_at(team, res.pos))
            .collect();

        for entity in entities {
            let entity = entity.borrow();
            if entity.is_agent()
                || entity.get_team() == team
                || !self.is_clear_fog_at(team, entity.get_pos())
            {
                continue;
            }
            if shadow_entities
                .iter()
                .all(|res| res.pos != entity.get_pos())
            {
                shadow_entities.push((&entity as &Entity).into());
            }
        }

        // println!(
        //     "team {team} entities: {} {:?}",
        //     shadow_entities.len(),
        //     shadow_entities.first().map(|e| &e as *const _)
        // );

        self.fog[team].entities = shadow_entities;
    }
}

/// Compute raycast graph with given size. Both forward and backward maps are returned, but the
/// useful one is backward, to quickly fill obstructed pixels.
pub(crate) fn precompute_raycast_map(range: usize) -> (Vec<Vec<[i32; 2]>>, Vec<Vec<[i32; 2]>>) {
    let mut backward = vec![vec![]; range * range];
    let mut forward = vec![vec![]; range * range];
    for y in 0..range as i32 {
        for x in 0..range as i32 {
            interpolate_raycast([0, 0], [x, y], |p| {
                backward[p.x as usize + p.y as usize * range].push([x, y].into());
                forward[x as usize + y as usize * range].push(p.into());
                false
            });
        }
    }
    (backward, forward)
}
