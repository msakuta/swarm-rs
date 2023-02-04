use super::{Agent, AgentTarget, AGENT_HALFLENGTH};
use crate::{game::Game, measure_time};

impl Agent {
    pub fn find_path(&mut self, target: [f64; 2], game: &mut Game) -> bool {
        let ((found_path, search_tree), _time) = measure_time(|| {
            let qtree = &game.qtree;
            if let Some(AgentTarget::Entity(tgt_id)) = self.target {
                qtree.path_find(&[self.id, tgt_id], self.pos, target, AGENT_HALFLENGTH * 1.5)
            } else {
                qtree.path_find(&[self.id], self.pos, target, AGENT_HALFLENGTH * 1.5)
            }
        });
        // println!("Agent::find_path: {:.03} ms", time * 1e3);
        self.search_tree = Some(search_tree);
        if let Some(path) = found_path {
            self.path = path;
            true
        } else {
            false
        }
    }
}
