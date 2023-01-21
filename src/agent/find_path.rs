use super::Agent;
use crate::{
    entity::Entity,
    game::Game,
    qtree::QTreePathNode,
    triangle_utils::{center_of_triangle_obj, find_triangle_at},
};
use ::cgmath::{InnerSpace, Vector2};

use std::{cell::RefCell, cmp::Reverse, collections::BinaryHeap};

fn delaunator_to_vector(p: delaunator::Point) -> Vector2<f64> {
    Vector2::new(p.x, p.y)
}

impl Agent {
    pub fn find_path(&mut self, target: [f64; 2], game: &mut Game) -> Result<(), ()> {
        println!("Agent::find_path");
        let (found_path, search_tree) = game.qtree.path_find(self.pos, target);
        self.search_tree = Some(search_tree);
        if let Some(path) = found_path {
            self.path = path
        }
        Ok(())
    }

    pub fn find_path_tri(&mut self, target: [f64; 2], game: &mut Game) -> Result<(), ()> {
        let triangulation = &game.mesh.triangulation;
        let points = &game.mesh.points;
        let mut profiler = game.triangle_profiler.borrow_mut();
        let this_triangle = find_triangle_at(&game.mesh, self.pos, &mut *profiler).ok_or(())?;
        let target_triangle = find_triangle_at(&game.mesh, target, &mut *profiler).ok_or(())?;
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
                if next_halfedge == delaunator::EMPTY
                    || !game.mesh.triangle_passable[next_halfedge / 3]
                {
                    continue;
                }
                let next_triangle = next_halfedge / 3;
                let next_cost = costmap[next_triangle];
                if next_cost != std::f64::INFINITY {
                    continue;
                }
                let center_next = center_of_triangle_obj(triangulation, points, next_triangle);
                let delta =
                    delaunator_to_vector(center_top.clone()) - delaunator_to_vector(center_next);
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
        if self.id == 1 {
            // println!("set: {target_triangle}, {:?}", came_from_map.iter().enumerate().filter(|(i, came)| came.is_some()).collect::<Vec<_>>());
        }
        if let Some(came_from) = came_from_map[target_triangle] {
            let first = center_of_triangle_obj(triangulation, points, target_triangle);
            self.path = vec![QTreePathNode::new([first.x, first.y], 5.)];
            let mut traverser = Some(came_from);
            while let Some(next) = traverser {
                if next == this_triangle {
                    break;
                }
                let p = center_of_triangle_obj(triangulation, points, next);
                self.path.push(QTreePathNode::new([p.x, p.y], 5.));
                traverser = came_from_map[next];
            }

            Ok(())
        } else {
            Err(())
        }
    }

    pub fn _find_path_to_enemy(
        &mut self,
        _target: usize,
        game: &mut Game,
        entities: &[RefCell<Entity>],
    ) -> Result<(), ()> {
        let Some(target) = self.target.and_then(|target| {
            entities.iter().find(|a| {
                a.try_borrow().map(|a| a.get_id() == target).unwrap_or(false)
            }).and_then(|a| a.try_borrow().ok())
        }) else {
            return Err(());
        };
        if self.find_path(target.get_pos(), game).is_err() {
            self.unreachables.insert(target.get_id());
            self.target = None;
            return Err(());
        };

        if !self.path.is_empty() {}

        Ok(())
    }
}
