use crate::models::error::WorkflowError;
use crate::models::event::WorkflowEvent;
use crate::models::event_bus::EventBus;
use crate::models::workflow::Node;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct FlowContext {
    pub payload: Value,
    pub results: DashMap<String, Arc<Value>>,
}

impl Default for FlowContext {
    fn default() -> Self {
        Self::new()
    }
}

impl FlowContext {
    pub fn new() -> Self {
        Self {
            payload: Value::Null,
            results: DashMap::new(),
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_vars(
        self,
        vars: impl IntoIterator<Item = (String, Value)>,
    ) -> Result<Self, WorkflowError> {
        for (key, value) in vars {
            validate_var_key(&key)?;
            self.results.insert(key, Arc::new(value));
        }
        Ok(self)
    }

    pub fn set_result(&self, key: &str, output: Arc<Value>) {
        self.results.insert(key.to_string(), output);
    }

    pub fn get_result(&self, key: &str) -> Option<Arc<Value>> {
        self.results
            .get(key)
            .map(|v| Arc::clone(v.value()))
    }
    
    pub fn get_results(&self) -> HashMap<String, Arc<Value>> {
        self.results
            .iter()
            .map(|item| (item.key().clone(), Arc::clone(item.value())))
            .collect()
    }

    pub fn resolve_value(&self, value: &Value) -> Result<Value, WorkflowError> {
        crate::utils::resolve_value(self, value)
    }
}

fn validate_var_key(key: &str) -> Result<(), WorkflowError> {
    if key.contains('.') {
        return Err(WorkflowError::ValidationError(format!(
            "FlowContext variable key '{}' cannot contain '.'",
            key
        )));
    }

    if key
        .chars()
        .next()
        .is_some_and(|first_char| first_char.is_ascii_digit())
    {
        return Err(WorkflowError::ValidationError(format!(
            "FlowContext variable key '{}' cannot start with a digit",
            key
        )));
    }

    Ok(())
}

pub struct NodeContext {
    pub node: Node,
    pub flow_context: Arc<FlowContext>,
    pub event_bus: EventBus,
    pub resolved_data: Arc<Value>,
    pub next_nodes: Arc<Vec<Node>>,
}

impl NodeContext {
    pub fn next_nodes(&self) -> Vec<Node> {
        self.next_nodes.as_ref().clone()
    }

    pub fn next_node(&self) -> Option<Node> {
        self.next_nodes.first().cloned()
    }

    pub fn send_message(&self, message: impl Into<Value>) {
        self.event_bus.emit(WorkflowEvent::NodeMessage {
            node_id: self.node.id.clone(),
            node_type: self.node.node_type.clone(),
            data: Arc::clone(&self.resolved_data),
            message: Arc::new(message.into()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::event_bus::EventBus;
    use crate::models::workflow::Node;
    use serde_json::json;

    #[test]
    fn test_node_context_next_nodes() {
        let current_node = Node {
            id: "node-1".to_string(),
            parent_id: None,
            node_type: "task".to_string(),
            data: Arc::new(json!({})),
            retry_policy: None,
        };
        let next_node_a = Node {
            id: "node-2".to_string(),
            parent_id: None,
            node_type: "task".to_string(),
            data: Arc::new(json!({})),
            retry_policy: None,
        };
        let next_node_b = Node {
            id: "node-3".to_string(),
            parent_id: None,
            node_type: "task".to_string(),
            data: Arc::new(json!({})),
            retry_policy: None,
        };
        let ctx = NodeContext {
            node: current_node,
            flow_context: Arc::new(FlowContext::new()),
            event_bus: EventBus::new(10),
            resolved_data: Arc::new(json!({})),
            next_nodes: Arc::new(vec![next_node_a.clone(), next_node_b.clone()]),
        };
        let next_ids = ctx
            .next_nodes()
            .into_iter()
            .map(|node| node.id)
            .collect::<Vec<_>>();
        assert_eq!(next_ids, vec![next_node_a.id, next_node_b.id]);
        assert_eq!(ctx.next_node().map(|node| node.id), Some("node-2".to_string()));
    }

    #[test]
    fn test_with_vars_rejects_dot_key() {
        let result = FlowContext::new().with_vars(vec![("user.name".to_string(), json!("Jason"))]);
        match result {
            Err(WorkflowError::ValidationError(message)) => {
                assert!(message.contains("cannot contain '.'"))
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn test_with_vars_rejects_numeric_prefix_key() {
        let result = FlowContext::new().with_vars(vec![("1name".to_string(), json!("Jason"))]);
        match result {
            Err(WorkflowError::ValidationError(message)) => {
                assert!(message.contains("cannot start with a digit"))
            }
            _ => panic!("expected validation error"),
        }
    }

    #[test]
    fn test_with_vars_accepts_valid_key() {
        let ctx = FlowContext::new()
            .with_vars(vec![("user_name".to_string(), json!("Jason"))])
            .unwrap();
        let value = ctx.get_result("user_name").unwrap();
        assert_eq!(value.as_ref(), &json!("Jason"));
    }
}
