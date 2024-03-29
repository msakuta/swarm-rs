use behavior_tree_lite::{
    boxify, error::LoadError, load, parse_file, BehaviorCallback, BehaviorNode, BehaviorResult,
    PortSpec, Registry,
};

use crate::{
    agent::AgentClass,
    behavior_tree_adapt::{common_tree_nodes, BehaviorTree},
};

pub(super) fn build_tree(source: &str) -> Result<BehaviorTree, LoadError> {
    let mut registry = Registry::default();
    common_tree_nodes(&mut registry);
    registry.register("SpawnFighter", boxify(|| SpawnFighter));
    registry.register("SpawnWorker", boxify(|| SpawnWorker));
    registry.register("LastSpawnResult", boxify(|| LastSpawnResult));
    registry.register("CurrentSpawnTask", boxify(|| CurrentSpawnTask));
    registry.register("CancelSpawnTask", boxify(|| CancelSpawnTask));

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
                let result = arg(&Self).and_then(|a| a.downcast_ref::<bool>().copied());
                if result.is_some() {
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

pub(super) struct LastSpawnResult;

impl BehaviorNode for LastSpawnResult {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_out("type")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = arg(self)
            .and_then(|a| a.downcast_ref::<Option<AgentClass>>().copied())
            .expect("LastSpawnResult should return an Option<AgentClass>");
        if let Some(result) = result {
            ctx.set("type", result.to_string());
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(super) struct CurrentSpawnTask;

impl BehaviorNode for CurrentSpawnTask {
    fn provided_ports(&self) -> Vec<behavior_tree_lite::PortSpec> {
        vec![
            PortSpec::new_out("class"),
            PortSpec::new_out("remaining_ticks"),
        ]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = arg(self)
            .and_then(|a| a.downcast_ref::<Option<(usize, AgentClass)>>().copied())
            .expect("CurrentSpawnTask should return an Option<AgentClass>");
        ctx.set(
            "class",
            result
                .map(|(_, r)| r.to_string())
                .unwrap_or_else(|| "None".to_owned()),
        );
        ctx.set(
            "remaining_ticks",
            result.map(|(r, _)| r as i32).unwrap_or(0),
        );
        BehaviorResult::Success
    }
}

pub(super) struct CancelSpawnTask;

impl BehaviorNode for CancelSpawnTask {
    fn tick(
        &mut self,
        arg: BehaviorCallback,
        _ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let result = arg(self)
            .and_then(|a| a.downcast_ref::<bool>().copied())
            .expect("CancelSpawnTask should return a bool");
        if result {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}
