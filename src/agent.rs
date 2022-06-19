use ::cgmath::{MetricSpace, Vector2};
use std::collections::HashSet;

#[derive(Clone, Debug)]
pub(crate) struct Agent {
    pub target: Option<usize>,
    active: bool,
    // path: Path,
    unreachables: HashSet<usize>,
    // behaviorTree = new BT.BehaviorTree();
    id: usize,
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
}
