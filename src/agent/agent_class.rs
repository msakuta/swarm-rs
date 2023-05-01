use std::fmt::Display;

use super::{
    AGENT_HALFLENGTH, AGENT_HALFWIDTH, AGENT_MAX_HEALTH, AGENT_SPEED, BULLET_DAMAGE, BULLET_SPEED,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentClass {
    Worker,
    Fighter,
}

impl Display for AgentClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Worker => "Worker",
                Self::Fighter => "Fighter",
            }
        )
    }
}

impl AgentClass {
    pub(crate) fn cost(&self) -> i32 {
        match self {
            Self::Worker => 100,
            Self::Fighter => 500,
        }
    }

    pub(crate) fn time(&self) -> usize {
        match self {
            Self::Worker => 200,
            Self::Fighter => 1500,
        }
    }

    pub(crate) fn health(&self) -> u32 {
        match self {
            Self::Worker => AGENT_MAX_HEALTH,
            Self::Fighter => AGENT_MAX_HEALTH * 3,
        }
    }

    pub(crate) fn damage(&self) -> u32 {
        match self {
            Self::Worker => BULLET_DAMAGE,
            Self::Fighter => BULLET_DAMAGE * 10,
        }
    }

    pub(crate) fn bullet_speed(&self) -> f64 {
        match self {
            Self::Worker => BULLET_SPEED * 0.7,
            Self::Fighter => BULLET_SPEED,
        }
    }

    pub(crate) fn cooldown(&self) -> f64 {
        match self {
            Self::Worker => 20.,
            Self::Fighter => 50.,
        }
    }

    pub(crate) fn speed(&self) -> f64 {
        match self {
            Self::Worker => AGENT_SPEED,
            Self::Fighter => AGENT_SPEED * 0.7,
        }
    }

    pub(crate) fn shape(&self) -> (f64, f64) {
        match self {
            Self::Worker => (AGENT_HALFLENGTH, AGENT_HALFWIDTH),
            Self::Fighter => (AGENT_HALFLENGTH * 1.5, AGENT_HALFWIDTH * 1.5),
        }
    }

    pub fn vertices(&self, mut f: impl FnMut([f64; 2])) {
        let (length, width) = self.shape();
        // Technically, we could return a reference to static array, since there are only 2 kinds of shapes.
        // However, returning the shape by a callback has an advantage that it can return dynamic shape
        // without heap allocation.
        if matches!(self, AgentClass::Fighter) {
            for v in [
                [-length, -width],
                [length, -width],
                [length, -width * 0.5],
                [length * 0.8, -width * 0.5],
                [length * 0.8, width * 0.5],
                [length, width * 0.5],
                [length, width],
                [-length, width],
                [-length, width * 0.5],
                [-length * 0.9, width * 0.5],
                [-length * 0.9, -width * 0.5],
                [-length, -width * 0.5],
            ] {
                f(v)
            }
        } else {
            for v in [
                [-length, -width],
                [length, -width],
                [length, width],
                [-length, width],
            ] {
                f(v)
            }
        }
    }
}
