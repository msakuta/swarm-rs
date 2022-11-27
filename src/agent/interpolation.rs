use cgmath::{MetricSpace, Vector2};

use super::{Agent, AgentState};

pub(crate) fn lerp(a: &[f64; 2], b: &[f64; 2], f: f64) -> [f64; 2] {
    [a[0] * (1. - f) + b[0] * f, a[1] * (1. - f) + b[1] * f]
}

pub(crate) trait AsPoint {
    fn as_point(&self) -> [f64; 2];
}

impl AsPoint for [f64; 2] {
    fn as_point(&self) -> [f64; 2] {
        *self
    }
}

impl AsPoint for AgentState {
    fn as_point(&self) -> [f64; 2] {
        [self.x, self.y]
    }
}

/// Collision checking with linear interpolation. A closure to check the collision must be provided.
pub(crate) fn interpolate<P: AsPoint>(
    start: P,
    target: P,
    interval: f64,
    mut f: impl FnMut([f64; 2]) -> bool,
) -> bool {
    let start = start.as_point();
    let target = target.as_point();
    let distance = Vector2::from(start).distance(Vector2::from(target));
    let interpolates = (distance.abs() / interval).floor() as usize + 1;
    for i in 0..=interpolates {
        let point = lerp(&start.as_point(), &target, i as f64 / interpolates as f64);
        if f(point) {
            return true;
        }
    }
    return false;
}

/// Collision checking with steering model. It can interpolate curvature.
pub(crate) fn interpolate_steer(
    start: &AgentState,
    steer: f64,
    distance: f64,
    interval: f64,
    f: impl Fn([f64; 2]) -> bool,
) -> bool {
    let interpolates = (distance.abs() / interval).floor() as usize;
    for i in 0..interpolates {
        let sign = if distance < 0. { -1. } else { 1. };
        let next = Agent::step_move(
            start.x,
            start.y,
            start.heading,
            steer,
            1.,
            sign * i as f64 * interval,
        );
        if f([next.x, next.y]) {
            return true;
        }
    }
    return false;
}

#[test]
fn test_lerp() {
    let a = [1., 10.];
    let b = [3., 30.];
    assert_eq!(lerp(&a, &b, 0.5), [2., 20.]);
}
