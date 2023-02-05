use behavior_tree_lite::{
    boxify, error::LoadError, load, parse_file, BehaviorCallback, BehaviorNode, BehaviorResult,
    Registry,
};

use crate::behavior_tree_adapt::{common_tree_nodes, BehaviorTree};

pub(super) fn build_tree(source: &str) -> Result<BehaviorTree, LoadError> {
    let mut registry = Registry::default();
    common_tree_nodes(&mut registry);
    registry.register("SpawnFighter", boxify(|| SpawnFighter));
    registry.register("SpawnWorker", boxify(|| SpawnWorker));

    let (_i, tree_source) = parse_file(source).unwrap();
    // println!("parse_file rest: {i:?}");
    Ok(BehaviorTree(load(&tree_source, &registry, true)?))
}

macro_rules! spawn_impl {
    {$name:ident} => {
        pub(super) struct $name;

        impl BehaviorNode for $name {
            fn tick(
                &mut self,
                arg: BehaviorCallback,
                _ctx: &mut behavior_tree_lite::Context,
            ) -> BehaviorResult {
                let result = arg(&Self).and_then(|a| a.downcast_ref::<bool>().copied()).expect("Spawn should return a bool");
                if result {
                    BehaviorResult::Success
                } else {
                    BehaviorResult::Fail
                }
            }
        }
    }
}

spawn_impl!(SpawnFighter);
spawn_impl!(SpawnWorker);
