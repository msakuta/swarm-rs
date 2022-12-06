use super::{motion::OrientToResult, AgentState, FollowPathResult};
use behavior_tree_lite::{
    error::LoadError, load, parse_file, BehaviorCallback, BehaviorNode, BehaviorResult, Context,
    Lazy, PortSpec, Registry, Symbol,
};
use cgmath::{Matrix2, Rad, Vector2};
use rand::{distributions::Uniform, prelude::Distribution};

/// Boundary to skip Debug trait from propagating to BehaviorNode trait
pub(super) struct BehaviorTree(pub Box<dyn BehaviorNode>);

impl std::fmt::Debug for BehaviorTree {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}

fn boxify<T>(cons: impl (Fn() -> T) + 'static) -> Box<dyn Fn() -> Box<dyn BehaviorNode>>
where
    for<'a> T: BehaviorNode + 'static,
{
    Box::new(move || Box::new(cons()))
}

pub(super) fn build_tree(source: &str) -> Result<BehaviorTree, LoadError> {
    let mut registry = Registry::default();
    registry.register("SetBool", boxify(|| SetBool));
    registry.register("Print", boxify(|| PrintNode));
    registry.register("HasTarget", boxify(|| HasTarget));
    registry.register("FindEnemy", boxify(|| FindEnemy));
    registry.register("HasPath", boxify(|| HasPath));
    registry.register("FindPath", boxify(|| FindPath));
    registry.register("Drive", boxify(|| DriveNode));
    registry.register("MoveTo", boxify(|| MoveToNode));
    registry.register("FollowPath", boxify(|| FollowPath));
    registry.register("Shoot", boxify(|| ShootNode));
    registry.register("Timeout", boxify(|| TimeoutNode(None)));
    registry.register("Randomize", boxify(|| RandomizeNode));
    registry.register("Avoidance", boxify(|| AvoidanceNode));
    registry.register("ClearAvoidance", boxify(|| ClearAvoidanceNode));
    registry.register("PathNextNode", boxify(|| PathNextNode));
    registry.register("PredictForward", boxify(|| PredictForwardNode));
    registry.register("NewPosition", boxify(|| NewPositionNode));
    registry.register("IsTargetVisible", boxify(|| IsTargetVisibleNode));
    registry.register("FaceToTarget", boxify(|| FaceToTargetNode));

    let (i, tree_source) = parse_file(source).unwrap();
    println!("parse_file rest: {i:?}");
    Ok(BehaviorTree(load(&tree_source, &registry, true)?))
}

pub(super) struct SetBool;

impl BehaviorNode for SetBool {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("value"), PortSpec::new_out("output")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = ctx.get_parse::<bool>("value");
        if let Some(value) = result {
            ctx.set("output", value);
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct PrintNode;

impl BehaviorNode for PrintNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("input")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(result) = ctx.get::<String>("input") {
            println!("PrintNode: {result:?}");
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct HasTarget;

static TARGET: Lazy<Symbol> = Lazy::new(|| "target".into());
static TARGET_SPEC: Lazy<PortSpec> = Lazy::new(|| PortSpec::new_in(*TARGET));

impl BehaviorNode for HasTarget {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![*TARGET_SPEC]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = ctx.get::<Option<usize>>(*TARGET);
        // println!("HasTarge node {result:?}");
        if result.map(|a| a.is_some()).unwrap_or(false) {
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
        // println!("FindEnemy node");
        arg(&FindEnemyCommand);
        BehaviorResult::Success
    }
}

pub(super) struct HasPath;

impl<'a> BehaviorNode for HasPath {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("has_path")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let has_path = ctx.get::<bool>("has_path");
        if has_path.copied().unwrap_or(false) {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct FindPathCommand;

pub(super) struct FindPath;

impl BehaviorNode for FindPath {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(&FindPathCommand);
        BehaviorResult::Success
    }
}

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
        let res = res
            .as_ref()
            .and_then(|res| res.downcast_ref::<FollowPathResult>())
            .map(|b| *b)
            .unwrap_or(FollowPathResult::Blocked);

        ctx.set("arrived", matches!(res, FollowPathResult::Arrived));

        if res.into() {
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
            if arg(&MoveToCommand(*pos))
                .and_then(|res| res.downcast_ref::<bool>().copied())
                .unwrap_or(false)
            {
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
        println!("ClearAvoidance node ticked!");
        let res = arg(&ClearAvoidanceCommand)
            .and_then(|res| res.downcast_ref::<bool>().copied())
            .unwrap_or(false);
        // println!("ClearAvoidance returns {res}");
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

pub(super) struct IsTargetVisibleCommand(pub usize);
pub(crate) struct IsTargetVisibleNode;

impl BehaviorNode for IsTargetVisibleNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![*TARGET_SPEC]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(target) = ctx.get::<Option<usize>>(*TARGET).copied().flatten() {
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

pub(super) struct FaceToTargetCommand(pub usize);

pub(super) struct FaceToTargetNode;

impl BehaviorNode for FaceToTargetNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![*TARGET_SPEC]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(target) = ctx.get::<Option<usize>>(*TARGET).copied().flatten() {
            let Some(val) = arg(&FaceToTargetCommand(target))
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
