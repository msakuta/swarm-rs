pub(crate) struct Agent {
    target: Option<usize>,
    active: bool,
    // path: Path,
    // unreachables = {};
    // behaviorTree = new BT.BehaviorTree();
    id: usize,
    pub pos: [f64; 2],
    team: usize,
    cooldown: f64,
}

impl Agent {
    pub(crate) fn new(id_gen: &mut usize, pos: [f64; 2], team: usize) -> Self {
        let id = *id_gen;
        *id_gen += 1;
        Self {
            target: None,
            active: true,
            id,
            pos,
            team,
            cooldown: 5.,
        }
    }
}
