use crate::model::{node_trait::{NodeExecutor, NodeOutput}, workflow::Node, context::WorkflowContext};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

pub struct LoopNode;

#[async_trait]
impl NodeExecutor for LoopNode {
    async fn execute(&self, node: &Node, _ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        let next = node.data.get("next").and_then(|v| v.as_str()).unwrap_or("next");
        Ok(NodeOutput { output: json!({"loop": "ok"}), matched_handles: Some(vec![next.to_string()]) })
    }
}
