use async_trait::async_trait;
use serde_json::Value;
use anyhow::Result;
use crate::model::{workflow::Node, context::WorkflowContext};

#[derive(Debug, Clone)]
pub struct NodeOutput {
    pub output: Value,
    pub matched_handles: Option<Vec<String>>, // Case 分支路由
}

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, node: &Node, ctx: &mut WorkflowContext) -> Result<NodeOutput>;
}
