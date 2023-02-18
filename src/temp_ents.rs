pub(crate) const MAX_TTL: f64 = 20.;

#[derive(Debug, Clone)]
pub struct TempEnt {
    pub pos: [f64; 2],
    pub ttl: f64,
    pub max_ttl: f64,
    pub max_radius: f64,
}

impl TempEnt {
    pub fn new(pos: [f64; 2], ttl: f64, radius: f64) -> Self {
        Self {
            pos,
            ttl,
            max_ttl: ttl,
            max_radius: radius,
        }
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
