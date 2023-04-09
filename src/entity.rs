use cgmath::{InnerSpace, Vector2};

use crate::{
    agent::Agent,
    agent::{AgentClass, Bullet, PathNode, AGENT_MAX_RESOURCE},
    behavior_tree_adapt::BehaviorTree,
    collision::CollisionShape,
    game::Game,
    measure_time,
    qtree::QTreePathNode,
    shape::Idx,
    spawner::{Spawner, SPAWNER_MAX_HEALTH, SPAWNER_MAX_RESOURCE},
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

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

    pub fn behavior_source(&self) -> Rc<String> {
        match self {
            Entity::Agent(agent) => agent.behavior_source(),
            Entity::Spawner(spawner) => spawner.behavior_source(),
        }
    }

    pub fn behavior_tree(&self) -> Option<&BehaviorTree> {
        match self {
            Entity::Agent(agent) => agent.behavior_tree(),
            _ => None,
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

        if game.params.fow {
            let (_, time) = measure_time(|| {
                if game.params.fow_raycasting {
                    self.fow_raycast(game);
                } else {
                    self.defog(game);
                }
            });
            game.fow_raycast_profiler.borrow_mut().add(time);
        }
        ret
    }

    fn fow_raycast(&mut self, game: &mut Game) {
        let pos = Vector2::from(self.get_pos());
        let pos_i = pos.cast::<i32>().unwrap();

        const VISION_RANGE_U: usize = VISION_RANGE as usize;
        const VISION_RANGE_I: i32 = VISION_RANGE as i32;
        const VISION_RANGE_FULL: usize = VISION_RANGE_U * 2 - 1;

        let graph_shape = (VISION_RANGE_U, VISION_RANGE_U);
        let pos_a: [i32; 2] = pos_i.into();

        let visibility_map = if let Some(cache) =
            game.fog_graph_cache
                .get(&self.get_id())
                .and_then(|(cache_pos, cache)| {
                    if *cache_pos == pos_a {
                        Some(cache)
                    } else {
                        None
                    }
                }) {
            cache
        } else {
            assert_eq!(game.fog_graph.len(), VISION_RANGE_U * VISION_RANGE_U);

            let mut visibility_map = vec![true; VISION_RANGE_FULL * VISION_RANGE_FULL];
            for yf in 0..VISION_RANGE_FULL {
                let y = yf as i32 - VISION_RANGE_I + 1;
                for xf in 0..VISION_RANGE_FULL {
                    let x = xf as i32 - VISION_RANGE_I + 1;
                    if VISION_RANGE_I * VISION_RANGE_I < x * x + y * y {
                        visibility_map[xf + yf * VISION_RANGE_FULL] = false;
                        continue;
                    }
                    let pos = pos_i + Vector2::new(x, y);
                    let res = game.is_passable_at(pos.cast::<f64>().unwrap().into());
                    if !res && visibility_map[xf + yf * VISION_RANGE_FULL] {
                        visibility_map[xf + yf * VISION_RANGE_FULL] = false;

                        let ray_inverse =
                            &game.fog_graph[graph_shape.idx(x.abs() as isize, y.abs() as isize)];
                        for ys in [-1, 1] {
                            if ys * y < 0 {
                                continue;
                            };
                            for xs in [-1, 1] {
                                if xs * x < 0 {
                                    continue;
                                };
                                for &[jx, jy] in ray_inverse {
                                    let jxf = (jx * xs + VISION_RANGE_I - 1) as usize;
                                    let jyf = (jy * ys + VISION_RANGE_I - 1) as usize;
                                    visibility_map[jxf + jyf * VISION_RANGE_FULL] = false;
                                }
                            }
                        }
                    }
                }
            }
            game.fog_graph_cache
                .insert(self.get_id(), (pos_i.into(), visibility_map));
            let Some((_, cache)) = game.fog_graph_cache.get(&self.get_id()) else { return };
            cache
        };

        let mut real_graph = vec![];
        for yf in 0..VISION_RANGE_FULL {
            let y = yf as i32 - VISION_RANGE_I + 1;
            for xf in 0..VISION_RANGE_FULL {
                let x = xf as i32 - VISION_RANGE_I + 1;
                let pos = pos_i + Vector2::new(x, y).cast::<i32>().unwrap();
                if visibility_map[xf + yf * VISION_RANGE_FULL] {
                    game.fog[self.get_team()].fow[pos.x as usize + pos.y as usize * game.xs] =
                        game.global_time;
                } else if game.params.fow_raycast_visible {
                    if game.fog_graph_forward[graph_shape.idx(x.abs() as isize, y.abs() as isize)]
                        .iter()
                        .any(|forward| {
                            let jxf = (forward[0] * x.signum() + VISION_RANGE_I - 1) as usize;
                            let jyf = (forward[1] * y.signum() + VISION_RANGE_I - 1) as usize;
                            if *forward == [x.abs(), y.abs()] {
                                return false;
                            }
                            !visibility_map[jxf + jyf * VISION_RANGE_FULL]
                        })
                    {
                        continue;
                    }

                    for ys in [-1, 1] {
                        if ys * y < 0 {
                            continue;
                        };
                        for xs in [-1, 1] {
                            if xs * x < 0 {
                                continue;
                            };
                            for &(mut casted) in
                                &game.fog_graph[graph_shape.idx(x.abs() as isize, y.abs() as isize)]
                            {
                                if xs < 0 {
                                    casted[0] *= -1;
                                }
                                if ys < 0 {
                                    casted[1] *= -1;
                                }
                                if casted != [0, 0] {
                                    real_graph.push([
                                        pos.into(),
                                        [pos_i.x + casted[0], pos_i.y + casted[1]],
                                    ]);
                                }
                            }
                        }
                    }
                }
            }
        }

        game.fog_graph_real.push(real_graph);
    }

    /// Erase fog unconditionally within the radius
    fn defog(&mut self, game: &mut Game) {
        let pos = Vector2::from(self.get_pos());
        let fog = &mut game.fog[self.get_team()];
        let iy0 = (pos.y - VISION_RANGE).max(0.) as usize;
        let iy1 = ((pos.y + VISION_RANGE) as usize).min(game.ys - 1) as usize;
        let ix0 = (pos.x - VISION_RANGE).max(0.) as usize;
        let ix1 = ((pos.x + VISION_RANGE) as usize).min(game.xs - 1) as usize;
        for iy in iy0..=iy1 {
            for ix in ix0..=ix1 {
                let delta = Vector2::from(pos) - Vector2::new(ix as f64, iy as f64);
                if delta.magnitude2() < VISION_RANGE.powf(2.) {
                    let p = &mut fog.fow[ix + iy * game.xs];
                    *p = game.global_time;
                }
            }
        }
    }
}

pub(crate) const VISION_RANGE: f64 = 15.;
