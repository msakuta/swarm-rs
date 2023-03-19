use std::cell::RefCell;

use cgmath::{InnerSpace, Vector2};

use crate::{entity::Entity, game::is_passable_at, game::Game};

use super::{wrap_angle, Agent, AgentState, MotionResult};

/// The agent can take only one of the motion commands in one tick.
/// This enum will store the result from previous tick, because behavior tree may try
/// to perform multiple commands in a tick, but the actual agent can take only one move.
#[derive(Debug)]
pub(super) enum MotionCommandResult {
    Drive(bool),
    MoveTo(MotionResult),
    FollowPath(MotionResult),
    FaceToTarget(OrientToResult),
}

macro_rules! impl_as_result {
    { $name:ident, $variant:path } => {
        pub(super) fn $name(this: &Option<Self>) -> Option<Box<dyn std::any::Any>> {
            if let Some($variant(r)) = this {
                Some(Box::new(*r) as Box<dyn std::any::Any>)
            } else {
                None
            }
        }
    }
}

impl MotionCommandResult {
    impl_as_result!(as_drive, MotionCommandResult::Drive);

    pub(super) fn as_move_to(this: &Option<Self>) -> Option<Box<dyn std::any::Any>> {
        let r = this.as_ref()?;
        if let MotionCommandResult::MoveTo(r) = r {
            Some(Box::new(*r) as Box<dyn std::any::Any>)
        } else {
            None
        }
    }

    /// FollowPath command will return Following until it finds a path
    pub(super) fn as_follow_path(this: &Option<Self>) -> Option<Box<dyn std::any::Any>> {
        let r = this.as_ref()?;
        if let MotionCommandResult::FollowPath(r) = r {
            Some(Box::new(*r) as Box<dyn std::any::Any>)
        } else {
            Some(Box::new(MotionResult::Following) as Box<dyn std::any::Any>)
        }
    }

    pub(super) fn as_face_to_target(this: &Option<Self>) -> Option<Box<dyn std::any::Any>> {
        if let Some(Self::FaceToTarget(r)) = this {
            Some(Box::new(*r) as Box<dyn std::any::Any>)
        } else {
            None
        }
    }
}

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

        let self_shape = state.collision_shape(self.class);

        if !entities
            .iter()
            .filter_map(|entity| entity.try_borrow().ok())
            .any(|entity| {
                let shape = entity.get_shape();
                self_shape.intersects(&shape)
            })
        {
            // self.orient = state.heading;
            self.steer = wrapped_angle.max(-0.25 * PI).min(0.25 * PI);
            println!("Set steer to {:.06}", self.steer);
            if arrived || true {
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
        const WHEEL_BASE: f64 = 1.;
        let forward = Vector2::new(self.orient.cos(), self.orient.sin());
        let speed = self.class.speed();
        let target_pos = Vector2::from(self.pos) + drive.min(speed).max(-speed) * forward;
        let target_state = AgentState {
            x: target_pos.x,
            y: target_pos.y,
            heading: self.orient + speed * self.steer.tan() / WHEEL_BASE,
        };

        if Self::collision_check(Some(self.id), target_state, self.class, others, false) {
            self.speed = 0.;
            return false;
        }

        if is_passable_at(&game.board, (game.xs, game.ys), target_state.into()) {
            // if check_shape_in_mesh(
            //     &game.mesh,
            //     &target_state.collision_shape(),
            //     &mut *game.triangle_profiler.borrow_mut(),
            // ) {
            // if game.mesh.triangle_passable[next_triangle] {
            if 100 < self.trace.len() {
                self.trace.pop_front();
            }
            self.trace.push_back(self.pos);
            self.pos = target_pos.into();
            self.orient = target_state.heading;
            self.speed = drive;
            return true;
            // }
        }
        false
    }

    /// Returns whether the motion was achieved
    pub(crate) fn move_to(
        &mut self,
        game: &mut Game,
        target_pos: [f64; 2],
        backward: bool,
        others: &[RefCell<Entity>],
    ) -> MotionResult {
        let orient = self.orient_to(target_pos, backward, others);
        match orient {
            OrientToResult::Approaching => MotionResult::Following,
            OrientToResult::Arrived => {
                let delta = Vector2::from(target_pos) - Vector2::from(self.pos);
                let distance = delta.magnitude();

                self.drive(if backward { -distance } else { distance }, game, others)
                    .into()
            }
            OrientToResult::Blocked => {
                self.speed = 0.;
                MotionResult::Blocked
            }
        }
    }
}
