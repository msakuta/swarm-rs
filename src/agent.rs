use crate::shape::{Idx, Shape};
use ::cgmath::{InnerSpace, MetricSpace, Vector2};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub(crate) struct Bullet {
    pub pos: [f64; 2],
    pub velo: [f64; 2],
    pub team: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Agent {
    pub target: Option<usize>,
    active: bool,
    // path: Path,
    unreachables: HashSet<usize>,
    // behaviorTree = new BT.BehaviorTree();
    pub id: usize,
    pub pos: [f64; 2],
    pub team: usize,
    cooldown: f64,
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

    pub(crate) fn find_enemy<'a>(&'a mut self, agents: impl Iterator<Item = &'a Agent>) {
        let mut best_agent = None;
        let mut best_distance = 1e6;
        for a in agents {
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

        self.cooldown += 5.;
        true
    }

    pub fn update(&mut self) {
        self.cooldown = (self.cooldown - 1.).max(0.);
    }
}
