use crate::context::WorkflowContext;
use crate::error::WorkflowError;
use async_trait::async_trait;
use serde_json::Value;
use crate::model::NodeOutput;
use crate::nodes::node_trait::NodeExecutor;

pub struct DecisionNode;

#[async_trait]
impl NodeExecutor for DecisionNode {
    async fn execute(&self, ctx: &WorkflowContext, data: &Value) -> Result<NodeOutput, WorkflowError> {
        let cases = data["cases"].as_array()
            .ok_or_else(|| WorkflowError::ConfigError("Missing cases".into()))?;

        for case in cases {
            let case_id = case["id"].as_str().unwrap_or_default();
            let opr_type = case["opr"].as_str().unwrap_or("and");
            let conditions = case["conditions"].as_array()
                .ok_or_else(|| WorkflowError::ConfigError("Missing conditions".into()))?;

            let mut case_matched = opr_type == "and";

            for cond in conditions {
                let var_path = cond["varId"].as_str().ok_or(WorkflowError::ConfigError("No varId".into()))?;
                let actual = ctx.get_var(var_path).unwrap_or(Value::Null);
                let target = &cond["value"];

                let op = cond["opr"].as_str().unwrap_or("eq");
                let is_match = match op {
                    "in" => {
                        if let Some(arr) = target.as_array() {
                            arr.contains(&actual)
                        } else if let (Some(a), Some(b)) = (actual.as_str(), target.as_str()) {
                            b.contains(a)
                        } else {
                            false
                        }
                    }
                    "eq" => &actual == target,
                    "gt" => {
                        if let (Some(a), Some(b)) = (actual.as_f64(), target.as_f64()) {
                            a > b
                        } else {
                            false
                        }
                    }
                    _ => false,
                };

                if opr_type == "and" {
                    if !is_match {
                        case_matched = false;
                        break;
                    }
                } else { // or
                    if is_match {
                        case_matched = true;
                        break;
                    }
                }
            }

            if case_matched {
                return Ok(NodeOutput {
                    next_handle: Some(case_id.to_string()),
                    ..Default::default()
                });
            }
        }
        Ok(NodeOutput::default())
    }
}