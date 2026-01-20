use async_trait::async_trait;
use serde_json::Value;
use crate::model::error::WorkflowError;
use crate::nodes::executor::{NodeContext, NodeExecutor};

pub struct EndNode;

#[async_trait]
impl NodeExecutor for EndNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        Ok(ctx.resolved_input.clone())
    }
}
