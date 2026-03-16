use crate::models::context::NodeContext;
use crate::models::error::WorkflowError;
use crate::nodes::NodeExecutor;
use async_trait::async_trait;
use serde_json::{Value, json};

pub struct DecisionNode;

#[async_trait]
impl NodeExecutor for DecisionNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved_input = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        let branches = resolved_input
            .get("cases")
            .or_else(|| resolved_input.get("branches"))
            .and_then(|v| v.as_array());

        if let Some(branches) = branches {
            for branch in branches {
                let conditions = branch.get("conditions").and_then(|v| v.as_array());
                let logic = branch
                    .get("logic")
                    .and_then(|v| v.as_str())
                    .unwrap_or("and");

                if let Some(conds) = conditions {
                    // Logic: "and" (all match) or "or" (any match)
                    let mut is_match = logic != "or";

                    if conds.is_empty() {
                        // Empty conditions:
                        // if logic is "and", usually means true (vacuously true).
                        // if logic is "or", usually means false.
                        // But for a decision branch, if no conditions are specified, maybe it shouldn't match?
                        // However, keeping consistent with boolean algebra:
                        // AND [] -> true
                        // OR [] -> false
                        // Let's stick to the initialization values.
                    } else {
                        for cond in conds {
                            let var = cond.get("var").and_then(|v| v.as_str()).unwrap_or("");
                            let opr = cond.get("opr").and_then(|v| v.as_str()).unwrap_or("eq");
                            let value = cond.get("value").and_then(|v| v.as_str()).unwrap_or("");

                            let result = match opr {
                                "eq" => var == value,
                                "ne" => var != value,
                                "gt" => var > value,
                                "ge" => var >= value,
                                "lt" => var < value,
                                "le" => var <= value,
                                "in" => value.contains(var), // checking if value (string) contains var (substring)
                                "contains" => var.contains(value), // var contains value
                                _ => var == value,
                            };

                            if logic == "or" {
                                if result {
                                    is_match = true;
                                    break;
                                }
                            } else {
                                // AND
                                if !result {
                                    is_match = false;
                                    break;
                                }
                            }
                        }
                    }

                    if is_match {
                        let handle = branch
                            .get("handle")
                            .and_then(|v| v.as_str())
                            .unwrap_or("default");
                        let mut map = serde_json::Map::new();
                        map.insert(handle.to_string(), json!({}));
                        return Ok(Value::Object(map));
                    }
                }
            }
        }

        // Default case
        let else_branch = resolved_input.get("else");
        let default_handle = if let Some(else_branch) = else_branch {
            else_branch
                .get("handle")
                .and_then(|v| v.as_str())
                .unwrap_or("default")
        } else {
            "default"
        };

        let mut map = serde_json::Map::new();
        map.insert(default_handle.to_string(), json!({}));
        Ok(Value::Object(map))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::context::FlowContext;
    use crate::models::event_bus::EventBus;
    use crate::models::workflow::Node;
    use serde_json::json;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_decision_node_in_opr() {
        let node_executor = DecisionNode;
        let data = json!({
            "title": "条件分支",
            "branches": [
            {
                "logic": "and",
                "conditions": [
                {
                    "var": "张三", // Simulated resolved value
                    "opr": "in",
                    "value": "张三"
                }
                ],
                "handle": "zhangsan"
            }
            ],
            "default": {
                "handle": "default"
            }
        });

        let node = Node {
            id: "decision_node".to_string(),
            node_type: "decision".to_string(),
            data: Arc::new(data.clone()),
            parent_id: None,
            retry_policy: None,
        };

        let ctx = NodeContext {
            instance_id: "test-instance".to_string(),
            node,
            flow_context: Arc::new(FlowContext::new()),
            event_bus: EventBus::new(10),
            resolved_data: Arc::new(data.clone()),
            next_nodes: Arc::new(Vec::new()),
        };

        let result = node_executor.execute(ctx).await.unwrap();
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("zhangsan"));
    }

    #[tokio::test]
    async fn test_decision_node_default() {
        let node_executor = DecisionNode;
        let data = json!({
            "title": "条件分支",
            "branches": [
            {
                "logic": "and",
                "conditions": [
                {
                    "var": "李四",
                    "opr": "in",
                    "value": "张三"
                }
                ],
                "handle": "zhangsan"
            }
            ],
            "default": {
                "handle": "default"
            }
        });

        let node = Node {
            id: "decision_node".to_string(),
            node_type: "decision".to_string(),
            data: Arc::new(data.clone()),
            parent_id: None,
            retry_policy: None,
        };

        let ctx = NodeContext {
            instance_id: "test-instance".to_string(),
            node,
            flow_context: Arc::new(FlowContext::new()),
            event_bus: EventBus::new(10),
            resolved_data: Arc::new(data.clone()),
            next_nodes: Arc::new(Vec::new()),
        };

        let result = node_executor.execute(ctx).await.unwrap();
        let obj = result.as_object().unwrap();
        assert!(obj.contains_key("default"));
    }
}
