use cgmath::{MetricSpace, Vector2};

use super::{Agent, AgentState};

pub fn lerp(a: &[f64; 2], b: &[f64; 2], f: f64) -> [f64; 2] {
    [a[0] * (1. - f) + b[0] * f, a[1] * (1. - f) + b[1] * f]
}

pub(crate) trait AsPoint {
    fn as_point(&self) -> [f64; 2];
}

/// Linearly interpolatable point-like object.
///
/// This trait is not object safe, thus made a separate trait from [`AsPoint`].
pub(crate) trait LerpPoint: AsPoint {
    fn lerp(&self, other: &Self, f: f64) -> Self;
}

impl AsPoint for [f64; 2] {
    fn as_point(&self) -> [f64; 2] {
        *self
    }
}

impl LerpPoint for [f64; 2] {
    fn lerp(&self, other: &Self, f: f64) -> Self {
        lerp(self, other, f)
    }
}

impl AsPoint for AgentState {
    fn as_point(&self) -> [f64; 2] {
        [self.x, self.y]
    }
}

impl LerpPoint for AgentState {
    fn lerp(&self, other: &Self, f: f64) -> Self {
        let p = lerp(&self.as_point(), &other.as_point(), f);
        Self {
            x: p[0],
            y: p[1],
            heading: self.heading,
        }
    }
}

/// Collision checking with linear interpolation. A closure to check the collision must be provided.
/// The closure shall return true if it has collision, and will be called multiple times to interpolate the range.
/// The function will return early if the closure returns true.
pub(crate) fn interpolate<P: LerpPoint>(
    start: P,
    target: P,
    interval: f64,
    mut f: impl FnMut(P) -> bool,
) -> bool {
    let start_p = start.as_point();
    let target_p = target.as_point();
    let distance = Vector2::from(start_p).distance(Vector2::from(target_p));
    let interpolates = (distance.abs() / interval).floor() as usize + 1;
    for i in 0..=interpolates {
        let point = start.lerp(&target, i as f64 / interpolates as f64);
        if f(point) {
            return true;
        }
    }
    return false;
}

fn lerp_i(a: Vector2<i32>, b: Vector2<i32>, f: f64) -> Vector2<i32> {
    Vector2::new(
        (a.x as f64 * (1. - f) + b.x as f64 * f + 0.5) as i32,
        (a.y as f64 * (1. - f) + b.y as f64 * f + 0.5) as i32,
    )
}

/// Integer interpolation. Interval is deterimned by Chebyshev distance, not Euclidean.
pub(crate) fn interpolate_i<P: Into<Vector2<i32>>>(
    start: P,
    target: P,
    mut f: impl FnMut(Vector2<i32>) -> bool,
) -> bool {
    let start_p = start.into();
    let target_p = target.into();
    let interpolates = (start_p.x - target_p.x)
        .abs()
        .max((start_p.y - target_p.y).abs());
    if interpolates == 0 {
        return f(start_p);
    }
    for i in 0..=interpolates {
        let point = lerp_i(start_p, target_p, i as f64 / interpolates as f64);
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
    f: impl Fn(AgentState) -> bool,
) -> bool {
    let interpolates = (distance.abs() / interval).floor() as usize;
    for i in 0..interpolates {
        let sign = if distance < 0. { -1. } else { 1. };
        let next = Agent::step_move(
            start.x,
            start.y,
            start.heading,
            steer,
            sign * i as f64 * interval,
        );
        if f(next) {
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
