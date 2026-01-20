use async_trait::async_trait;
use serde_json::Value;

use crate::model::WorkflowError;
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
        } else if let Value::Object(map) = ctx.flow_context.input.clone() {
            out.extend(map);
        }
        Ok(Value::Object(out))
    }
}
