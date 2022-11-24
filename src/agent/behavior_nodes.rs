use super::State;
use behavior_tree_lite::{
    error::LoadError, load, parse_file, BehaviorCallback, BehaviorNode, BehaviorResult, Context,
    Lazy, Registry, Symbol,
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
    registry.register("Move", boxify(|| MoveNode));
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

    Ok(BehaviorTree(load(
        &parse_file(source).unwrap().1,
        &registry,
        true,
    )?))
}

pub(super) struct SetBool;

impl BehaviorNode for SetBool {
    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = ctx.get::<Option<usize>>("direction");
        // println!("HasTarge node {result:?}");
        if result.map(|a| a.is_some()).unwrap_or(false) {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct PrintNode;

impl BehaviorNode for PrintNode {
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["input".into()]
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

impl BehaviorNode for HasTarget {
    fn provided_ports(&self) -> Vec<Symbol> {
        vec![*TARGET]
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
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["has_path".into()]
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
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let res = arg(&FollowPathCommand);
        if res
            .as_ref()
            .and_then(|res| res.downcast_ref::<bool>())
            .map(|b| *b)
            .unwrap_or(false)
        {
            BehaviorResult::Success
        } else {
            // println!("Can't follow path!");
            BehaviorResult::Fail
        }
    }
}

pub(super) struct MoveCommand(pub String);

pub(super) struct MoveNode;

impl BehaviorNode for MoveNode {
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["direction".into()]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(direction) = ctx.get::<String>("direction") {
            arg(&MoveCommand(direction.clone()));
        }
        BehaviorResult::Success
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
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["time".into()]
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
        } else if let Some(input) = ctx
            .get::<String>("time")
            .and_then(|s| s.parse::<usize>().ok())
            .or_else(|| ctx.get::<usize>("time").copied())
        {
            // println!("Timer set! {}", input);
            self.0 = Some(input);
        }
        BehaviorResult::Fail
    }
}

struct RandomizeNode;

impl BehaviorNode for RandomizeNode {
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["min".into(), "max".into(), "value".into()]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(max) = ctx
            .get::<String>("max")
            .and_then(|s| s.parse::<usize>().ok())
        {
            let min = ctx
                .get::<String>("min")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(0);
            let between = Uniform::from(min..max);
            let value = between.sample(&mut rand::thread_rng());
            // println!("Randomizing! {}/{}", value, max);
            ctx.set::<usize>("value", value);
            return BehaviorResult::Success;
        }
        BehaviorResult::Fail
    }
}

pub(super) struct AvoidanceCommand(pub [f64; 2]);

pub(super) struct AvoidanceNode;

impl BehaviorNode for AvoidanceNode {
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["goal".into()]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(goal) = ctx.get::<[f64; 2]>("goal") {
            let res = arg(&AvoidanceCommand(*goal))
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
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["output".into()]
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
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["distance".into(), "output".into()]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(distance) = ctx.get::<f64>("distance").copied().or_else(|| {
            ctx.get::<String>("distance")
                .and_then(|val| val.parse::<f64>().ok())
        }) {
            if let Some(state) =
                arg(&GetStateCommand).and_then(|val| val.downcast_ref::<State>().cloned())
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

fn get_f64<K: Into<Symbol> + Copy>(ctx: &mut Context, key: K) -> Option<f64> {
    ctx.get::<f64>(key).copied().or_else(|| {
        ctx.get::<String>(key)
            .and_then(|val| val.parse::<f64>().ok())
    })
}

pub(super) struct NewPositionNode;

impl BehaviorNode for NewPositionNode {
    fn provided_ports(&self) -> Vec<Symbol> {
        vec!["x".into(), "y".into(), "output".into()]
    }

    fn tick(&mut self, _arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some((x, y)) = get_f64(ctx, "x").zip(get_f64(ctx, "y")) {
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
    fn provided_ports(&self) -> Vec<Symbol> {
        vec![*TARGET]
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
    fn provided_ports(&self) -> Vec<Symbol> {
        vec![*TARGET]
    }

    fn tick(&mut self, arg: BehaviorCallback, ctx: &mut Context) -> BehaviorResult {
        if let Some(target) = ctx.get::<Option<usize>>(*TARGET).copied().flatten() {
            let val = arg(&FaceToTargetCommand(target))
                .and_then(|val| val.downcast_ref::<bool>().copied())
                .unwrap_or(false);

            if val {
                BehaviorResult::Success
            } else {
                BehaviorResult::Running
            }
        } else {
            BehaviorResult::Fail
        }
    }
}
