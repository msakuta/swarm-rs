use ::delaunator::{Point, Triangulation};
use std::collections::HashSet;

use crate::{game::Profiler, measure_time};

pub(crate) fn center_of_triangle(v1: Point, v2: Point, v3: Point) -> Point {
    Point {
        x: (v1.x + v2.x + v3.x) / 3.,
        y: (v1.y + v2.y + v3.y) / 3.,
    }
}

pub(crate) fn center_of_triangle_obj(
    triangulation: &Triangulation,
    points: &[Point],
    idx: usize,
) -> Point {
    center_of_triangle(
        points[triangulation.triangles[idx * 3]].clone(),
        points[triangulation.triangles[idx * 3 + 1]].clone(),
        points[triangulation.triangles[idx * 3 + 2]].clone(),
    )
}

pub(crate) fn sign(p1: Point, p2: Point, p3: Point) -> f64 {
    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
}

pub(crate) fn point_in_triangle(pt: Point, v1: Point, v2: Point, v3: Point) -> bool {
    let d1 = sign(pt.clone(), v1.clone(), v2.clone());
    let d2 = sign(pt.clone(), v2.clone(), v3.clone());
    let d3 = sign(pt.clone(), v3.clone(), v1.clone());

    let has_neg = (d1 < 0.) || (d2 < 0.) || (d3 < 0.);
    let has_pos = (d1 > 0.) || (d2 > 0.) || (d3 > 0.);

    return !(has_neg && has_pos);
}

fn to_point(p: [f64; 2]) -> Point {
    Point { x: p[0], y: p[1] }
}

/// Returns triangle id (multiply with 3 to get index into `triangulation.triangles`)
pub(crate) fn find_triangle_at(
    triangulation: &Triangulation,
    points: &[Point],
    point: [f64; 2],
    profiler: &mut Profiler,
) -> Option<usize> {
    let (ret, time) = measure_time(move || {
        let triangles = &triangulation.triangles;
        let point = to_point(point);
        for (i, triangle) in triangles.chunks(3).enumerate() {
            let [v1, v2, v3] = [
                points[triangle[0]].clone(),
                points[triangle[1]].clone(),
                points[triangle[2]].clone(),
            ];
            if point_in_triangle(point.clone(), v1, v2, v3) {
                return Some(i);
            }
        }
        None
    });
    profiler.add(time);
    ret
}

pub(crate) fn label_triangles(
    triangulation: &Triangulation,
    passable_triangles: &[bool],
) -> Vec<i32> {
    let mut ret = vec![-1; triangulation.triangles.len() / 3];
    let mut label_gen = 0;
    for (i, _) in passable_triangles.iter().enumerate().filter(|(_, f)| **f) {
        if 0 <= ret[i] {
            continue;
        }
        let mut next_set = HashSet::new();
        ret[i] = label_gen;
        for j in 0..3 {
            let cand_tri = triangulation.halfedges[i * 3 + j];
            if cand_tri != delaunator::EMPTY {
                next_set.insert(cand_tri / 3);
            }
        }
        while let Some(&next_tri) = next_set.iter().next() {
            next_set.remove(&next_tri);
            if passable_triangles[next_tri] && ret[next_tri] == -1 {
                ret[next_tri] = label_gen;
                for j in 0..3 {
                    let cand_tri = triangulation.halfedges[next_tri * 3 + j];
                    if cand_tri != delaunator::EMPTY {
                        next_set.insert(cand_tri / 3);
                    }
                }
            }
        }
        label_gen += 1;
    }
    for i in 0..10 {
        println!(
            "label {} has {}",
            i,
            ret.iter().filter(|l| **l == i).count()
        );
    }
    ret
}
