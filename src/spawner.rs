use crate::{
    entity::{Entity, GameEvent},
    game::Game,
};
use std::cell::RefCell;

const SPAWNER_MAX_HEALTH: u32 = 10;

#[derive(Clone, Debug)]
pub(crate) struct Spawner {
    pub id: usize,
    pub pos: [f64; 2],
    pub team: usize,
    pub active: bool,
    pub health: u32,
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
            health: SPAWNER_MAX_HEALTH,
        }
    }

    pub(crate) fn get_health_rate(&self) -> f64 {
        self.health as f64 / SPAWNER_MAX_HEALTH as f64
    }

    pub(crate) fn update(
        &mut self,
        game: &mut Game,
        entities: &[RefCell<Entity>],
    ) -> Vec<GameEvent> {
        let mut ret = vec![];
        let rng = &mut game.rng;
        if entities
            .iter()
            .filter(|entity| {
                entity
                    .try_borrow()
                    .map(|entity| entity.is_agent() && entity.get_team() == self.team)
                    .unwrap_or(false)
            })
            .count()
            < 3
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
