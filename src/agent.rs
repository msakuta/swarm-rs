mod agent_class;
pub mod avoidance;
mod behavior_nodes;
mod find_path;
pub mod interpolation;
mod motion;

pub use self::agent_class::AgentClass;
pub(crate) use self::avoidance::{AgentState, PathNode, SearchState};
use self::{
    behavior_nodes::{
        build_tree, AvoidanceCommand, ClearAvoidanceCommand, ClearPathNode, ClearTarget,
        CollectResource, DepositResource, DriveCommand, FaceToTargetCommand, FindEnemyCommand,
        FindFog, FindPathCommand, FindResource, FindSpawner, FollowPathCommand, GetClass,
        GetPathNextNodeCommand, GetStateCommand, GetTargetTypeNode, HasPathNode, HasTargetNode,
        IsResourceFull, IsSpawnerResourceFull, IsTargetVisibleCommand, MoveToCommand, ShootCommand,
        SimpleAvoidanceCommand, TargetIdNode, TargetPosCommand,
    },
    motion::{MotionCommandResult, OrientToResult},
};
use crate::{
    behavior_tree_adapt::{BehaviorTree, GetIdCommand, GetResource, PrintCommand},
    collision::{aabb_intersects, CollisionShape, Obb},
    entity::{Entity, MAX_LOG_ENTRIES},
    fog_of_war::{EntityShadow, FOG_MAX_AGE},
    game::{is_passable_at_i, Game, Profiler, Resource},
    measure_time,
    qtree::{PathFindResponse, QTreePath, SearchTree},
    spawner::{SPAWNER_MAX_RESOURCE, SPAWNER_RADIUS},
};
use ::behavior_tree_lite::Context;
use ::cgmath::{InnerSpace, MetricSpace, Vector2, Zero};
use behavior_tree_lite::{error::LoadError, BehaviorResult, Blackboard, Lazy};

use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    rc::Rc,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

#[derive(Clone, Debug)]
pub struct Bullet {
    pub pos: [f64; 2],
    pub velo: [f64; 2],
    pub team: usize,
    pub damage: u32,
    /// Distance traveled, used for rendering
    pub traveled: f64,
    pub shooter_class: AgentClass,
}

impl Bullet {
    pub fn new(pos: [f64; 2], velo: [f64; 2], team: usize, damage: u32, class: AgentClass) -> Self {
        Self {
            pos,
            velo,
            team,
            damage,
            traveled: 0.,
            shooter_class: class,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum AgentTarget {
    Entity(usize),
    Resource([f64; 2]),
    Fog([f64; 2]),
}

#[derive(Debug)]
pub struct Agent {
    pub(crate) target: Option<AgentTarget>,
    pub active: bool,
    pub unreachables: HashSet<usize>,
    pub id: usize,
    pub pos: [f64; 2],
    pub orient: f64,
    pub speed: f64,
    pub team: usize,
    pub(crate) class: AgentClass,
    cooldown: f64,
    pub health: u32,
    pub resource: i32,
    pub(crate) goal: Option<AgentState>,
    pub search_state: Option<SearchState>,
    pub(crate) search_tree: Option<SearchTree>,
    pub avoidance_plan: Option<Vec<(f64, f64)>>,
    pub(crate) path: QTreePath,
    pub trace: VecDeque<[f64; 2]>,
    last_motion_result: Option<MotionCommandResult>,
    last_state: Option<AgentState>,
    behavior_source: Rc<String>,
    behavior_tree: Option<BehaviorTree>,
    blackboard: Blackboard,
    log_buffer: VecDeque<String>,
}

const AGENT_SCALE: f64 = 1.;
pub const AGENT_HALFWIDTH: f64 = 0.3 * AGENT_SCALE;
pub const AGENT_HALFLENGTH: f64 = 0.6 * AGENT_SCALE;
pub(crate) const AGENT_SPEED: f64 = 0.125;
pub(crate) const AGENT_MAX_HEALTH: u32 = 100;
pub(crate) const AGENT_MAX_RESOURCE: i32 = 100;
const AGENT_VISIBLE_DISTANCE: f64 = 30.;
pub const BULLET_RADIUS: f64 = 0.15;
pub(crate) const BULLET_SPEED: f64 = 2.;
pub(crate) const BULLET_DAMAGE: u32 = 10;

struct GameEnv<'a> {
    _game: &'a mut Game,
    entities: &'a [RefCell<Entity>],
}

impl Agent {
    pub(crate) fn new(
        id_gen: &mut usize,
        pos: [f64; 2],
        orient: f64,
        team: usize,
        class: AgentClass,
        behavior_source: Rc<String>,
    ) -> Result<Self, LoadError> {
        let id = *id_gen;
        *id_gen += 1;

        let (tree, _build_time) = measure_time(|| build_tree(&behavior_source));
        let tree = tree?;
        // println!("tree build: {}", build_time);

        Ok(Self {
            target: None,
            active: true,
            unreachables: HashSet::new(),
            id,
            pos,
            orient,
            speed: 0.,
            team,
            class,
            cooldown: 5.,
            health: class.health(),
            resource: 0,
            goal: None,
            search_state: None,
            search_tree: None,
            avoidance_plan: None,
            path: vec![],
            trace: VecDeque::new(),
            last_motion_result: None,
            last_state: None,
            behavior_source,
            behavior_tree: Some(tree),
            blackboard: Blackboard::new(),
            log_buffer: VecDeque::new(),
        })
    }

    pub(crate) fn get_target(&self) -> Option<usize> {
        self.target.and_then(|target| {
            if let AgentTarget::Entity(id) = target {
                Some(id)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_target_pos(&self, game: &Game) -> Option<[f64; 2]> {
        self.target.and_then(|target| match target {
            AgentTarget::Entity(id) => game.entities.iter().find_map(|entity| {
                let entity = entity.try_borrow().ok()?;
                if entity.get_id() == id {
                    Some(entity.get_pos())
                } else {
                    None
                }
            }),
            AgentTarget::Resource(pos) | AgentTarget::Fog(pos) => Some(pos),
        })
    }

    pub(crate) fn get_shape(&self) -> CollisionShape {
        CollisionShape::BBox(Obb {
            center: self.pos.into(),
            xs: AGENT_HALFLENGTH,
            ys: AGENT_HALFWIDTH,
            orient: self.orient,
        })
    }

    pub(crate) fn get_last_state(&self) -> Option<AgentState> {
        self.last_state
    }

    pub(crate) fn get_health_rate(&self) -> f64 {
        self.health as f64 / self.class.health() as f64
    }

    pub(crate) fn get_max_health(&self) -> u32 {
        self.class.health()
    }

    pub(crate) fn log_buffer(&self) -> &VecDeque<String> {
        &self.log_buffer
    }

    pub(crate) fn behavior_source(&self) -> Rc<String> {
        self.behavior_source.clone()
    }

    pub(crate) fn behavior_tree(&self) -> Option<&BehaviorTree> {
        self.behavior_tree.as_ref()
    }

    /// Check collision in qtree bounding boxes
    pub(crate) fn qtree_collision(
        ignore: Option<usize>,
        newpos: AgentState,
        class: AgentClass,
        others: &[RefCell<Entity>],
    ) -> bool {
        let aabb = newpos.collision_shape(class).to_aabb();
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

    /// Check collision with other entities, but not walls
    ///
    /// Simpler interface of [`collision_check_fn`]
    pub(crate) fn collision_check(
        ignore: Option<usize>,
        newpos: AgentState,
        class: AgentClass,
        others: &[RefCell<Entity>],
        prediction: bool,
    ) -> bool {
        Self::collision_check_fn(|id| Some(id) == ignore, newpos, class, others, prediction)
    }

    /// Check collision with other entities, but not walls
    pub(crate) fn collision_check_fn(
        ignore: impl Fn(usize) -> bool,
        newpos: AgentState,
        class: AgentClass,
        others: &[RefCell<Entity>],
        prediction: bool,
    ) -> bool {
        let shape = newpos.collision_shape(class);
        for entity in others.iter() {
            if let Ok(entity) = entity.try_borrow() {
                if ignore(entity.get_id()) {
                    continue;
                }
                let buffer = if prediction && entity.get_speed() != 0. {
                    1.
                } else {
                    0.
                };
                let dist2 = Vector2::from(entity.get_pos()).distance2(Vector2::from(newpos));
                if dist2 < ((AGENT_HALFLENGTH + buffer) * 2.).powf(2.) {
                    let mut entity_shape = entity.get_shape();
                    if buffer != 0. {
                        entity_shape = entity_shape.buffer(buffer);
                    }
                    if shape.intersects(&entity_shape) {
                        // println!("Collision with another entity");
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(crate) fn find_enemy(&mut self, game: &Game, agents: &[RefCell<Entity>]) {
        let best_agent = agents
            .iter()
            .filter_map(|a| a.try_borrow().ok())
            .filter(|a| {
                let aid = a.get_id();
                let ateam = a.get_team();
                let apos = a.get_pos();
                game.is_clear_fog_at(self.team, apos)
                    && !self.unreachables.contains(&aid)
                    && aid != self.id
                    && ateam != self.team
            })
            .filter_map(|a| {
                let distance = Vector2::from(a.get_pos()).distance(Vector2::from(self.pos));
                Some((distance, a))
            })
            .fold(None, |acc: Option<(f64, _)>, cur| {
                if let Some(acc) = acc {
                    if cur.0 < acc.0 {
                        Some(cur)
                    } else {
                        Some(acc)
                    }
                } else {
                    Some(cur)
                }
            });

        // Theoretically, a shadow entity could have shorter distance than know entities.
        let best_shadow = game.fog[self.team]
            .entities
            .iter()
            .map(|a| {
                let distance = Vector2::from(a.pos).distance(Vector2::from(self.pos));
                (distance, a)
            })
            .fold(None, |acc: Option<(f64, &EntityShadow)>, cur| {
                if let Some(acc) = acc {
                    if cur.0 < acc.0 {
                        Some(cur)
                    } else {
                        Some(acc)
                    }
                } else {
                    Some(cur)
                }
            });

        match (best_agent, best_shadow) {
            (Some(agent), Some(shadow)) => {
                self.target = if agent.0 < shadow.0 {
                    Some(AgentTarget::Entity(agent.1.get_id()))
                } else {
                    Some(AgentTarget::Entity(shadow.1.id))
                };
            }
            (Some((_, agent)), None) => {
                self.target = Some(AgentTarget::Entity(agent.get_id()));
            }
            (None, Some((_, shadow))) => {
                self.target = Some(AgentTarget::Entity(shadow.id));
            }
            _ => self.target = None,
        }
    }

    fn has_target(&self, entities: &[RefCell<Entity>]) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        match target {
            AgentTarget::Entity(id) => entities.iter().any(|entity| {
                let Ok(entity) = entity.try_borrow() else {
                    return false;
                };
                entity.get_id() == id
            }),
            AgentTarget::Resource(_) | AgentTarget::Fog(_) => true,
        }
    }

    fn get_target_type(&self) -> Option<Box<dyn std::any::Any>> {
        Some(Box::new(
            match self.target? {
                AgentTarget::Entity(_) => "Entity",
                AgentTarget::Fog(_) => "Fog",
                AgentTarget::Resource(_) => "Resource",
            }
            .to_string(),
        ))
    }

    fn is_spawner_resource_full(&self, entities: &[RefCell<Entity>]) -> bool {
        entities
            .iter()
            .filter_map(|a| a.try_borrow().ok())
            .filter(|a| {
                let aid = a.get_id();
                let ateam = a.get_team();
                !self.unreachables.contains(&aid)
                    && aid != self.id
                    && ateam == self.team
                    && !a.is_agent()
            })
            .all(|a| {
                if let Entity::Spawner(spawner) = &*a {
                    spawner.resource == SPAWNER_MAX_RESOURCE
                } else {
                    false
                }
            })
    }

    fn find_spawner(&mut self, agents: &[RefCell<Entity>]) {
        let best_spawner = agents
            .iter()
            .filter_map(|a| a.try_borrow().ok())
            .filter(|a| {
                let aid = a.get_id();
                let ateam = a.get_team();
                !self.unreachables.contains(&aid)
                    && aid != self.id
                    && ateam == self.team
                    && !a.is_agent()
            })
            .filter_map(|a| {
                let distance = Vector2::from(a.get_pos()).distance(Vector2::from(self.pos));
                Some((distance, a))
            })
            .fold(None, |acc: Option<(f64, _)>, cur| {
                if let Some(acc) = acc {
                    if cur.0 < acc.0 {
                        Some(cur)
                    } else {
                        Some(acc)
                    }
                } else {
                    Some(cur)
                }
            });

        if let Some((_dist, spawner)) = best_spawner {
            self.target = Some(AgentTarget::Entity(spawner.get_id()));
        }
    }

    pub(crate) fn find_resource(&mut self, resources: &[Resource]) -> bool {
        let best_resource = resources
            .iter()
            // .filter_map(|a| a.try_borrow().ok())
            .filter_map(|a| {
                let distance = Vector2::from(a.pos).distance(Vector2::from(self.pos));
                Some((distance, a))
            })
            .fold(None, |acc: Option<(f64, _)>, cur| {
                if let Some(acc) = acc {
                    if cur.0 < acc.0 {
                        Some(cur)
                    } else {
                        Some(acc)
                    }
                } else {
                    Some(cur)
                }
            });

        if let Some((_dist, resource)) = best_resource {
            self.target = Some(AgentTarget::Resource(resource.pos));
            true
        } else {
            false
        }
    }

    fn collect_resource(&mut self, resources: &mut [Resource]) -> BehaviorResult {
        if AGENT_MAX_RESOURCE <= self.resource {
            return BehaviorResult::Fail;
        }
        for resource in resources {
            if Vector2::from(resource.pos).distance2(Vector2::from(self.pos))
                < (AGENT_HALFLENGTH * 2.).powf(2.)
                && 0 < resource.amount
            {
                let moved = resource
                    .amount
                    .min(AGENT_MAX_RESOURCE - self.resource)
                    .min(10);
                resource.amount -= moved;
                self.resource += moved;
                return BehaviorResult::Running;
            }
        }
        BehaviorResult::Success
    }

    fn deposit_resource(&mut self, entities: &[RefCell<Entity>]) -> BehaviorResult {
        if self.resource == 0 {
            return BehaviorResult::Fail;
        }
        for mut entity in entities.iter().filter_map(|ent| ent.try_borrow_mut().ok()) {
            let Entity::Spawner(ref mut spawner) = &mut entity as &mut Entity else {
                continue;
            };
            if spawner.team != self.team {
                continue;
            }
            if Vector2::from(spawner.pos).distance2(Vector2::from(self.pos))
                < ((AGENT_HALFLENGTH + SPAWNER_RADIUS) * 1.5).powf(2.)
                && spawner.resource < SPAWNER_MAX_RESOURCE
            {
                let moved = self
                    .resource
                    .min(SPAWNER_MAX_RESOURCE - spawner.resource)
                    .min(10);
                self.resource -= moved;
                spawner.resource += moved;
                if self.resource == 0 {
                    self.path.clear();
                    return BehaviorResult::Success;
                } else {
                    return BehaviorResult::Running;
                }
            }
        }
        BehaviorResult::Success
    }

    pub(crate) fn find_fog(&mut self, game: &mut Game) -> bool {
        let team = self.team;
        let qtree = &game.qtree;
        let found_path = self.find_path_many(qtree, &game.path_find_profiler, |pos| {
            if game.is_passable_at(pos) && game.is_fog_older_than(team, pos, FOG_MAX_AGE) {
                PathFindResponse::Goal
            } else {
                PathFindResponse::Continue
            }
        });
        let Ok(path) = found_path else { return false };
        match path.first().copied() {
            Some(node) => {
                self.path = path;
                self.target = Some(AgentTarget::Fog(node.pos));
                true
            }
            _ => false,
        }
    }

    pub(crate) fn shoot_bullet(&mut self, bullets: &mut Vec<Bullet>, target_pos: [f64; 2]) -> bool {
        if 0. < self.cooldown {
            return false;
        }
        let dir = Vector2::new(self.orient.cos(), self.orient.sin());
        if dir.dot((Vector2::from(target_pos) - Vector2::from(self.pos)).normalize()) < 0.5 {
            return false;
        }
        let bullet = Bullet::new(
            self.pos,
            (dir * self.class.bullet_speed()).into(),
            self.team,
            self.class.damage(),
            self.class,
        );

        bullets.push(bullet);

        self.cooldown += self.class.cooldown();
        true
    }

    pub fn get_avoidance_state(&self, (drive, steer): (f64, f64)) -> Vector2<f64> {
        let desired_angle = wrap_angle(self.orient + steer);
        drive * Vector2::new(desired_angle.cos(), desired_angle.sin()) + Vector2::from(self.pos)
    }

    pub(super) fn get_avoidance_agent_state(&self, (drive, steer): (f64, f64)) -> AgentState {
        let desired_angle = wrap_angle(self.orient + steer);
        let pos = drive * Vector2::new(desired_angle.cos(), desired_angle.sin())
            + Vector2::from(self.pos);
        AgentState {
            x: pos.x,
            y: pos.y,
            heading: desired_angle,
        }
    }

    fn do_simple_avoidance(
        &mut self,
        _game: &mut Game,
        entities: &[RefCell<Entity>],
    ) -> Option<OrientToResult> {
        let Some(steer) = &self.avoidance_plan else {
            return None;
        };
        let min_steer = steer
            .iter()
            .min_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap());
        let Some(&(drive, steer)) = min_steer else {
            return None;
        };
        if steer != 0. {
            let target = self.get_avoidance_state((drive, steer));
            let res = self.orient_to(target.into(), false, entities);
            println!("{}: do_simple_avoidance: {res:?} steer: {steer:?}", self.id);
            Some(res)
        } else {
            None
        }
    }

    fn do_avoidance(
        &mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
        cmd: &AvoidanceCommand,
    ) -> Box<dyn std::any::Any> {
        static TIME_WINDOW: Lazy<Mutex<VecDeque<f64>>> = Lazy::new(|| Mutex::new(VecDeque::new()));
        static AVG_COUNT: AtomicUsize = AtomicUsize::new(0);
        self.goal = Some(avoidance::AgentState::new(
            cmd.goal[0],
            cmd.goal[1],
            self.orient,
        ));
        let (res, time) = measure_time(|| {
            self.avoidance_search(game, entities, cmd.back, false, game.params.avoidance_mode)
        });
        // println!("Avoidance goal set to {:?}, returns {res:?}", self.goal);
        if let Ok(mut time_window) = TIME_WINDOW.lock() {
            time_window.push_back(time);
            while 100 < time_window.len() {
                time_window.pop_front();
            }
            let count = AVG_COUNT.fetch_add(1, Ordering::Relaxed);
            if count % 100 == 0 && !time_window.is_empty() {
                let avg = time_window.iter().sum::<f64>() / time_window.len() as f64;
                println!("Avoidance search ({count}): {avg:.06}s");
            }
        }
        Box::new(res)
    }

    pub(crate) fn update(
        &mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
        bullets: &mut Vec<Bullet>,
    ) {
        if let Some(mut tree) = self.behavior_tree.take() {
            enum Command {
                Drive(DriveCommand),
                MoveTo(MoveToCommand),
                FollowPath(FollowPathCommand),
                FaceToTarget(FaceToTargetCommand),
            }
            let mut command = None;
            let mut ctx = Context::new(std::mem::take(&mut self.blackboard));
            ctx.set("target", self.target);
            ctx.set("has_path", !self.path.is_empty());
            let mut process = |f: &dyn std::any::Any| {
                if f.downcast_ref::<GetIdCommand>().is_some() {
                    return Some(Box::new(self.id) as Box<dyn std::any::Any>);
                } else if let Some(s) = f.downcast_ref::<PrintCommand>() {
                    self.log(s.0.clone());
                } else if f.downcast_ref::<GetResource>().is_some() {
                    return Some(Box::new(self.resource));
                } else if let Some(com) = f.downcast_ref::<DriveCommand>() {
                    command = Some(Command::Drive(*com));
                    return MotionCommandResult::as_drive(&self.last_motion_result);
                } else if let Some(com) = f.downcast_ref::<MoveToCommand>() {
                    command = Some(Command::MoveTo(*com));
                    return MotionCommandResult::as_move_to(&self.last_motion_result);
                } else if f.downcast_ref::<GetClass>().is_some() {
                    return Some(Box::new(self.class));
                } else if f.downcast_ref::<HasTargetNode>().is_some() {
                    return Some(Box::new(self.has_target(&entities)));
                } else if f.downcast_ref::<GetTargetTypeNode>().is_some() {
                    return self.get_target_type();
                } else if f.downcast_ref::<TargetIdNode>().is_some() {
                    return Some(Box::new(self.target));
                } else if f.downcast_ref::<FindEnemyCommand>().is_some() {
                    self.find_enemy(game, entities)
                } else if f.downcast_ref::<FindSpawner>().is_some() {
                    self.find_spawner(entities)
                } else if f.downcast_ref::<FindResource>().is_some() {
                    return Some(Box::new(self.find_resource(&game.fog[self.team].resources)));
                } else if f.downcast_ref::<FindFog>().is_some() {
                    return Some(Box::new(self.find_fog(game)));
                } else if f.downcast_ref::<ClearTarget>().is_some() {
                    let had_target = self.target.is_some();
                    self.target = None;
                    return Some(Box::new(had_target));
                } else if f.downcast_ref::<CollectResource>().is_some() {
                    return Some(Box::new(self.collect_resource(&mut game.resources)));
                } else if f.downcast_ref::<DepositResource>().is_some() {
                    return Some(Box::new(self.deposit_resource(&entities)));
                } else if f.downcast_ref::<IsResourceFull>().is_some() {
                    return Some(Box::new(AGENT_MAX_RESOURCE <= self.resource));
                } else if f.downcast_ref::<IsSpawnerResourceFull>().is_some() {
                    return Some(Box::new(self.is_spawner_resource_full(&entities)));
                } else if f.downcast_ref::<HasPathNode>().is_some() {
                    return Some(Box::new(!self.path.is_empty()));
                } else if f.downcast_ref::<ClearPathNode>().is_some() {
                    let ret = !self.path.is_empty();
                    self.path.clear();
                    return Some(Box::new(ret));
                } else if f.downcast_ref::<TargetPosCommand>().is_some() {
                    match self.target {
                        Some(AgentTarget::Entity(target)) => {
                            let found = entities.iter().find(|a| {
                                a.try_borrow()
                                    .map(|a| a.get_id() == target)
                                    .unwrap_or(false)
                            });
                            if let Some(target) = found {
                                let target = target.borrow_mut();
                                return Some(Box::new(target.get_pos()));
                            } else {
                                println!("Target could not be found!");
                            }
                        }
                        Some(AgentTarget::Resource(pos)) | Some(AgentTarget::Fog(pos)) => {
                            return Some(Box::new(pos))
                        }
                        _ => (),
                    }
                } else if let Some(com) = f.downcast_ref::<FindPathCommand>() {
                    let found_path = self.find_path(com, game);
                    return Some(Box::new(found_path));
                } else if let Some(cmd) = f.downcast_ref::<FollowPathCommand>() {
                    command = Some(Command::FollowPath(*cmd));
                    return MotionCommandResult::as_follow_path(&self.last_motion_result);
                } else if f.downcast_ref::<ShootCommand>().is_some() {
                    let forward = Vector2::new(self.orient.cos(), self.orient.sin());
                    self.shoot_bullet(bullets, (Vector2::from(self.pos) + forward).into());
                } else if let Some(goal) = f.downcast_ref::<AvoidanceCommand>() {
                    return Some(self.do_avoidance(game, entities, goal));
                } else if let Some(cmd) = f.downcast_ref::<SimpleAvoidanceCommand>() {
                    let routes = self.plan_simple_avoidance(cmd.0, entities);
                    self.avoidance_plan = Some(routes);
                    return Some(Box::new(true));
                } else if f.downcast_ref::<ClearAvoidanceCommand>().is_some() {
                    self.search_state = None;
                    self.avoidance_plan = None;
                } else if f.downcast_ref::<GetPathNextNodeCommand>().is_some() {
                    if let Some(path) = self.path.last() {
                        return Some(Box::new(path.pos));
                    }
                } else if f.downcast_ref::<GetStateCommand>().is_some() {
                    return Some(Box::new(self.to_state()));
                } else if let Some(com) = f.downcast_ref::<IsTargetVisibleCommand>() {
                    let target_pos = com.0;
                    let ret = Box::new(self.is_position_visible(
                        target_pos,
                        &game,
                        &mut game.triangle_profiler.borrow_mut(),
                    ));
                    return Some(ret);
                } else if let Some(com) = f.downcast_ref::<FaceToTargetCommand>() {
                    command = Some(Command::FaceToTarget(*com));
                    return MotionCommandResult::as_face_to_target(&self.last_motion_result);
                }
                None
                // if let Some(f) = f.downcast_ref::<dyn Fn(&Agent)>() {
                //     f(self);
                // }
            };
            let _res = tree.0.tick(&mut process, &mut ctx);
            // eprintln!("[{}] BehaviorTree ticked! {:?}", self.id, res);
            self.behavior_tree = Some(tree);
            self.blackboard = ctx.take_blackboard();

            if command.is_some() {
                self.last_state = Some(self.to_state());
            } else {
                self.last_state = None;
            }

            // An agent can only run one command per tick.
            match command {
                Some(Command::Drive(com)) => {
                    let res = self.drive(com.0, game, entities);
                    self.last_motion_result = Some(MotionCommandResult::Drive(res));
                }
                Some(Command::MoveTo(com)) => {
                    let res = self.move_to(game, com.0, false, entities);
                    self.last_motion_result = Some(MotionCommandResult::MoveTo(res));
                }
                Some(Command::FollowPath(_com)) => {
                    let res = self.follow_path(game, entities);
                    self.last_motion_result = Some(MotionCommandResult::FollowPath(res));
                }
                Some(Command::FaceToTarget(com)) => {
                    let res = self.orient_to(com.0, false, entities);
                    self.last_motion_result = Some(MotionCommandResult::FaceToTarget(res));
                }
                _ => {
                    self.last_motion_result = None;
                    self.speed = 0.;
                }
            }
        }

        if let (Some(_), _time) = measure_time(|| {
            self.check_avoidance_collision(&GameEnv {
                _game: game,
                entities,
            })
        }) {
            // println!("check_avoidance_collision time: {time:.06}");
        }

        // if let Some(target) = self.target.and_then(|target| {
        //     entities.iter().find(|a| {
        //         a.try_borrow()
        //             .map(|a| a.get_id() == target)
        //             .unwrap_or(false)
        //     })
        // }) {
        //     let target = target.borrow_mut();
        //     if 5. < Vector2::from(target.get_pos()).distance(Vector2::from(self.pos)) {
        //         if self.find_path(Some(&target), game).is_ok() {
        //             if let Some(target) = self.path.last() {
        //                 let target_pos = *target;
        //                 self.move_to(game, target_pos, entities);
        //             }
        //         } else {
        //             self.move_to(game, target.get_pos(), entities);
        //         }
        //     } else {
        //         // println!("Orienting {}", self.id);
        //         self.orient_to(target.get_pos());
        //     }
        //     self.shoot_bullet(bullets, target.get_pos());
        // } else {
        //     self.path = vec![];
        // }
        self.cooldown = (self.cooldown - 1.).max(0.);
    }

    fn log(&mut self, msg: String) {
        self.log_buffer.push_back(msg);
        while MAX_LOG_ENTRIES < self.log_buffer.len() {
            self.log_buffer.pop_front();
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum MotionResult {
    Following,
    Blocked,
    Arrived,
}

impl From<bool> for MotionResult {
    fn from(b: bool) -> Self {
        if b {
            MotionResult::Following
        } else {
            MotionResult::Blocked
        }
    }
}

impl From<MotionResult> for bool {
    fn from(b: MotionResult) -> bool {
        matches!(b, MotionResult::Following)
    }
}

impl Agent {
    fn follow_path(&mut self, game: &mut Game, entities: &[RefCell<Entity>]) -> MotionResult {
        if self.follow_avoidance_path(game, entities) {
            MotionResult::Following
        } else if let Some(target) = self.path.last() {
            if target.radius < Vector2::from(target.pos).distance(Vector2::from(self.pos)) {
                let target_pos = target.pos;
                let res = self.do_simple_avoidance(game, entities);
                match res {
                    Some(OrientToResult::Blocked) => MotionResult::Blocked,
                    Some(OrientToResult::Approaching) | Some(OrientToResult::Arrived) => {
                        MotionResult::Following
                    }
                    None => self.move_to(game, target_pos, false, entities).into(),
                }
            } else {
                self.path.pop();
                MotionResult::Following
            }
        } else {
            MotionResult::Arrived
        }
    }

    fn is_position_visible(&self, target: [f64; 2], game: &Game, _profiler: &mut Profiler) -> bool {
        let board = &game.board;
        let shape = (game.xs, game.ys);

        let self_pos = self.pos;
        let self_vec = Vector2::from(self_pos);
        let target_vec = Vector2::from(target);

        let distance = self_vec.distance(target_vec);
        if AGENT_VISIBLE_DISTANCE < distance {
            return false;
        }
        let Some(self_veci) = self_vec.cast::<i32>() else {
            return false;
        };
        let Some(target_veci) = target_vec.cast::<i32>() else {
            return false;
        };
        let delta = self_veci - target_veci;
        let horizontal = delta.y.abs() < delta.x.abs();
        let check_shape = if horizontal {
            [Vector2::new(0, -1), Vector2::zero(), Vector2::new(0, 1)]
        } else {
            [Vector2::new(-1, 0), Vector2::zero(), Vector2::new(1, 0)]
        };

        let mut raycast_board = if game.enable_raycast_board {
            Some(game.raycast_board.borrow_mut())
        } else {
            None
        };

        !interpolation::interpolate_i(self_veci, target_veci, |point| {
            let point = Vector2::from(point);
            check_shape.iter().any(|check_pix| {
                let pix = point + check_pix;
                if let Some(raycast_board) = &mut raycast_board {
                    raycast_board[pix.x as usize + pix.y as usize * shape.0] += 1;
                }
                !is_passable_at_i(board, shape, pix)
            })
        })
    }
}

/// Wrap the angle value in [-pi, pi)
pub(crate) fn wrap_angle(x: f64) -> f64 {
    use std::f64::consts::PI;
    const TWOPI: f64 = PI * 2.;
    // ((x + PI) - ((x + PI) / TWOPI).floor() * TWOPI) - PI
    x - (x + PI).div_euclid(TWOPI) * TWOPI
}
