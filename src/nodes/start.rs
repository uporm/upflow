use async_trait::async_trait;
use serde_json::Value;
use crate::model::error::WorkflowError;
use crate::nodes::executor::{NodeContext, NodeExecutor};

pub struct StartNode;

#[async_trait]
impl NodeExecutor for StartNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let mut out = serde_json::Map::new();
        if let Some(inputs) = ctx.resolved_input.get("inputs").and_then(|v| v.as_array()) {
            for input in inputs {
                if let Some(var) = input.get("var").and_then(|v| v.as_str()) {
                    let value = ctx
                        .flow_context
                        .resolve_value(&Value::String(format!("{{{{input.{}}}}}", var)))?;
                    out.insert(var.to_string(), value);
                }
            }
        } else if let Value::Object(map) = ctx.flow_context.payload.clone() {
            out.extend(map);
        }
        Ok(Value::Object(out))
    }

    fn validate(&self, data: &Value) -> Result<(), WorkflowError> {
        if let Some(inputs) = data.get("inputs") {
            let inputs_arr = inputs
                .as_array()
                .ok_or_else(|| WorkflowError::ValidationError("inputs must be an array".to_string()))?;

            for input in inputs_arr {
                if !input.is_object() {
                    return Err(WorkflowError::ValidationError(
                        "input item must be an object".to_string(),
                    ));
                }
                if input.get("var").is_none() {
                    return Err(WorkflowError::ValidationError(
                        "input item must have a 'var' field".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }
}
