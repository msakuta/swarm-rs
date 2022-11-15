use behavior_tree_lite::{
    load, parse_file, BehaviorCallback, BehaviorNode, BehaviorResult, Registry,
};

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

pub(super) fn build_tree(source: &str) -> BehaviorTree {
    let mut registry = Registry::default();
    registry.register("SetBool", boxify(|| SetBool));
    registry.register("HasTarget", boxify(|| HasTarget));
    registry.register("FindEnemy", boxify(|| FindEnemy));
    registry.register("HasPath", boxify(|| HasPath));
    registry.register("FindPath", boxify(|| FindPath));
    registry.register("Move", boxify(|| Move));
    registry.register("FollowPath", boxify(|| FollowPath));
    registry.register("Shoot", boxify(|| ShootNode));
    registry.register("Timeout", boxify(|| TimeoutNode(None)));

    BehaviorTree(load(&parse_file(source).unwrap().1, &registry).unwrap())
}

pub(super) struct SetBool;

impl BehaviorNode for SetBool {
    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = ctx.get::<Option<usize>>("direction".into());
        // println!("HasTarge node {result:?}");
        if result.map(|a| a.is_some()).unwrap_or(false) {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct HasTarget;

impl BehaviorNode for HasTarget {
    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = ctx.get::<Option<usize>>("target".into());
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
    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let has_path = ctx.get::<bool>("has_path".into());
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

pub(super) struct Move;

impl BehaviorNode for Move {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(direction) = ctx.get::<String>("direction".into()) {
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
            .get::<String>("time".into())
            .and_then(|s| s.parse::<usize>().ok())
        {
            // println!("Timer set! {}", input);
            self.0 = Some(input);
        }
        BehaviorResult::Fail
    }
}
