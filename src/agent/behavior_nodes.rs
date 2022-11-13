use behavior_tree_lite::{load, parse_file, BehaviorNode, BehaviorResult, Registry};

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

pub(super) fn build_tree() -> BehaviorTree {
    let mut registry = Registry::default();
    registry.register("SetBool", boxify(|| SetBool));
    registry.register("HasTarget", boxify(|| HasTarget));
    registry.register("FindEnemy", boxify(|| FindEnemy));
    registry.register("PrintTarget", boxify(|| PrintTarget));
    registry.register("HasPath", boxify(|| HasPath));
    registry.register("FindPath", boxify(|| FindPath));
    registry.register("Move", boxify(|| Move));
    registry.register("FollowPath", boxify(|| FollowPath));

    BehaviorTree(
        load(
            &parse_file(
                r#"
tree main = Sequence {
    Fallback {
        HasTarget (target <- target)
        FindEnemy
    }
    Fallback {
        HasPath (has_path <- has_path)
        FindPath
    }
    Sequence {
        HasPath (has_path <- has_path)
        FollowPath
    }
}"#,
            )
            .unwrap()
            .1,
            &registry,
        )
        .unwrap(),
    )
}

pub(super) struct SetBool;

impl BehaviorNode for SetBool {
    fn tick(
        &mut self,
        _arg: &mut dyn FnMut(&dyn std::any::Any),
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

pub(super) struct PrintTarget;

impl BehaviorNode for PrintTarget {
    fn tick(
        &mut self,
        arg: &mut dyn FnMut(&dyn std::any::Any),
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let target = ctx.get::<Option<usize>>("target".into());
        println!("PrintTarget: {target:?}");
        BehaviorResult::Success
    }
}

pub(super) struct HasTarget;

impl BehaviorNode for HasTarget {
    fn tick(
        &mut self,
        _arg: &mut dyn FnMut(&dyn std::any::Any),
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
        arg: &mut dyn FnMut(&dyn std::any::Any),
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
        arg: &mut dyn FnMut(&dyn std::any::Any),
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
        arg: &mut dyn FnMut(&dyn std::any::Any),
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        println!("FindPath node");
        arg(&FindPathCommand);
        BehaviorResult::Success
    }
}

pub(super) struct FollowPathCommand;

pub(super) struct FollowPath;

impl BehaviorNode for FollowPath {
    fn tick(
        &mut self,
        arg: &mut dyn FnMut(&dyn std::any::Any),
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        arg(&FollowPathCommand);
        BehaviorResult::Success
    }
}

pub(super) struct MoveCommand(pub String);

pub(super) struct Move;

impl BehaviorNode for Move {
    fn tick(
        &mut self,
        arg: &mut dyn FnMut(&dyn std::any::Any),
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(direction) = ctx.get::<String>("direction".into()) {
            println!("Direction: {direction:?}");
            arg(&MoveCommand(direction.clone()));
        }
        BehaviorResult::Success
    }
}
