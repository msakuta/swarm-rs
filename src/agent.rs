mod avoidance;
mod behavior_nodes;
mod find_path;
mod interpolation;
mod motion;

pub(crate) use self::avoidance::{AgentState, PathNode, SearchState};
use self::{
    avoidance::DIST_RADIUS,
    behavior_nodes::{
        build_tree, AvoidanceCommand, BehaviorTree, ClearAvoidanceCommand, DriveCommand,
        FaceToTargetCommand, FindEnemyCommand, FindPathCommand, FollowPathCommand,
        GetPathNextNodeCommand, GetStateCommand, IsTargetVisibleCommand, MoveToCommand,
        ShootCommand,
    },
    motion::MotionResult,
};
use crate::{
    collision::{CollisionShape, Obb},
    entity::Entity,
    game::{Game, Profiler},
    measure_time,
    mesh::Mesh,
    triangle_utils::find_triangle_at,
};
use ::behavior_tree_lite::Context;
use ::cgmath::{InnerSpace, MetricSpace, Vector2};
use behavior_tree_lite::{error::LoadError, Blackboard, Lazy};

use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct Bullet {
    pub pos: [f64; 2],
    pub velo: [f64; 2],
    pub team: usize,
    /// Distance traveled, used for rendering
    pub traveled: f64,
}

#[derive(Debug)]
pub(crate) struct Agent {
    pub target: Option<usize>,
    pub active: bool,
    pub unreachables: HashSet<usize>,
    pub id: usize,
    pub pos: [f64; 2],
    pub orient: f64,
    pub speed: f64,
    pub team: usize,
    cooldown: f64,
    pub health: u32,
    pub goal: Option<AgentState>,
    pub search_state: Option<SearchState>,
    /// Avoidance path is more local.
    pub avoidance_path: Vec<PathNode>,
    pub path: Vec<[f64; 2]>,
    pub trace: VecDeque<[f64; 2]>,
    last_motion_result: Option<MotionResult>,
    behavior_tree: Option<BehaviorTree>,
    blackboard: Blackboard,
}

pub(crate) const AGENT_HALFWIDTH: f64 = 0.3 * 4.;
pub(crate) const AGENT_HALFLENGTH: f64 = 0.6 * 4.;
pub(crate) const AGENT_SPEED: f64 = 0.25;
pub(crate) const AGENT_MAX_HEALTH: u32 = 3;
const AGENT_VISIBLE_DISTANCE: f64 = 10.;
pub(crate) const BULLET_RADIUS: f64 = 0.15;
pub(crate) const BULLET_SPEED: f64 = 2.;

impl Agent {
    pub(crate) fn new(
        id_gen: &mut usize,
        pos: [f64; 2],
        orient: f64,
        team: usize,
        behavior_source: &str,
    ) -> Result<Self, LoadError> {
        let id = *id_gen;
        *id_gen += 1;

        let (tree, build_time) = measure_time(|| build_tree(behavior_source));
        let tree = tree?;
        println!("tree build: {}", build_time);

        Ok(Self {
            target: None,
            active: true,
            unreachables: HashSet::new(),
            id,
            pos,
            orient,
            speed: 0.,
            team,
            cooldown: 5.,
            health: AGENT_MAX_HEALTH,
            goal: None,
            search_state: None,
            avoidance_path: vec![],
            path: vec![],
            trace: VecDeque::new(),
            last_motion_result: None,
            behavior_tree: Some(tree),
            blackboard: Blackboard::new(),
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

    pub(crate) fn get_health_rate(&self) -> f64 {
        self.health as f64 / AGENT_MAX_HEALTH as f64
    }

    /// Check collision with other entities, but not walls
    pub(crate) fn collision_check(
        ignore: Option<usize>,
        newpos: AgentState,
        others: &[RefCell<Entity>],
    ) -> bool {
        let shape = newpos.collision_shape();
        for entity in others.iter() {
            if let Ok(entity) = entity.try_borrow() {
                if Some(entity.get_id()) == ignore {
                    continue;
                }
                let dist2 = Vector2::from(entity.get_pos()).distance2(Vector2::from(newpos));
                if dist2 < (AGENT_HALFLENGTH * 2.).powf(2.) {
                    let entity_shape = entity.get_shape();
                    if shape.intersects(&entity_shape) {
                        println!("Collision with another entity");
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(crate) fn find_enemy<'a>(&'a mut self, agents: &[RefCell<Entity>]) {
        let best_agent = agents
            .iter()
            .filter_map(|a| a.try_borrow().ok())
            .filter(|a| {
                let aid = a.get_id();
                let ateam = a.get_team();
                !self.unreachables.contains(&aid) && aid != self.id && ateam != self.team
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

        if let Some((_dist, agent)) = best_agent {
            self.target = Some(agent.get_id());
        }
    }

    pub fn shoot_bullet(&mut self, bullets: &mut Vec<Bullet>, target_pos: [f64; 2]) -> bool {
        if 0. < self.cooldown {
            return false;
        }
        let dir = Vector2::new(self.orient.cos(), self.orient.sin());
        if dir.dot((Vector2::from(target_pos) - Vector2::from(self.pos)).normalize()) < 0.5 {
            return false;
        }
        let bullet = Bullet {
            pos: self.pos,
            velo: (dir * BULLET_SPEED).into(),
            team: self.team,
            traveled: 0.,
        };

        bullets.push(bullet);

        self.cooldown += 50.;
        true
    }

    fn do_avoidance(
        &mut self,
        game: &Game,
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
        let (res, time) =
            measure_time(|| self.avoidance_search(game, entities, |_, _| (), cmd.back, false));
        // println!("Avoidance goal set to {:?}, returns {res:?}", self.goal);
        if let Ok(mut time_window) = TIME_WINDOW.lock() {
            time_window.push_back(time);
            while 10 < time_window.len() {
                time_window.pop_front();
            }
            let count = AVG_COUNT.fetch_add(1, Ordering::Relaxed);
            if count % 10 == 0 && !time_window.is_empty() {
                let avg = time_window.iter().sum::<f64>() / time_window.len() as f64;
                println!("Avoidance search ({count}): {avg:.06}s");
            }
        }
        Box::new(res)
    }

    pub fn update<'a, 'b>(
        &'a mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
        bullets: &mut Vec<Bullet>,
    ) {
        if let Some(mut tree) = self.behavior_tree.take() {
            enum Command {
                Drive(DriveCommand),
                MoveTo(MoveToCommand),
                FollowPath(FollowPathCommand),
            }
            let mut command = None;
            let mut ctx = Context::new(std::mem::take(&mut self.blackboard));
            ctx.set("target", self.target);
            ctx.set("has_path", !self.path.is_empty());
            let mut process = |f: &dyn std::any::Any| {
                if let Some(com) = f.downcast_ref::<DriveCommand>() {
                    command = Some(Command::Drive(*com));
                    return MotionResult::as_drive(&self.last_motion_result);
                } else if let Some(com) = f.downcast_ref::<MoveToCommand>() {
                    command = Some(Command::MoveTo(*com));
                    return MotionResult::as_move_to(&self.last_motion_result);
                } else if f.downcast_ref::<FindEnemyCommand>().is_some() {
                    // println!("FindEnemy process");
                    self.find_enemy(entities);
                } else if f.downcast_ref::<FindPathCommand>().is_some() {
                    if let Some(target) = self.target.and_then(|target| {
                        entities.iter().find(|a| {
                            a.try_borrow()
                                .map(|a| a.get_id() == target)
                                .unwrap_or(false)
                        })
                    }) {
                        let target = target.borrow_mut();
                        let found_path = self.find_path(Some(&target), game).is_ok();
                        return Some(Box::new(found_path) as Box<dyn std::any::Any>);
                    }
                } else if let Some(cmd) = f.downcast_ref::<FollowPathCommand>() {
                    command = Some(Command::FollowPath(*cmd));
                    return MotionResult::as_follow_path(&self.last_motion_result);
                } else if f.downcast_ref::<ShootCommand>().is_some() {
                    let forward = Vector2::new(self.orient.cos(), self.orient.sin());
                    self.shoot_bullet(bullets, (Vector2::from(self.pos) + forward).into());
                } else if let Some(goal) = f.downcast_ref::<AvoidanceCommand>() {
                    return Some(self.do_avoidance(game, entities, goal));
                } else if f.downcast_ref::<ClearAvoidanceCommand>().is_some() {
                    self.search_state = None;
                } else if f.downcast_ref::<GetPathNextNodeCommand>().is_some() {
                    if let Some(path) = self.path.last() {
                        return Some(Box::new(*path));
                    }
                } else if f.downcast_ref::<GetStateCommand>().is_some() {
                    return Some(Box::new(self.to_state()));
                } else if let Some(com) = f.downcast_ref::<IsTargetVisibleCommand>() {
                    if let Some(target) = entities.get(com.0).and_then(|e| e.try_borrow().ok()) {
                        let target_pos = target.get_pos();
                        let target_triangle = find_triangle_at(
                            &game.mesh,
                            target_pos,
                            &mut *game.triangle_profiler.borrow_mut(),
                        );
                        let self_triangle = find_triangle_at(
                            &game.mesh,
                            self.pos,
                            &mut *game.triangle_profiler.borrow_mut(),
                        );
                        if target_triangle == self_triangle {
                            return Some(Box::new(true));
                        }
                        return Some(Box::new(self.is_position_visible(
                            target_pos,
                            &game.mesh,
                            &mut *game.triangle_profiler.borrow_mut(),
                        )));
                    }
                } else if let Some(com) = f.downcast_ref::<FaceToTargetCommand>() {
                    if let Some(target) = entities.get(com.0).and_then(|e| e.try_borrow().ok()) {
                        let target_pos = target.get_pos();
                        let res = self.orient_to(target_pos, false, entities);
                        return Some(Box::new(res));
                    }
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

            match command {
                Some(Command::Drive(com)) => {
                    let res = self.drive(com.0, game, entities);
                    self.last_motion_result = Some(MotionResult::Drive(res));
                }
                Some(Command::MoveTo(com)) => {
                    let res = self.move_to(game, com.0, false, entities);
                    self.last_motion_result = Some(MotionResult::MoveTo(res));
                }
                Some(Command::FollowPath(_com)) => {
                    let res = self.follow_path(game, entities);
                    self.last_motion_result = Some(MotionResult::FollowPath(res));
                }
                _ => self.last_motion_result = None,
            }
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
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum FollowPathResult {
    Following,
    Blocked,
    Arrived,
}

impl From<bool> for FollowPathResult {
    fn from(b: bool) -> Self {
        if b {
            FollowPathResult::Following
        } else {
            FollowPathResult::Blocked
        }
    }
}

impl From<FollowPathResult> for bool {
    fn from(b: FollowPathResult) -> bool {
        matches!(b, FollowPathResult::Following)
    }
}

impl Agent {
    fn follow_path(&mut self, game: &mut Game, entities: &[RefCell<Entity>]) -> FollowPathResult {
        if self.follow_avoidance_path(game, entities) {
            FollowPathResult::Following
        } else if let Some(target) = self.path.last() {
            if 5. < Vector2::from(*target).distance(Vector2::from(self.pos)) {
                let target_pos = *target;
                self.move_to(game, target_pos, false, entities).into()
            } else {
                self.path.pop();
                FollowPathResult::Following
            }
        } else {
            FollowPathResult::Arrived
        }
    }

    fn is_position_visible(&self, target: [f64; 2], mesh: &Mesh, profiler: &mut Profiler) -> bool {
        const INTERPOLATE_INTERVAL: f64 = AGENT_HALFLENGTH;

        let self_pos = self.pos;

        let distance = Vector2::from(self_pos).distance(Vector2::from(target));
        if AGENT_VISIBLE_DISTANCE < distance {
            return false;
        }
        interpolation::interpolate(self_pos, target, INTERPOLATE_INTERVAL, |point| {
            find_triangle_at(mesh, point, profiler).is_some()
        })
    }
}

/// Wrap the angle value in [-pi, pi)
fn wrap_angle(x: f64) -> f64 {
    use std::f64::consts::PI;
    const TWOPI: f64 = PI * 2.;
    // ((x + PI) - ((x + PI) / TWOPI).floor() * TWOPI) - PI
    x - (x + PI).div_euclid(TWOPI) * TWOPI
}
