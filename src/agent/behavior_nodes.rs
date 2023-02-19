use crate::{
    behavior_tree_adapt::{common_tree_nodes, BehaviorTree},
    qtree::{qtree::PathFindError, QTreePathNode},
};

use super::{motion::OrientToResult, AgentClass, AgentState, MotionResult};
use behavior_tree_lite::{
    boxify, error::LoadError, load, parse_file, BehaviorCallback, BehaviorNode, BehaviorResult,
    Context, Lazy, PortSpec, Registry, Symbol,
};
use cgmath::{Matrix2, MetricSpace, Rad, Vector2};
use rand::{distributions::Uniform, prelude::Distribution};

pub(super) fn build_tree(source: &str) -> Result<BehaviorTree, LoadError> {
    let mut registry = Registry::default();
    common_tree_nodes(&mut registry);
    registry.register("GetClass", boxify(|| GetClass));
    registry.register("HasTarget", boxify(|| HasTargetNode));
    registry.register("TargetId", boxify(|| TargetIdNode));
    registry.register("TargetPos", boxify(|| TargetPosNode));
    registry.register("FindEnemy", boxify(|| FindEnemy));
    registry.register("FindSpawner", boxify(|| FindSpawner));
    registry.register("FindResource", boxify(|| FindResource));
    registry.register("ClearTarget", boxify(|| ClearTarget));
    registry.register("CollectResource", boxify(|| CollectResource));
    registry.register("DepositResource", boxify(|| DepositResource));
    registry.register("IsResourceFull", boxify(|| IsResourceFull));
    registry.register("IsSpawnerResourceFull", boxify(|| IsSpawnerResourceFull));
    registry.register("HasPath", boxify(|| HasPathNode));
    registry.register("ClearPath", boxify(|| ClearPathNode));
    registry.register("FindPath", boxify(|| FindPathNode));
    registry.register("DigestPath", boxify(|| DigestPathNode));
    registry.register("Drive", boxify(|| DriveNode));
    registry.register("MoveTo", boxify(|| MoveToNode));
    registry.register("FollowPath", boxify(|| FollowPath));
    registry.register("Shoot", boxify(|| ShootNode));
    registry.register("Timeout", boxify(|| TimeoutNode(None)));
    registry.register("Randomize", boxify(|| RandomizeNode));
    registry.register("Avoidance", boxify(|| AvoidanceNode));
    registry.register("SimpleAvoidance", boxify(|| SimpleAvoidanceNode));
    registry.register("ClearAvoidance", boxify(|| ClearAvoidanceNode));
    registry.register("PathNextNode", boxify(|| PathNextNode));
    registry.register("PredictForward", boxify(|| PredictForwardNode));
    registry.register("NewPosition", boxify(|| NewPositionNode));
    registry.register("IsTargetVisible", boxify(|| IsTargetVisibleNode));
    registry.register("FaceToTarget", boxify(|| FaceToTargetNode));

    let (_i, tree_source) = parse_file(source).unwrap();
    // println!("parse_file rest: {i:?}");
    Ok(BehaviorTree(load(&tree_source, &registry, true)?))
}

pub(super) struct GetClass;

impl BehaviorNode for GetClass {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_out("output")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = arg(self)
            .and_then(|a| a.downcast_ref::<AgentClass>().copied())
            .expect("Level (u32) should be always available");
        ctx.set("output", result.to_string());
        BehaviorResult::Success
    }
}

pub(super) struct HasTargetNode;

static TARGET: Lazy<Symbol> = Lazy::new(|| "target".into());
static TARGET_SPEC: Lazy<PortSpec> = Lazy::new(|| PortSpec::new_in(*TARGET));

impl BehaviorNode for HasTargetNode {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let Some(result) = arg(&Self).and_then(|a| a.downcast_ref::<bool>().copied()) else {
            return BehaviorResult::Fail;
        };
        if result {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct TargetIdNode;

impl BehaviorNode for TargetIdNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![*TARGET_SPEC]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(target) = arg(self).map(|res| res.downcast_ref::<usize>().copied()) {
            ctx.set(*TARGET, target);
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct FindEnemyCommand;

pub(super) struct FindEnemy;

impl BehaviorNode for FindEnemy {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(&FindEnemyCommand);
        BehaviorResult::Success
    }
}

pub(super) struct FindSpawner;

impl BehaviorNode for FindSpawner {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(self);
        BehaviorResult::Success
    }
}

pub(super) struct FindResource;

impl BehaviorNode for FindResource {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if arg(&Self)
            .and_then(|res| res.downcast_ref().copied())
            .unwrap_or(false)
        {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct ClearTarget;

impl BehaviorNode for ClearTarget {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let Some(result) = arg(self).and_then(|a| a.downcast_ref::<bool>().copied()) else {
            return BehaviorResult::Fail;
        };
        if result {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct CollectResource;

impl BehaviorNode for CollectResource {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(self)
            .and_then(|res| res.downcast_ref().copied())
            .unwrap()
    }
}

pub(super) struct DepositResource;

impl BehaviorNode for DepositResource {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(self)
            .and_then(|res| res.downcast_ref().copied())
            .unwrap()
    }
}

pub(super) struct IsResourceFull;

impl BehaviorNode for IsResourceFull {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if arg(self)
            .and_then(|res| res.downcast_ref().copied())
            .unwrap_or(false)
        {
            // println!("IsResourceFull? yes");
            BehaviorResult::Success
        } else {
            // println!("IsResourceFull? no");
            BehaviorResult::Fail
        }
    }
}

pub(super) struct IsSpawnerResourceFull;

impl BehaviorNode for IsSpawnerResourceFull {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if arg(self)
            .and_then(|res| res.downcast_ref().copied())
            .unwrap_or(false)
        {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct HasPathNode;

impl BehaviorNode for HasPathNode {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let has_path = arg(self)
            .and_then(|a| a.downcast::<bool>().ok())
            .map(|b| *b)
            .unwrap_or(false);
        if has_path {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct ClearPathNode;

impl BehaviorNode for ClearPathNode {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(self);
        BehaviorResult::Success
    }
}

pub(super) struct TargetPosCommand;

struct TargetPosNode;

impl BehaviorNode for TargetPosNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_out("pos")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(pos) =
            arg(&TargetPosCommand).and_then(|pos| pos.downcast_ref::<[f64; 2]>().copied())
        {
            ctx.set("pos", pos);
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct FindPathCommand(pub [f64; 2]);

pub(super) struct FindPathNode;

impl BehaviorNode for FindPathNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![
            *TARGET_SPEC,
            PortSpec::new_out("path"),
            PortSpec::new_out("fail_reason"),
        ]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let path_find_result = ctx
            .get::<[f64; 2]>(*TARGET)
            .and_then(|target| arg(&FindPathCommand(*target)))
            .and_then(|res| {
                res.downcast::<Result<Vec<QTreePathNode>, PathFindError>>()
                    .ok()
            })
            .expect(
                "PathFindCommand should always return Result<Vec<QTreePathNode>, PathFindError>",
            );
        match *path_find_result {
            Ok(path) => {
                ctx.set("path", path);
                BehaviorResult::Success
            }
            Err(err) => {
                ctx.set("fail_reason", err.to_string());
                BehaviorResult::Fail
            }
        }
    }
}

struct DigestPathNode;

impl BehaviorNode for DigestPathNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("input"), PortSpec::new_out("output")]
    }

    fn tick(&mut self, _arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(path) = ctx.get::<Vec<QTreePathNode>>("input") {
            ctx.set(
                "output",
                format!(
                    "{{nodes: {}, length: {:.03}}}",
                    path.len(),
                    path.iter().zip(path.iter().skip(1)).fold(0., |acc, cur| {
                        let prev = Vector2::from(cur.0.pos);
                        let next = Vector2::from(cur.1.pos);
                        acc + prev.distance(next)
                    })
                ),
            );
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct FollowPathCommand;

pub(super) struct FollowPath;

impl BehaviorNode for FollowPath {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_out("arrived")]
    }
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let res = arg(&FollowPathCommand);
        let res = res.as_ref().map(|r| {
            *r.downcast_ref::<MotionResult>()
                .expect("MotionResult type expected")
        });

        ctx.set("arrived", matches!(res, Some(MotionResult::Arrived)));

        if res.unwrap_or(MotionResult::Following).into() {
            BehaviorResult::Success
        } else {
            // println!("Can't follow path!");
            BehaviorResult::Fail
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct DriveCommand(pub f64);

pub(super) struct DriveNode;

impl BehaviorNode for DriveNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("direction")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(direction) = ctx.get::<String>("direction") {
            arg(&match direction as _ {
                "forward" => DriveCommand(1.),
                "backward" => DriveCommand(-1.),
                _ => return BehaviorResult::Fail,
            });
        }
        BehaviorResult::Success
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct MoveToCommand(pub [f64; 2]);

pub(super) struct MoveToNode;

impl BehaviorNode for MoveToNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("pos")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(pos) = ctx.get::<[f64; 2]>("pos") {
            if matches!(
                arg(&MoveToCommand(*pos))
                    .and_then(|res| res.downcast_ref::<MotionResult>().copied())
                    .expect("MotionResult type is expected"),
                MotionResult::Following
            ) {
                BehaviorResult::Success
            } else {
                BehaviorResult::Fail
            }
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct ShootCommand;

pub(super) struct ShootNode;

impl BehaviorNode for ShootNode {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(&ShootCommand);
        BehaviorResult::Success
    }
}

struct TimeoutNode(Option<usize>);

impl BehaviorNode for TimeoutNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("time")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(ref mut remaining) = self.0 {
            if *remaining == 0 {
                // println!("Timed out");
                self.0 = None;
                return BehaviorResult::Success;
            } else {
                *remaining -= 1;
                return BehaviorResult::Running;
            }
        } else if let Some(input) = ctx.get_parse::<usize>("time") {
            // println!("Timer set! {}", input);
            self.0 = Some(input);
        }
        BehaviorResult::Fail
    }
}

struct RandomizeNode;

impl BehaviorNode for RandomizeNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![
            PortSpec::new_in("min"),
            PortSpec::new_in("max"),
            PortSpec::new_out("value"),
        ]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(max) = ctx.get_parse::<usize>("max") {
            let min = ctx.get_parse::<usize>("min").unwrap_or(0);
            let between = Uniform::from(min..max);
            let value = between.sample(&mut rand::thread_rng());
            // println!("Randomizing! {}/{}", value, max);
            ctx.set::<usize>("value", value);
            return BehaviorResult::Success;
        }
        BehaviorResult::Fail
    }
}

pub(super) struct AvoidanceCommand {
    pub goal: [f64; 2],
    pub back: bool,
}

pub(super) struct AvoidanceNode;

impl BehaviorNode for AvoidanceNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("goal"), PortSpec::new_in("back")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(goal) = ctx.get::<[f64; 2]>("goal") {
            let back = ctx.get_parse::<bool>("back").unwrap_or(false);
            let res = arg(&AvoidanceCommand { goal: *goal, back })
                .and_then(|res| res.downcast_ref::<bool>().copied())
                .unwrap_or(false);
            if res {
                BehaviorResult::Success
            } else {
                BehaviorResult::Fail
            }
        } else {
            println!("Avoidance could not get goal!");
            BehaviorResult::Fail
        }
    }
}

pub(super) struct SimpleAvoidanceCommand(pub bool);

struct SimpleAvoidanceNode;

impl BehaviorNode for SimpleAvoidanceNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("back")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let back = ctx.get_parse::<bool>("back").unwrap_or(false);
        let res = arg(&SimpleAvoidanceCommand(back))
            .and_then(|res| res.downcast_ref::<bool>().copied())
            .unwrap_or(false);
        if res {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct IsArrivedGoalCommand;

pub(super) struct IsArrivedGoalNode;

impl BehaviorNode for IsArrivedGoalNode {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let res = arg(&IsArrivedGoalCommand)
            .and_then(|res| res.downcast_ref::<bool>().copied())
            .unwrap_or(false);
        if res {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct ClearAvoidanceCommand;

/// Clear avoidance path search state
pub(super) struct ClearAvoidanceNode;

impl BehaviorNode for ClearAvoidanceNode {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let res = arg(&ClearAvoidanceCommand)
            .and_then(|res| res.downcast_ref::<bool>().copied())
            .unwrap_or(false);
        if res {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct GetPathNextNodeCommand;

pub(super) struct PathNextNode;

impl BehaviorNode for PathNextNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_out("output")]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(value) =
            arg(&GetPathNextNodeCommand).and_then(|val| val.downcast_ref::<[f64; 2]>().copied())
        {
            ctx.set("output", value);
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct GetStateCommand;

pub(super) struct PredictForwardNode;

impl BehaviorNode for PredictForwardNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("distance"), PortSpec::new_out("output")]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(distance) = ctx.get_parse::<f64>("distance") {
            if let Some(state) =
                arg(&GetStateCommand).and_then(|val| val.downcast_ref::<AgentState>().cloned())
            {
                let pos = Matrix2::from_angle(Rad(state.heading)) * Vector2::new(distance, 0.)
                    + Vector2::new(state.x, state.y);
                let pos: [f64; 2] = pos.into();
                ctx.set("output", pos);
                return BehaviorResult::Success;
            }
        }
        BehaviorResult::Fail
    }
}

pub(super) struct NewPositionNode;

impl BehaviorNode for NewPositionNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![
            PortSpec::new_in("x"),
            PortSpec::new_in("y"),
            PortSpec::new_out("output"),
        ]
    }

    fn tick(&mut self, _arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some((x, y)) = ctx.get_parse::<f64>("x").zip(ctx.get_parse::<f64>("y")) {
            let pos: [f64; 2] = [x, y];
            ctx.set("output", pos);
            return BehaviorResult::Success;
        }
        BehaviorResult::Fail
    }
}

pub(super) struct IsTargetVisibleCommand(pub [f64; 2]);
pub(crate) struct IsTargetVisibleNode;

impl BehaviorNode for IsTargetVisibleNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![*TARGET_SPEC]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(target) = ctx.get::<[f64; 2]>(*TARGET).copied() {
            let res = arg(&IsTargetVisibleCommand(target))
                .and_then(|res| res.downcast_ref::<bool>().copied())
                .unwrap_or(false);
            if res {
                BehaviorResult::Success
            } else {
                BehaviorResult::Fail
            }
        } else {
            BehaviorResult::Fail
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct FaceToTargetCommand(pub [f64; 2]);

pub(super) struct FaceToTargetNode;

impl BehaviorNode for FaceToTargetNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![*TARGET_SPEC]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(target) = ctx.get::<[f64; 2]>(*TARGET) {
            let Some(val) = arg(&FaceToTargetCommand(*target))
                .and_then(|val| val.downcast_ref::<OrientToResult>().copied()) else {
                    return BehaviorResult::Fail;
                };

            if val.into() {
                BehaviorResult::Success
            } else {
                BehaviorResult::Running
            }
        } else {
            BehaviorResult::Fail
        }
    }
}
