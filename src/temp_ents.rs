pub(crate) const MAX_TTL: f64 = 20.;

#[derive(Debug, Clone)]
pub(crate) struct TempEnt {
    pub pos: [f64; 2],
    pub ttl: f64,
}

impl TempEnt {
    pub fn new(pos: [f64; 2]) -> Self {
        Self { pos, ttl: MAX_TTL }
    }

    pub fn update(&mut self) -> bool {
        if self.ttl < 1. {
            false
        } else {
            self.ttl -= 1.;
            true
        }
    }
}
