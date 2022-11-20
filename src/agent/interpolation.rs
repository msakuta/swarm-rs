use cgmath::{MetricSpace, Vector2};

use super::{Agent, State};

fn lerp(a: &[f64; 2], b: &[f64; 2], f: f64) -> [f64; 2] {
    [a[0] * (1. - f) + b[0] * f, a[1] * (1. - f) + a[1] * f]
}

/// Collision checking with linear interpolation. A closure to check the collision must be provided.
pub(crate) fn interpolate(
    start: [f64; 2],
    target: [f64; 2],
    interval: f64,
    mut f: impl FnMut([f64; 2]) -> bool,
) -> bool {
    let distance = Vector2::from(start).distance(Vector2::from(target));
    let interpolates = (distance.abs() / interval).floor() as usize;
    for i in 0..interpolates {
        let point = lerp(&start, &target, i as f64 * interval);
        if f(point) {
            return true;
        }
    }
    return false;
}

/// Collision checking with steering model. It can interpolate curvature.
pub(crate) fn interpolate_steer(
    start: &State,
    steer: f64,
    distance: f64,
    interval: f64,
    f: impl Fn(State) -> bool,
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
        if f(next) {
            return true;
        }
    }
    return false;
}
