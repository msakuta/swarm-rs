mod find_path;

use crate::{entity::Entity, game::Game, triangle_utils::find_triangle_at};
use ::cgmath::{InnerSpace, MetricSpace, Vector2};
use std::{
    cell::RefCell,
    collections::{HashSet, VecDeque},
};

#[derive(Clone, Debug)]
pub(crate) struct Bullet {
    pub pos: [f64; 2],
    pub velo: [f64; 2],
    pub team: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Agent {
    pub target: Option<usize>,
    pub active: bool,
    // path: Path,
    pub unreachables: HashSet<usize>,
    // behaviorTree = new BT.BehaviorTree();
    pub id: usize,
    pub pos: [f64; 2],
    pub orient: f64,
    pub team: usize,
    cooldown: f64,
    pub path: Vec<[f64; 2]>,
    pub trace: VecDeque<[f64; 2]>,
}

pub(crate) const AGENT_HALFWIDTH: f64 = 10.;
pub(crate) const AGENT_HALFLENGTH: f64 = 20.;

impl Agent {
    pub(crate) fn new(id_gen: &mut usize, pos: [f64; 2], orient: f64, team: usize) -> Self {
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
            path: vec![],
            trace: VecDeque::new(),
        }
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

    pub(crate) fn move_to<'a>(&'a mut self, game: &mut Game, target_pos: [f64; 2]) {
        const SPEED: f64 = 0.5;

        if self.orient_to(target_pos) {
            let delta = Vector2::from(target_pos) - Vector2::from(self.pos);
            let distance = delta.magnitude();
            let newpos = if distance <= SPEED {
                target_pos
            } else {
                let forward = Vector2::new(self.orient.cos(), self.orient.sin());
                (Vector2::from(self.pos) + SPEED * forward).into()
            };
            if let Some(next_triangle) = find_triangle_at(
                &game.triangulation,
                &game.points,
                target_pos,
                &mut game.triangle_profiler,
            ) {
                if game.triangle_passable[next_triangle] {
                    if 100 < self.trace.len() {
                        self.trace.pop_front();
                    }
                    self.trace.push_back(self.pos);
                    self.pos = newpos;
                }
            }
        }
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
        const BULLET_SPEED: f64 = 5.;
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
        if let Some(target) = self.target.and_then(|target| {
            entities.iter().find(|a| {
                a.try_borrow()
                    .map(|a| a.get_id() == target)
                    .unwrap_or(false)
            })
        }) {
            let target = target.borrow_mut();
            if 5. < Vector2::from(target.get_pos()).distance(Vector2::from(self.pos)) {
                if self.find_path(Some(&target), game).is_ok() {
                    if let Some(target) = self.path.last() {
                        let target_pos = *target;
                        self.move_to(game, target_pos);
                    }
                } else {
                    self.move_to(game, target.get_pos());
                }
            } else {
                // println!("Orienting {}", self.id);
                self.orient_to(target.get_pos());
            }
            self.shoot_bullet(bullets, target.get_pos());
        } else {
            self.path = vec![];
        }
        self.cooldown = (self.cooldown - 1.).max(0.);
    }
}
