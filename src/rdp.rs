//! Path simplify with [Ramer–Douglas–Peucker algorithm](https://en.wikipedia.org/wiki/Ramer%E2%80%93Douglas%E2%80%93Peucker_algorithm)

use cgmath::{InnerSpace, Vector2};

fn perpendicular_distance(point: &[f64; 2], line: &[[f64; 2]; 2]) -> f64 {
    let org = Vector2::from(line[0]);
    let delta_p = Vector2::from(*point) - org;
    let delta_l = (Vector2::from(line[1]) - org).normalize();
    let dot = delta_p.dot(delta_l);
    let delta = delta_p - delta_l * dot;
    delta.magnitude()
}

pub(crate) fn rdp(point_list: &[[f64; 2]], epsilon: f64) -> Vec<[f64; 2]> {
    if point_list.len() <= 2 {
        return point_list.to_vec();
    }
    // Find the point with the maximum distance
    let end = point_list.len() - 1;
    let segment = &[point_list[0], point_list[end]];
    let (index, dmax) = point_list[1..end]
        .iter()
        .enumerate()
        .map(|(i, point)| (i + 1, perpendicular_distance(point, segment)))
        .fold((0, 0.), |a, b| if a.1 < b.1 { b } else { a });

    // If max distance is greater than epsilon, recursively simplify
    if dmax > epsilon {
        // Recursive call
        let mut results1 = rdp(&point_list[..index + 1], epsilon);
        let results2 = rdp(&point_list[index..], epsilon);

        // Build the result list
        results1.extend_from_slice(&results2);
        results1
    } else {
        vec![point_list[0], point_list[end]]
    }
}
