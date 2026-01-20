use async_trait::async_trait;
use serde_json::{json, Value};

use crate::model::WorkflowError;
use crate::nodes::executor::{NodeContext, NodeExecutor};

pub struct DecisionNode;

#[async_trait]
impl NodeExecutor for DecisionNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let data = ctx.resolved_input;
        let conditions = data
            .get("conditions")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        for cond in conditions {
            if let Some(id) = cond.get("id").and_then(|v| v.as_str()) {
                if let Some(rules) = cond.get("rules").and_then(|v| v.as_array()) {
                    let mut pass = true;
                    for rule in rules {
                        let var = rule.get("variable").cloned().unwrap_or(Value::Null);
                        let op = rule
                            .get("operator")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        let val = rule.get("value").cloned().unwrap_or(Value::Null);
                        let resolved = match var {
                            Value::String(s) => ctx.flow_context.resolve_value(&Value::String(s))?,
                            other => other,
                        };
                        pass &= compare(&resolved, op, &val);
                    }
                    if pass {
                        return Ok(json!({ "selected": id }));
                    }
                } else {
                    return Ok(json!({ "selected": id }));
                }
            }
        }
        Ok(json!({ "selected": "default" }))
    }
}

fn compare(left: &Value, op: &str, right: &Value) -> bool {
    match op {
        ">" => left.as_f64().unwrap_or(0.0) > right.as_f64().unwrap_or(0.0),
        ">=" => left.as_f64().unwrap_or(0.0) >= right.as_f64().unwrap_or(0.0),
        "<" => left.as_f64().unwrap_or(0.0) < right.as_f64().unwrap_or(0.0),
        "<=" => left.as_f64().unwrap_or(0.0) <= right.as_f64().unwrap_or(0.0),
        "==" => left == right,
        "!=" => left != right,
        _ => false,
    }
}
