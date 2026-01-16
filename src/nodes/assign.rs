use crate::model::{node_trait::{NodeExecutor, NodeOutput}, workflow::Node, context::WorkflowContext};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::{json, Value};

pub struct AssignNode;

#[async_trait]
impl NodeExecutor for AssignNode {
    async fn execute(&self, node: &Node, ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        let var = node.data.get("var").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("assign missing var"))?;
        let value: Value = node.data.get("value").cloned().unwrap_or(Value::Null);
        ctx.set_var(var, value);
        Ok(NodeOutput { output: json!({"assign": "ok", "var": var}), matched_handles: None })
    }
}
