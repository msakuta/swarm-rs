use std::collections::HashMap;

use delaunator::{triangulate, Triangulation};
#[cfg(feature = "druid")]
use druid::{kurbo::BezPath, Point};

use crate::{
    app_data::is_passable_at,
    dijkstra::label,
    marching_squares::{trace_lines, BoolField},
    measure_time,
    triangle_utils::{center_of_triangle_obj, label_triangles},
};

/// The Mesh is a data structure to allow AI controlled agents to navigate or help detection
/// collision.
#[derive(Debug)]
pub struct Mesh {
    #[cfg(feature = "druid")]
    pub simplified_border: Vec<BezPath>,
    pub polygons: geo::geometry::MultiPolygon,
    pub points: Vec<delaunator::Point>,
    pub triangulation: Triangulation,
    pub triangle_passable: Vec<bool>,
    pub triangle_labels: Vec<i32>,
    pub largest_label: Option<i32>,
    pub labeled_image: Vec<i32>,
}

pub(crate) struct MeshResult {
    pub board: Vec<bool>,
    pub mesh: Mesh,
}

pub(crate) fn create_mesh(
    (xs, ys): (usize, usize),
    simplify_epsilon: f64,
    mut pixel_proc: impl FnMut(usize, usize) -> bool,
) -> MeshResult {
    let mut board = vec![false; xs * ys];
    for (i, cell) in board.iter_mut().enumerate() {
        let xi = i % xs;
        let yi = i / xs;
        *cell = pixel_proc(xi, yi);
    }

    println!(
        "true: {}, false: {}",
        board.iter().filter(|c| **c).count(),
        board.iter().filter(|c| !**c).count()
    );

    let (labeled_image, time) = measure_time(|| label(&board, (xs, ys)));
    let max_labels = labeled_image.iter().max();
    println!("Labeled: {:?} in {} s", max_labels, time);

    if let Some((largest_label, _)) = max_labels.and_then(|max_labels| {
        (1..*max_labels)
            .map(|label| {
                (
                    label,
                    labeled_image.iter().filter(|pix| **pix == label).count(),
                )
            })
            .max_by_key(|(_, count)| *count)
    }) {
        for (pix, pix_label) in board.iter_mut().zip(labeled_image.iter()) {
            if *pix_label != 0 && *pix_label != largest_label {
                *pix = false;
            }
        }
    }

    let shape = (xs as isize, ys as isize);

    let field = BoolField::new(&board, shape);

    #[cfg(feature = "druid")]
    let mut simplified_border = vec![];
    let mut polygons = vec![];
    let mut points = vec![];

    #[cfg(feature = "druid")]
    let to_point = |p: [f64; 2]| Point::new(p[0] as f64, p[1] as f64);

    let lines = trace_lines(&field);
    #[cfg(feature = "druid")]
    let mut _simplified_vertices = 0;
    for line in &lines {
        let simplified = if simplify_epsilon == 0. {
            line.iter().map(|p| [p[0] as f64, p[1] as f64]).collect()
        } else {
            // println!("rdp closed: {} start/end: {:?}/{:?}", line.first() == line.last(), line.first(), line.last());

            // if the ring is closed, remove the last element to open it, because rdp needs different start and end points
            let mut slice = &line[..];
            while 1 < slice.len() && slice.first() == slice.last() {
                slice = &slice[..slice.len() - 1];
            }

            crate::rdp::rdp(
                &slice
                    .iter()
                    .map(|p| [p[0] as f64, p[1] as f64])
                    .collect::<Vec<_>>(),
                simplify_epsilon,
            )
        };

        // If the polygon does not make up a triangle, skip it
        if simplified.len() <= 2 {
            continue;
        }

        #[cfg(feature = "druid")]
        if let Some((first, rest)) = simplified.split_first() {
            let mut bez_path = BezPath::new();
            bez_path.move_to(to_point(*first));
            for point in rest {
                bez_path.line_to(to_point(*point));
                points.push(delaunator::Point {
                    x: point[0],
                    y: point[1],
                });
            }
            bez_path.close_path();
            simplified_border.push(bez_path);
            _simplified_vertices += simplified.len();
        }

        let line_string: geo::geometry::LineString = simplified
            .iter()
            .copied()
            .map(geo::geometry::Point::from)
            .collect();
        polygons.push(geo::geometry::Polygon::new(line_string, vec![]));
    }
    // println!(
    //     "trace_lines: {}, vertices: {}, simplified_border: {} vertices: {}",
    //     lines.len(),
    //     lines.iter().map(|line| line.len()).sum::<usize>(),
    //     simplified_border.len(),
    //     _simplified_vertices
    // );

    let triangulation = triangulate(&points);

    let triangle_passable = calc_passable_triangles(&board, (xs, ys), &points, &triangulation);

    let triangle_labels = label_triangles(&triangulation, &triangle_passable);

    let mut label_stats = HashMap::new();
    for label in &triangle_labels {
        if *label != -1 {
            *label_stats.entry(*label).or_insert(0) += 1;
        }
    }
    let largest_label = label_stats
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(key, _)| *key);

    MeshResult {
        board,
        mesh: Mesh {
            #[cfg(feature = "druid")]
            simplified_border,
            polygons: polygons.into_iter().collect(),
            points,
            triangulation,
            triangle_passable,
            triangle_labels,
            largest_label,
            labeled_image,
        },
    }
}

pub(crate) fn calc_passable_triangles(
    board: &[bool],
    shape: (usize, usize),
    points: &[delaunator::Point],
    triangulation: &Triangulation,
) -> Vec<bool> {
    triangulation
        .triangles
        .chunks(3)
        .enumerate()
        .map(|(t, _)| {
            let pos = center_of_triangle_obj(&triangulation, points, t);
            is_passable_at(&board, shape, [pos.x, pos.y])
        })
        .collect()
}
