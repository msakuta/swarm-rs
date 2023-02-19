use std::cell::Cell;

use cgmath::{MetricSpace, Vector2};

use super::{behavior_nodes::FindPathCommand, Agent, AgentTarget, AGENT_HALFLENGTH};
use crate::{
    game::Game,
    measure_time,
    qtree::{qtree::PathFindError, QTreePathNode},
    CellState,
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
            Ok(mut path) => {
                self.shortcut_path(&mut path, game);
                self.path = path.clone();
                Ok(path)
            }
            Err(err) => Err(err),
        }
    }

    /// Shortcut last few nodes if it's still visible. It won't attempt to shortcut the whole path
    /// since line-of-sight check can be expensive.
    fn shortcut_path(&mut self, path: &mut Vec<QTreePathNode>, game: &Game) {
        const MAX_SHORTCUT_DISTANCE: f64 = 10.;
        let current = self.pos;
        let ignore_id = self.id;
        let before_num = path.len();
        let checks = Cell::new(0);
        for _ in 0..5 {
            if path.len() < 3 {
                break;
            }
            let second_last = path[path.len() - 2].pos;

            // Don't attempt to shortcut too far away
            if MAX_SHORTCUT_DISTANCE.powf(2.)
                < Vector2::from(second_last).distance2(Vector2::from(current))
            {
                break;
            }
            let ret = game.qtree.get_cache_map().is_position_visible(
                |cell_state| {
                    checks.set(checks.get() + 1);
                    match cell_state {
                        CellState::Obstacle => true,
                        CellState::Occupied(id) => id != ignore_id,
                        CellState::Free => false,
                        _ => false,
                    }
                },
                current,
                second_last,
            );
            if ret {
                path.pop();
            } else {
                break;
            }
        }
        let after_num = path.len();
        if before_num != after_num {
            // self.log(format!(
            //     "Path shortcut from {before_num} to {after_num} checks: {}",
            //     checks.get()
            // ));
        }
    }
}
