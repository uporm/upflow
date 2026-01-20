use async_trait::async_trait;
use serde_json::Value;

use crate::engine::WorkflowEngine;
use crate::model::WorkflowError;
use crate::nodes::executor::{NodeContext, NodeExecutor};

pub struct SubflowNode;

#[async_trait]
impl NodeExecutor for SubflowNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let data = ctx.resolved_input;
        let flow_id = data
            .get("flow_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| WorkflowError::RuntimeError("missing flow_id".to_string()))?;
        let mut input_map = serde_json::Map::new();
        if let Some(inputs) = data.get("inputs").and_then(|v| v.as_object()) {
            for (k, v) in inputs {
                let resolved = ctx.flow_context.resolve_value(v)?;
                input_map.insert(k.clone(), resolved);
            }
        }
        let input = Value::Object(input_map);
        let result = WorkflowEngine::global()
            .run_with_input(flow_id, ctx.event_bus.clone(), input)
            .await?;
        Ok(result.output.unwrap_or(Value::Null))
    }
}
