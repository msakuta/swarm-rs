//! An adapter functions and types for behavior_tree_lite

use behavior_tree_lite::{
    boxify, BehaviorCallback, BehaviorNode, BehaviorNodeContainer, BehaviorResult, NumChildren,
    PortSpec, Registry,
};

use crate::qtree::QTreePathNode;

/// Boundary to skip Debug trait from propagating to BehaviorNode trait
pub struct BehaviorTree(pub BehaviorNodeContainer);

impl std::fmt::Debug for BehaviorTree {
    fn fmt(&self, _f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Result::Ok(())
    }
}

pub(super) fn common_tree_nodes(registry: &mut Registry) {
    registry.register("StringEq", boxify(|| StringEqNode));
    registry.register("Gt", boxify(|| GtNode));
    registry.register("Ge", boxify(|| GeNode));
    registry.register("Print", boxify(|| PrintNode));
    registry.register("GetResource", boxify(|| GetResource));
    registry.register("Throttle", boxify(|| ThrottleNode::default()));
}

/// Because behavior-tree-lite doesn't support string variables in expressions, we need a silly node like this.
struct StringEqNode;

impl BehaviorNode for StringEqNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("lhs"), PortSpec::new_in("rhs")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let lhs = ctx.get::<String>("lhs");
        let rhs = ctx.get::<String>("rhs");
        if lhs.zip(rhs).map(|(lhs, rhs)| lhs == rhs).unwrap_or(false) {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

struct GtNode;

impl BehaviorNode for GtNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("lhs"), PortSpec::new_in("rhs")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let lhs = ctx.get_parse::<i32>("lhs");
        let rhs = ctx.get_parse::<i32>("rhs");
        if lhs.zip(rhs).map(|(lhs, rhs)| lhs > rhs).unwrap_or(false) {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

struct GeNode;

impl BehaviorNode for GeNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("lhs"), PortSpec::new_in("rhs")]
    }

    fn tick(
        &mut self,
        _arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let lhs = ctx.get_parse::<i32>("lhs");
        let rhs = ctx.get_parse::<i32>("rhs");
        if lhs.zip(rhs).map(|(lhs, rhs)| lhs >= rhs).unwrap_or(false) {
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(crate) struct PrintCommand(pub String);

/// A node with string output with interpolation.
struct PrintNode;

pub(super) struct GetIdCommand;

impl BehaviorNode for PrintNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![
            PortSpec::new_in("input"),
            PortSpec::new_in("arg0"),
            PortSpec::new_in("arg1"),
        ]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        if let Some(result) = ctx.get::<String>("input") {
            let get_string = |key| {
                ctx.get::<String>(key)
                    .cloned()
                    .or_else(|| ctx.get::<bool>(key).map(|v| v.to_string()))
                    .or_else(|| ctx.get::<i32>(key).map(|v| v.to_string()))
                    .or_else(|| ctx.get::<f64>(key).map(|v| v.to_string()))
                    .or_else(|| ctx.get::<[i32; 2]>(key).map(|v| format!("{:?}", v)))
                    .or_else(|| ctx.get::<[f64; 2]>(key).map(|v| format!("{:?}", v)))
                    .or_else(|| {
                        ctx.get::<Vec<QTreePathNode>>(key)
                            .map(|v| format!("{:?}", v))
                    })
            };
            let arg0 = get_string("arg0");
            let arg1 = get_string("arg1");
            let result = match (arg0, arg1) {
                (Some(arg0), Some(arg1)) => {
                    result.replacen("{}", &arg0, 1).replacen("{}", &arg1, 1)
                }
                (Some(arg0), None) => result.replacen("{}", &arg0, 1),
                _ => result.clone(),
            };
            if let Some(id) = arg(&GetIdCommand).and_then(|res| res.downcast::<usize>().ok()) {
                println!("PrintNode({}): {result:?}", *id);
            } else {
                println!("PrintNode(?): {result:?}");
            }
            arg(&PrintCommand(result));
            BehaviorResult::Success
        } else {
            BehaviorResult::Fail
        }
    }
}

pub(crate) struct GetResource;

impl BehaviorNode for GetResource {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_out("output")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let resource = *arg(self)
            .and_then(|res| res.downcast::<i32>().ok())
            .expect("Resource should be always available");

        ctx.set("output", resource);
        BehaviorResult::Success
    }
}

/// Throttle the execution of the child node by a given number of ticks. Useful to limit frequency of calling expensive operations.
///
/// For example, the following behavior will print "Hello" once every 20 ticks.
///
/// ```txt
/// Throttle (time <- "20") {
///     Print (input <- "Hello")
/// }
/// ```
///
/// If the child node fails, this node will fail too. Wrap the child node in a `ForceSuccess` to suppress this behavior.
#[derive(Default)]
struct ThrottleNode {
    timer: Option<usize>,
}

impl BehaviorNode for ThrottleNode {
    fn provided_ports(&self) -> Vec<PortSpec> {
        vec![PortSpec::new_in("time")]
    }

    fn tick(
        &mut self,
        arg: BehaviorCallback,
        ctx: &mut behavior_tree_lite::Context,
    ) -> BehaviorResult {
        let set_timer = |this: &mut Self| {
            if let Some(input) = ctx.get_parse::<usize>("time") {
                // println!("Timer set! {}", input);
                this.timer = Some(input);
            } else {
                this.timer = None;
            }
        };

        if let Some(ref mut remaining) = self.timer {
            if *remaining == 0 {
                // println!("Timed out");
                set_timer(self);
                let res = ctx.tick_child(0, arg).unwrap_or(BehaviorResult::Fail);
                return res;
            } else {
                *remaining -= 1;
                return BehaviorResult::Success;
            }
        } else {
            set_timer(self);
        }
        BehaviorResult::Success
    }

    fn max_children(&self) -> NumChildren {
        NumChildren::Finite(1)
    }
}
