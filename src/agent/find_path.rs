use super::Agent;
use crate::{game::Game, measure_time};

impl Agent {
    pub fn find_path(&mut self, target: [f64; 2], game: &mut Game) -> Result<(), ()> {
        let ((found_path, search_tree), time) = measure_time(|| {
            let qtree = game.qtree.borrow();
            if let Some(tgt_id) = self.target {
                qtree.path_find(&[self.id, tgt_id], self.pos, target)
            } else {
                qtree.path_find(&[self.id], self.pos, target)
            }
        });
        println!("Agent::find_path: {:.03} ms", time * 1e3);
        self.search_tree = Some(search_tree);
        if let Some(path) = found_path {
            self.path = path
        }
        Ok(())
    }
}
