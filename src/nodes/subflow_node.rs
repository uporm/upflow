use crate::engine::engine::WorkflowEngine;
use crate::models::context::{FlowContext, NodeContext};
use crate::models::error::WorkflowError;
use crate::nodes::NodeExecutor;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

pub struct SubflowNode;

#[async_trait]
impl NodeExecutor for SubflowNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved_input = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        let subflow_id = resolved_input
            .get("subflowId")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if subflow_id.is_empty() {
            return Err(WorkflowError::RuntimeError("missing subflowId".to_string()));
        }
        let payload = resolved_input
            .get("input")
            .cloned()
            .unwrap_or_else(|| ctx.flow_context.payload.clone());
        let env = ctx.flow_context.env.clone();
        let subflow_context = Arc::new(FlowContext::new().with_payload(payload).with_env(env));
        let result = WorkflowEngine::global()
            .run_with_ctx_event(&subflow_id, subflow_context, ctx.event_bus.clone())
            .await?;
        Ok(result.output.unwrap_or(Value::Null))
    }
}
