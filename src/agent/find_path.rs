use super::{behavior_nodes::FindPathCommand, Agent, AgentTarget, AGENT_HALFLENGTH};
use crate::{
    game::Game,
    measure_time,
    qtree::{qtree::PathFindError, QTreePathNode},
};

impl Agent {
    pub(super) fn find_path(
        &mut self,
        com: &FindPathCommand,
        game: &mut Game,
    ) -> Result<Vec<QTreePathNode>, PathFindError> {
        let ((found_path, search_tree), time) = measure_time(|| {
            let qtree = &game.qtree;
            fn ignore_id<'a>(ignore_ids: &'a [usize]) -> impl Fn(usize) -> bool + 'a {
                |id| ignore_ids.iter().any(|i| *i == id)
            }
            let target = com.target;
            if com.ignore_obstacles {
                qtree.path_find(|_| true, self.pos, target, AGENT_HALFLENGTH * 1.5)
            } else if let Some(AgentTarget::Entity(tgt_id)) = self.target {
                qtree.path_find(
                    ignore_id(&[self.id, tgt_id]),
                    self.pos,
                    target,
                    AGENT_HALFLENGTH * 1.5,
                )
            } else {
                qtree.path_find(
                    ignore_id(&[self.id]),
                    self.pos,
                    target,
                    AGENT_HALFLENGTH * 1.5,
                )
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
