use super::{Agent, AgentTarget, AGENT_HALFLENGTH};
use crate::{
    game::Game,
    measure_time,
    qtree::{qtree::PathFindError, QTreePathNode},
};

impl Agent {
    pub(crate) fn find_path(
        &mut self,
        target: [f64; 2],
        game: &mut Game,
    ) -> Result<Vec<QTreePathNode>, PathFindError> {
        let ((found_path, search_tree), time) = measure_time(|| {
            let qtree = &game.qtree;
            if let Some(AgentTarget::Entity(tgt_id)) = self.target {
                qtree.path_find(&[self.id, tgt_id], self.pos, target, AGENT_HALFLENGTH * 1.5)
            } else {
                qtree.path_find(&[self.id], self.pos, target, AGENT_HALFLENGTH * 1.5)
            }
        });
        game.path_find_profiler.get_mut().add(time);
        self.search_tree = Some(search_tree);
        match found_path {
            Ok(path) => {
                self.path = path.clone();
                Ok(path)
            }
            Err(err) => Err(err),
        }
    }
}
