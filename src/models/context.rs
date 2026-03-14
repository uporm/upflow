use crate::models::error::WorkflowError;
use crate::models::event::WorkflowEvent;
use crate::models::event_bus::EventBus;
use crate::models::workflow::Node;
use dashmap::DashMap;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone)]
pub struct FlowContext {
    pub payload: Value,
    pub node_results: DashMap<String, Arc<Value>>,
    pub env: DashMap<String, Value>,
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
            node_results: DashMap::new(),
            env: DashMap::new(),
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_env(mut self, env: impl IntoIterator<Item = (String, Value)>) -> Self {
        self.env = env.into_iter().collect();
        self
    }

    pub fn update_environment(&self, key: &str, value: Value) -> Result<(), WorkflowError> {
        if key.starts_with("session.") {
            self.env.insert(key.to_string(), value);
            Ok(())
        } else {
            Err(WorkflowError::RuntimeError(format!(
                "Cannot update environment variable '{}'. Only variables starting with 'session.' can be updated.",
                key
            )))
        }
    }

    pub fn set_result(&self, node_id: &str, output: Arc<Value>) {
        self.node_results.insert(node_id.to_string(), output);
    }

    pub fn get_result(&self, node_id: &str) -> Option<Arc<Value>> {
        self.node_results
            .get(node_id)
            .map(|v| Arc::clone(v.value()))
    }

    pub fn resolve_value(&self, value: &Value) -> Result<Value, WorkflowError> {
        crate::utils::resolve_value(self, value)
    }
}

pub struct NodeContext {
    pub node: Node,
    pub flow_context: Arc<FlowContext>,
    pub event_bus: EventBus,
    pub resolved_data: Arc<Value>,
}

impl NodeContext {
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
    use serde_json::json;

    #[test]
    fn test_update_environment_allowed() {
        let ctx = FlowContext::new();
        let result = ctx.update_environment("session.user_id", json!("12345"));
        assert!(result.is_ok());
        assert_eq!(
            ctx.env.get("session.user_id").map(|v| v.value().clone()),
            Some(json!("12345"))
        );
    }

    #[test]
    fn test_update_environment_disallowed() {
        let ctx = FlowContext::new();
        let result = ctx.update_environment("system.config", json!("value"));
        assert!(result.is_err());
        assert!(ctx.env.get("system.config").is_none());
    }

    #[test]
    fn test_update_environment_overwrite() {
        let ctx = FlowContext::new();
        let _ = ctx.update_environment("session.user_id", json!("12345"));
        let result = ctx.update_environment("session.user_id", json!("67890"));
        assert!(result.is_ok());
        assert_eq!(
            ctx.env.get("session.user_id").map(|v| v.value().clone()),
            Some(json!("67890"))
        );
    }
}
