use behavior_tree_lite::{load, parse_file, BehaviorNode, Registry};

/// Boundary to skip Debug trait from propagating to BehaviorNode trait
pub(super) struct BehaviorTree(pub Box<dyn BehaviorNode>);

impl std::fmt::Debug for BehaviorTree {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}

pub(super) fn build_tree() -> BehaviorTree {
    let mut registry = Registry::default();
    registry.register("HasTarget", Box::new(|| Box::new(HasTarget)));
    registry.register("PrintTarget", Box::new(|| Box::new(PrintMe)));

    BehaviorTree(
        load(
            &parse_file(
                "tree main = Sequence {
        HasTarget (target <- target)
        PrintTarget (target <- target)
    }",
            )
            .unwrap()
            .1,
            &registry,
        )
        .unwrap(),
    )
}

pub(super) struct PrintMe;

impl BehaviorNode for PrintMe {
    fn tick(
        &mut self,
        ctx: &mut behavior_tree_lite::Context,
    ) -> behavior_tree_lite::BehaviorResult {
        let target = ctx.get::<Option<usize>>("target".into());
        println!("PrintTarget: {target:?}");
        behavior_tree_lite::BehaviorResult::Success
    }
}

pub(super) struct HasTarget;

impl BehaviorNode for HasTarget {
    fn tick(
        &mut self,
        ctx: &mut behavior_tree_lite::Context,
    ) -> behavior_tree_lite::BehaviorResult {
        if ctx
            .get::<Option<usize>>("target".into())
            .map(|target| target.is_some())
            .unwrap_or(false)
        {
            behavior_tree_lite::BehaviorResult::Success
        } else {
            behavior_tree_lite::BehaviorResult::Fail
        }
    }
}
