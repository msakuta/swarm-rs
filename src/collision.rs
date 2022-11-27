//! Collision detection algorithms

use std::sync::atomic::{AtomicUsize, Ordering};

use behavior_tree_lite::Lazy;
use cgmath::{InnerSpace, Matrix2, Rad, Vector2};

use crate::entity::{BoundingCircle, CollisionShape, Obb};

static TOTAL_CALLS: AtomicUsize = AtomicUsize::new(0);
const MAX_RECURSES: usize = 3;
static RECURSE_COUNTS: Lazy<Vec<AtomicUsize>> = Lazy::new(|| {
    let mut ret = vec![];
    for _ in 0..MAX_RECURSES {
        ret.push(AtomicUsize::new(0));
    }
    ret
});
static TOTAL_INTERSECTION_CHECKS: AtomicUsize = AtomicUsize::new(0);

/// Binary search collision between 2 shapes in linear motion. Good for detecting
/// collisions between small, fast moving objects, but not so much for large objects like terrain walls.
///
/// This function returns if the collision happens in the first element of returned tuple,
/// and how many recursions it did to check in the second element.
pub(crate) fn bsearch_collision(
    obj1: &CollisionShape,
    obj1_bounding_circle: &BoundingCircle,
    obj1_velo: &Vector2<f64>,
    obj2: &CollisionShape,
    obj2_bounding_circle: &BoundingCircle,
    obj2_velo: &Vector2<f64>,
) -> (bool, usize) {
    // Assume obj2's stationary coordinate frame
    let rel_velo = obj1_velo - obj2_velo;

    let fetched = TOTAL_CALLS.fetch_add(1, Ordering::Relaxed);
    if fetched % 100 == 0 {
        println!(
            "bsearch_collision: Total calls: {fetched} recurses: {:?} checks: {TOTAL_INTERSECTION_CHECKS:?}",
            *RECURSE_COUNTS,
        );
    }

    // println!(
    //     "Circles: {:?} {:?}",
    //     obj1_bounding_circle.center, obj2_bounding_circle.center
    // );

    collision_internal(
        obj1,
        obj1_bounding_circle,
        rel_velo,
        obj2,
        obj2_bounding_circle,
        0,
    )
}

fn collision_internal(
    obj1: &CollisionShape,
    obj1_bounding_circle: &BoundingCircle,
    velo: Vector2<f64>,
    obj2: &CollisionShape,
    obj2_bounding_circle: &BoundingCircle,
    level: usize,
) -> (bool, usize) {
    // Potential collision radius
    let potential_radius = velo.magnitude() + obj1_bounding_circle.radius;
    let potential_center = velo / 2. + obj1_bounding_circle.center;

    // println!("collision recursing {level}");
    RECURSE_COUNTS
        .get(level)
        .map(|count| count.fetch_add(1, Ordering::Relaxed));

    // If the distance between the centers of the 2 objects is larger than the sum of the radii of bounding circles,
    // there will be no chance of collision.
    let dist2_centers = (potential_center - obj2_bounding_circle.center).magnitude2();
    if potential_radius.powf(2.) < dist2_centers {
        return (false, level);
    }

    let mut max_level = level;

    if level < MAX_RECURSES {
        // First half recusion
        let (hit, hitlevel) = collision_internal(
            obj1,
            obj1_bounding_circle,
            velo / 2.,
            obj2,
            obj2_bounding_circle,
            level + 1,
        );
        if hit {
            return (hit, hitlevel);
        }
        max_level = max_level.max(hitlevel);

        let (hit, hitlevel) = collision_internal(
            &obj1.translated(velo / 2.),
            &obj1_bounding_circle.translated(velo / 2.),
            velo / 2.,
            obj2,
            obj2_bounding_circle,
            level + 1,
        );
        if hit {
            return (hit, hitlevel);
        }
        max_level = max_level.max(hitlevel);
        (false, max_level)
    } else {
        TOTAL_INTERSECTION_CHECKS.fetch_add(1, Ordering::Relaxed);
        let obj1_copy = obj1.oriented(velo[1].atan2(velo[0]));
        (obj1_copy.intersects(obj2), max_level)
    }
}

impl BoundingCircle {
    fn translated(&self, offset: Vector2<f64>) -> Self {
        let mut copy = *self;
        copy.center += offset;
        copy
    }
}

impl CollisionShape {
    /// Return a translated copy.
    pub(crate) fn translated(&self, offset: Vector2<f64>) -> Self {
        match *self {
            Self::BBox(mut obb) => {
                obb.center += offset;
                Self::BBox(obb)
            }
        }
    }

    /// Return a copy with specified orientation
    pub(crate) fn oriented(&self, orient: f64) -> Self {
        match *self {
            Self::BBox(mut obb) => {
                obb.orient = orient;
                Self::BBox(obb)
            }
        }
    }

    pub(crate) fn intersects(&self, other: &Self) -> bool {
        self.intersects_inner(other) && other.intersects_inner(self)
    }

    fn intersects_inner(&self, other: &Self) -> bool {
        let Self::BBox(obb) = self;

        let rot_mat = Matrix2::from_angle(Rad(obb.orient));
        let x_normal = rot_mat * Vector2::new(1., 0.);
        let y_normal = rot_mat * Vector2::new(0., 1.);
        let org = obb.center;

        let Some(vertices) = other.to_vertices() else {
            println!("WARNING: OBB does not have vertices");
            return false;
        };
        let Some((x_min, x_max, y_min, y_max)) = vertices.into_iter().fold(None, |acc: Option<(f64, f64, f64, f64)>, vertex| {
            let rel_pos = Vector2::from(vertex) - org;
            let x_dot = rel_pos.dot(x_normal);
            let y_dot = rel_pos.dot(y_normal);
            if let Some((x_min, x_max, y_min, y_max)) = acc {
                Some((x_min.min(x_dot), x_max.max(x_dot), y_min.min(y_dot), y_max.max(y_dot)))
            } else {
                Some((x_dot, x_dot, y_dot, y_dot))
            }
        }) else {
            println!("WARNING: OBB does not have vertices");
            return false
        };

        println!(
            "intersecs_inner [{obb:?} <> {other:?}] {} {} {} {}",
            x_min, x_max, y_min, y_max
        );

        if obb.xs < x_min || x_max < -obb.xs || obb.ys < y_min || y_max < -obb.ys {
            return false;
        }

        true
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_obb_collision() {
        let a = CollisionShape::BBox(Obb {
            center: Vector2::new(-1., 0.),
            xs: 1.,
            ys: 1.,
            orient: 0.,
        });

        let a2 = CollisionShape::BBox(Obb {
            center: Vector2::new(-1., 0.),
            xs: 1.,
            ys: 1.,
            orient: std::f64::consts::PI / 4.,
        });

        let b = CollisionShape::BBox(Obb {
            center: Vector2::new(1.2, 0.),
            xs: 1.,
            ys: 1.,
            orient: 0.,
        });

        assert_eq!(a.intersects(&b), false);
        assert_eq!(a2.intersects(&b), true);
    }

    #[test]
    fn test_obb_collision_long() {
        let a = CollisionShape::BBox(Obb {
            center: Vector2::new(-2., 0.),
            xs: 5.,
            ys: 0.5,
            orient: 0.,
        });

        let a2 = CollisionShape::BBox(Obb {
            center: Vector2::new(-2., 0.),
            xs: 5.,
            ys: 0.5,
            orient: std::f64::consts::PI / 4.,
        });

        let b = CollisionShape::BBox(Obb {
            center: Vector2::new(2., 0.),
            xs: 1.,
            ys: 1.,
            orient: 0.,
        });

        assert_eq!(a.intersects(&b), true);
        assert_eq!(a2.intersects(&b), false);
    }
}
