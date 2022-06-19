mod find_path;

use crate::shape::{Idx, Shape};
use ::cgmath::{InnerSpace, MetricSpace, Vector2};
use ::delaunator::{Point, Triangulation};
use std::{cell::RefCell, collections::HashSet};

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
    unreachables: HashSet<usize>,
    // behaviorTree = new BT.BehaviorTree();
    pub id: usize,
    pub pos: [f64; 2],
    pub team: usize,
    cooldown: f64,
    pub path: Vec<[f64; 2]>,
}

impl Agent {
    pub(crate) fn new(id_gen: &mut usize, pos: [f64; 2], team: usize) -> Self {
        let id = *id_gen;
        *id_gen += 1;
        Self {
            target: None,
            active: true,
            unreachables: HashSet::new(),
            id,
            pos,
            team,
            cooldown: 5.,
            path: vec![],
        }
    }

    pub(crate) fn move_to<'a>(&'a mut self, board: &[bool], shape: Shape, target_pos: [f64; 2]) {
        const SPEED: f64 = 1.;
        let delta = Vector2::from(target_pos) - Vector2::from(self.pos);
        let distance = delta.magnitude();
        let newpos = if distance <= SPEED {
            target_pos
        } else {
            (Vector2::from(self.pos) + SPEED * delta / distance).into()
        };
        if board[shape.idx(newpos[0] as isize, newpos[1] as isize)] {
            self.pos = newpos;
        }
    }

    pub(crate) fn find_enemy<'a>(&'a mut self, agents: &[RefCell<Agent>]) {
        let mut best_agent = None;
        let mut best_distance = 1e6;
        for a in agents {
            if let Ok(a) = a.try_borrow() {
                if self.unreachables.contains(&a.id) {
                    continue;
                }
                if a.id != self.id && a.team != self.team {
                    let distance = Vector2::from(a.pos).distance(Vector2::from(self.pos));
                    if distance < best_distance {
                        best_agent = Some(a);
                        best_distance = distance;
                    }
                }
            }
        }

        if let Some(agent) = best_agent {
            self.target = Some(agent.id);
        }
    }

    pub fn shoot_bullet(&mut self, bullets: &mut Vec<Bullet>, target_pos: [f64; 2]) -> bool {
        if 0. < self.cooldown {
            return false;
        }
        let delta = Vector2::from(target_pos) - Vector2::from(self.pos);
        let distance = delta.magnitude();
        let bullet = Bullet {
            pos: self.pos,
            velo: (Vector2::from(delta) * 3. / distance).into(),
            team: self.team,
        };

        bullets.push(bullet);

        self.cooldown += 50.;
        true
    }

    pub fn update<'a, 'b>(
        &'a mut self,
        agents: &[RefCell<Agent>],
        triangulation: &Triangulation,
        points: &[Point],
        triangle_passable: &[bool],
        board: &[bool],
        shape: Shape,
        bullets: &mut Vec<Bullet>,
    ) {
        if let Some(target) = self.target.and_then(|target| {
            agents
                .iter()
                .find(|a| a.try_borrow().map(|a| a.id == target).unwrap_or(false))
        }) {
            let target = target.borrow_mut();
            if self
                .find_path(Some(&target), triangulation, points, triangle_passable)
                .is_ok()
            {
                if let Some(target) = self.path.last() {
                    let target_pos = *target;
                    self.move_to(board, shape, target_pos);
                }
            } else if 10. < Vector2::from(target.pos).distance(Vector2::from(self.pos)) {
                self.move_to(board, shape, target.pos);
            }
            self.shoot_bullet(bullets, target.pos);
        } else {
            self.path = vec![];
        }
        self.cooldown = (self.cooldown - 1.).max(0.);
    }
}
