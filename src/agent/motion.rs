use std::cell::RefCell;

use cgmath::{InnerSpace, Vector2};

use crate::{entity::Entity, game::Game, triangle_utils::check_shape_in_mesh};

use super::{wrap_angle, Agent, AgentState, AGENT_SPEED};

#[derive(Debug, Clone, Copy)]
pub(super) enum OrientToResult {
    Approaching,
    Arrived,
    Blocked,
}

impl From<OrientToResult> for bool {
    fn from(result: OrientToResult) -> Self {
        matches!(result, OrientToResult::Arrived)
    }
}

impl Agent {
    pub(super) fn orient_to(
        &mut self,
        target: [f64; 2],
        backward: bool,
        entities: &[RefCell<Entity>],
    ) -> OrientToResult {
        use std::f64::consts::PI;
        const TWOPI: f64 = PI * 2.;
        const ANGLE_SPEED: f64 = PI / 50.;
        let delta = Vector2::from(target) - Vector2::from(self.pos);
        let target_angle = delta.y.atan2(delta.x);
        let target_angle = if backward {
            wrap_angle(target_angle + PI)
        } else {
            target_angle
        };
        let delta_angle = target_angle - self.orient;
        let wrapped_angle = wrap_angle(delta_angle);
        let (state, arrived) = if wrapped_angle.abs() < ANGLE_SPEED {
            (self.to_state().with_orient(target_angle), true)
        } else if wrapped_angle < 0. {
            let orient = (self.orient - ANGLE_SPEED) % TWOPI;
            (
                self.to_state().with_orient(orient),
                wrapped_angle.abs() < PI / 4.,
            )
        } else {
            let orient = (self.orient + ANGLE_SPEED) % TWOPI;
            (
                self.to_state().with_orient(orient),
                wrapped_angle.abs() < PI / 4.,
            )
        };

        let self_shape = state.collision_shape();

        if !entities
            .iter()
            .filter_map(|entity| entity.try_borrow().ok())
            .any(|entity| {
                let shape = entity.get_shape();
                self_shape.intersects(&shape)
            })
        {
            self.orient = state.heading;
            if arrived {
                OrientToResult::Arrived
            } else {
                OrientToResult::Approaching
            }
        } else {
            OrientToResult::Blocked
        }
    }

    pub(crate) fn drive(
        &mut self,
        drive: f64,
        game: &mut Game,
        others: &[RefCell<Entity>],
    ) -> bool {
        let forward = Vector2::new(self.orient.cos(), self.orient.sin());
        let target_pos =
            Vector2::from(self.pos) + drive.min(AGENT_SPEED).max(-AGENT_SPEED) * forward;
        let target_state = AgentState {
            x: target_pos.x,
            y: target_pos.y,
            heading: self.orient,
        };

        if Self::collision_check(Some(self.id), target_state, others) {
            self.speed = 0.;
            return false;
        }

        if check_shape_in_mesh(
            &game.mesh,
            &target_state.collision_shape(),
            &mut *game.triangle_profiler.borrow_mut(),
        ) {
            // if game.mesh.triangle_passable[next_triangle] {
            if 100 < self.trace.len() {
                self.trace.pop_front();
            }
            self.trace.push_back(self.pos);
            self.pos = target_pos.into();
            self.speed = drive;
            return true;
            // }
        }
        false
    }

    pub(crate) fn move_to(
        &mut self,
        game: &mut Game,
        target_pos: [f64; 2],
        backward: bool,
        others: &[RefCell<Entity>],
    ) -> bool {
        if self.orient_to(target_pos, backward, others).into() {
            let delta = Vector2::from(target_pos) - Vector2::from(self.pos);
            let distance = delta.magnitude();

            self.drive(if backward { -distance } else { distance }, game, others)
        } else {
            true
        }
    }
}
