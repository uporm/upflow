use crate::model::{node_trait::{NodeExecutor, NodeOutput}, workflow::Node, context::WorkflowContext};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

pub struct EndNode;

#[async_trait]
impl NodeExecutor for EndNode {
    async fn execute(&self, _node: &Node, _ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        Ok(NodeOutput { output: json!({"end": "ok"}), matched_handles: None })
    }
}
