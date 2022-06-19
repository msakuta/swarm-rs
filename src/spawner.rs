use crate::{
    app_data::AppData,
    entity::{Entity, GameEvent},
};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Debug)]
pub(crate) struct Spawner {
    pub id: usize,
    pub pos: [f64; 2],
    pub team: usize,
    pub active: bool,
    pub health: usize,
}

impl Spawner {
    pub(crate) fn new(id_gen: &mut usize, pos: [f64; 2], team: usize) -> Self {
        let id = *id_gen;
        *id_gen += 1;
        Spawner {
            id,
            pos,
            team,
            active: true,
            health: 5,
        }
    }

    pub(crate) fn update(
        &mut self,
        app_data: &mut AppData,
        entities: &[RefCell<Entity>],
    ) -> Vec<GameEvent> {
        let mut ret = vec![];
        let rng = Rc::make_mut(&mut app_data.rng);
        if entities
            .iter()
            .filter(|entity| {
                entity
                    .try_borrow()
                    .map(|entity| entity.is_agent() && entity.get_team() == self.team)
                    .unwrap_or(false)
            })
            .count()
            < 5
            && rng.next() < 0.1
        {
            ret.push(GameEvent::SpawnAgent {
                pos: self.pos,
                team: self.team,
            })
        }
        ret
    }
}
