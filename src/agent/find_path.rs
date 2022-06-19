use super::Agent;
use crate::triangle_utils::{center_of_triangle_obj, find_triangle_at};
use ::cgmath::{InnerSpace, Vector2};
use ::delaunator::Triangulation;
use std::{cmp::Reverse, collections::BinaryHeap};

fn delaunator_to_vector(p: delaunator::Point) -> Vector2<f64> {
    Vector2::new(p.x, p.y)
}

#[test]
fn inftest() {
    assert!(std::f64::INFINITY == std::f64::INFINITY);
}

impl Agent {
    pub fn find_path<'a, 'b>(
        &'a mut self,
        target: Option<&Agent>,
        triangulation: &Triangulation,
        points: &[delaunator::Point],
        triangle_passable: &[bool],
    ) -> Result<(), ()> {
        if let Some(target) = target {
            let this_triangle = find_triangle_at(&triangulation, &points, self.pos).ok_or(())?;
            let target_triangle = find_triangle_at(&triangulation, &points, target.pos).ok_or(())?;
            if this_triangle == target_triangle {
                // self.path_line = vec![
                //         self.pos,
                //         target.pos
                // ];
                return Ok(());
            }
            let mut costmap = vec![std::f64::INFINITY; triangulation.triangles.len()];
            let mut came_from_map = vec![None; triangulation.triangles.len()];
            costmap[this_triangle] = 0.;
            let mut open_set = BinaryHeap::new();
            open_set.push(Reverse(this_triangle));
            'topLabel: while let Some(Reverse(top)) = open_set.pop() {
                let center_top = center_of_triangle_obj(triangulation, points, top);
                let top_cost = costmap[top];
                for j in 0..3 {
                    let next_halfedge = triangulation.halfedges[top * 3 + j];
                    if next_halfedge == delaunator::EMPTY || !triangle_passable[next_halfedge / 3] {
                        continue;
                    }
                    let next_triangle = next_halfedge / 3;
                    let next_cost = costmap[next_triangle];
                    if next_cost != std::f64::INFINITY {
                        continue;
                    }
                    let center_next = center_of_triangle_obj(triangulation, points, next_triangle);
                    let delta = delaunator_to_vector(center_top.clone())
                        - delaunator_to_vector(center_next);
                    let dist = delta.magnitude();
                    if next_cost < top_cost + dist {
                        continue;
                    }
                    costmap[next_triangle] = top_cost + dist;
                    came_from_map[next_triangle] = Some(top);
                    open_set.push(Reverse(next_triangle));
                    if next_triangle == target_triangle {
                        break 'topLabel;
                    }
                }
            }
            let mut found_path = false;
            if let Some(came_from) = came_from_map[target_triangle] {
                self.path = vec![];
                let mut traverser = Some(came_from);
                while let Some(next) = traverser {
                    if next == this_triangle {
                        break;
                    }
                    let p = center_of_triangle_obj(triangulation, points, next);
                    self.path.push([p.x, p.y]);
                    traverser = came_from_map[next];
                    found_path = true;
                }
            }
            if !found_path {
                self.unreachables.insert(target.id);
                self.target = None;
            }

            if !self.path.is_empty() {}
        }
        Ok(())
    }
}
