use crate::models::context::NodeContext;
use crate::models::error::WorkflowError;
use crate::nodes::NodeExecutor;
use async_trait::async_trait;
use serde_json::Value;

pub struct StartNode;

#[async_trait]
impl NodeExecutor for StartNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved_input = ctx.flow_context.resolve_value(&ctx.node.data)?;
        Ok(resolved_input)
    }
}
