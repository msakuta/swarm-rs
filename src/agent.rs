mod avoidance;
mod behavior_nodes;
mod find_path;

pub(crate) use self::avoidance::{SearchState, State};
use self::{
    avoidance::DIST_RADIUS,
    behavior_nodes::{
        build_tree, AvoidanceCommand, BehaviorTree, FaceToTargetCommand, FindEnemyCommand,
        FindPathCommand, FollowPathCommand, GetPathNextNodeCommand, GetStateCommand,
        IsTargetVisibleCommand, MoveCommand, ShootCommand,
    },
};
use crate::{
    entity::Entity,
    game::{Game, Profiler},
    triangle_utils::find_triangle_at,
};
use ::behavior_tree_lite::Context;
use ::cgmath::{InnerSpace, MetricSpace, Vector2};
use behavior_tree_lite::Blackboard;
use delaunator::{Point, Triangulation};
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
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
    pub team: usize,
    cooldown: f64,
    pub health: u32,
    pub goal: Option<State>,
    pub search_state: Option<SearchState>,
    /// Avoidance path is more local.
    pub avoidance_path: Vec<[f64; 2]>,
    pub path: Vec<[f64; 2]>,
    pub trace: VecDeque<[f64; 2]>,
    behavior_tree: Option<BehaviorTree>,
    blackboard: Blackboard,
}

pub(crate) const AGENT_HALFWIDTH: f64 = 0.3;
pub(crate) const AGENT_HALFLENGTH: f64 = 0.6;
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
    ) -> Self {
        let id = *id_gen;
        *id_gen += 1;

        Self {
            target: None,
            active: true,
            unreachables: HashSet::new(),
            id,
            pos,
            orient,
            team,
            cooldown: 5.,
            health: AGENT_MAX_HEALTH,
            goal: None,
            search_state: None,
            avoidance_path: vec![],
            path: vec![],
            trace: VecDeque::new(),
            behavior_tree: Some(build_tree(behavior_source)),
            blackboard: Blackboard::new(),
        }
    }

    pub(crate) fn get_health_rate(&self) -> f64 {
        self.health as f64 / AGENT_MAX_HEALTH as f64
    }

    fn orient_to(&mut self, target: [f64; 2]) -> bool {
        use std::f64::consts::PI;
        const TWOPI: f64 = PI * 2.;
        const ANGLE_SPEED: f64 = PI / 50.;
        let delta = Vector2::from(target) - Vector2::from(self.pos);
        let target_angle = delta.y.atan2(delta.x);
        let delta_angle = target_angle - self.orient;
        let wrap_angle = ((delta_angle + PI) - ((delta_angle + PI) / TWOPI).floor() * TWOPI) - PI;
        if wrap_angle.abs() < ANGLE_SPEED {
            self.orient = target_angle;
            true
        } else if wrap_angle < 0. {
            self.orient = (self.orient - ANGLE_SPEED) % TWOPI;
            wrap_angle.abs() < PI / 4.
        } else {
            self.orient = (self.orient + ANGLE_SPEED) % TWOPI;
            wrap_angle.abs() < PI / 4.
        }
    }

    pub(crate) fn drive(
        &mut self,
        drive: f64,
        game: &mut Game,
        others: &[RefCell<Entity>],
    ) -> bool {
        let forward = Vector2::new(self.orient.cos(), self.orient.sin());
        let target_pos =
            (Vector2::from(self.pos) + drive.min(AGENT_SPEED).max(-AGENT_SPEED) * forward).into();

        if Self::collision_check(Some(self.id), target_pos, others) {
            return false;
        }

        if let Some(next_triangle) = find_triangle_at(
            &game.triangulation,
            &game.points,
            target_pos,
            &mut *game.triangle_profiler.borrow_mut(),
        ) {
            if game.triangle_passable[next_triangle] {
                if 100 < self.trace.len() {
                    self.trace.pop_front();
                }
                self.trace.push_back(self.pos);
                self.pos = target_pos;
            }
        }
        true
    }

    pub(crate) fn move_to<'a>(
        &'a mut self,
        game: &mut Game,
        target_pos: [f64; 2],
        others: &[RefCell<Entity>],
    ) -> bool {
        if self.orient_to(target_pos) {
            let delta = Vector2::from(target_pos) - Vector2::from(self.pos);
            let distance = delta.magnitude();

            self.drive(distance, game, others)
        } else {
            true
        }
    }

    pub(crate) fn collision_check(
        ignore: Option<usize>,
        newpos: [f64; 2],
        others: &[RefCell<Entity>],
    ) -> bool {
        for entity in others.iter() {
            if let Ok(entity) = entity.try_borrow() {
                if Some(entity.get_id()) == ignore {
                    continue;
                }
                let dist2 = Vector2::from(entity.get_pos()).distance2(Vector2::from(newpos));
                if dist2 < (AGENT_HALFLENGTH * 2.).powf(2.) {
                    // Collision with another entity
                    return true;
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

    pub fn update<'a, 'b>(
        &'a mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
        bullets: &mut Vec<Bullet>,
    ) {
        if let Some(mut tree) = self.behavior_tree.take() {
            let mut ctx = Context::new(std::mem::take(&mut self.blackboard));
            ctx.set("target", self.target);
            ctx.set("has_path", !self.path.is_empty());
            let mut process = |f: &dyn std::any::Any| {
                if let Some(com) = f.downcast_ref::<MoveCommand>() {
                    let drive = match &com.0 as &str {
                        "forward" => 1.,
                        "backward" => -1.,
                        _ => return None,
                    };
                    let drive_result = self.drive(drive, game, entities);
                    return Some(Box::new(drive_result) as Box<dyn std::any::Any>);
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
                        return Some(Box::new(found_path));
                    }
                } else if f.downcast_ref::<FollowPathCommand>().is_some() {
                    let ret = self.follow_path(game, entities);
                    return Some(Box::new(ret) as Box<dyn std::any::Any>);
                } else if f.downcast_ref::<ShootCommand>().is_some() {
                    let forward = Vector2::new(self.orient.cos(), self.orient.sin());
                    self.shoot_bullet(bullets, (Vector2::from(self.pos) + forward).into());
                } else if let Some(goal) = f.downcast_ref::<AvoidanceCommand>() {
                    self.goal = Some(avoidance::State::new(goal.0[0], goal.0[1], self.orient));
                    return Some(Box::new(self.search(1, game, entities, |_, _| (), false))
                        as Box<dyn std::any::Any>);
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
                            &game.triangulation,
                            &game.points,
                            target_pos,
                            &mut *game.triangle_profiler.borrow_mut(),
                        );
                        let self_triangle = find_triangle_at(
                            &game.triangulation,
                            &game.points,
                            self.pos,
                            &mut *game.triangle_profiler.borrow_mut(),
                        );
                        if target_triangle == self_triangle {
                            return Some(Box::new(true));
                        }
                        return Some(Box::new(self.is_position_visible(
                            target_pos,
                            &game.triangulation,
                            &game.points,
                            &mut *game.triangle_profiler.borrow_mut(),
                        )));
                    }
                } else if let Some(com) = f.downcast_ref::<FaceToTargetCommand>() {
                    if let Some(target) = entities.get(com.0).and_then(|e| e.try_borrow().ok()) {
                        let target_pos = target.get_pos();
                        let res = self.orient_to(target_pos);
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

    fn follow_path(&mut self, game: &mut Game, entities: &[RefCell<Entity>]) -> bool {
        if let Some(target) = self.avoidance_path.last() {
            if DIST_RADIUS < Vector2::from(*target).distance(Vector2::from(self.pos)) {
                let target_pos = *target;
                self.move_to(game, target_pos, entities)
            } else {
                self.avoidance_path.pop();
                true
            }
        } else if let Some(target) = self.path.last() {
            if 5. < Vector2::from(*target).distance(Vector2::from(self.pos)) {
                let target_pos = *target;
                self.move_to(game, target_pos, entities)
            } else {
                self.path.pop();
                true
            }
        } else {
            false
        }
    }

    fn is_position_visible(
        &self,
        target: [f64; 2],
        triangulation: &Triangulation,
        points: &[Point],
        profiler: &mut Profiler,
    ) -> bool {
        fn lerp(a: &[f64; 2], b: &[f64; 2], f: f64) -> [f64; 2] {
            [a[0] * (1. - f) + b[0] * f, a[1] * (1. - f) + a[1] * f]
        }
        fn interpolate(
            start: [f64; 2],
            target: [f64; 2],
            mut f: impl FnMut([f64; 2]) -> bool,
        ) -> bool {
            const INTERPOLATE_INTERVAL: f64 = AGENT_HALFLENGTH;
            let distance = Vector2::from(start).distance(Vector2::from(target));
            let interpolates = (distance.abs() / INTERPOLATE_INTERVAL).floor() as usize;
            for i in 0..interpolates {
                let point = lerp(&start, &target, i as f64 * INTERPOLATE_INTERVAL);
                if f(point) {
                    return true;
                }
            }
            return false;
        }

        let self_pos = self.pos;

        let distance = Vector2::from(self_pos).distance(Vector2::from(target));
        if AGENT_VISIBLE_DISTANCE < distance {
            return false;
        }
        interpolate(self_pos, target, |point| {
            find_triangle_at(triangulation, points, point, profiler).is_some()
        })
    }
}
