use crate::core::constants::GROUP_FLOW_ID_KEY;
use crate::engine::engine::WorkflowEngine;
use crate::models::context::NodeContext;
use crate::models::error::WorkflowError;
use crate::nodes::NodeExecutor;
use async_trait::async_trait;
use serde_json::Value;

pub struct GroupNode;

#[async_trait]
impl NodeExecutor for GroupNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved_input = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        let group_flow_id = resolved_input
            .get(GROUP_FLOW_ID_KEY)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if group_flow_id.is_empty() {
            return Err(WorkflowError::RuntimeError(
                "missing groupFlowId".to_string(),
            ));
        }
        let result = WorkflowEngine::global()
            .run_with_ctx_event(
                &group_flow_id,
                ctx.flow_context.clone(),
                ctx.event_bus.clone(),
            )
            .await?;
        Ok(result.output.unwrap_or(Value::Null))
    }
}
