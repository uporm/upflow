use crate::model::{node_trait::{NodeExecutor, NodeOutput}, workflow::Node, context::WorkflowContext};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;

pub struct StartNode;

#[async_trait]
impl NodeExecutor for StartNode {
    async fn execute(&self, _node: &Node, _ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        Ok(NodeOutput { output: json!({"start": "ok"}), matched_handles: None })
    }
}
