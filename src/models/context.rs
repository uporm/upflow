use crate::models::error::WorkflowError;
use crate::models::event::WorkflowEvent;
use crate::models::event_bus::EventBus;
use dashmap::DashMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use crate::models::workflow::Node;

#[derive(Clone)]
pub struct FlowContext {
    pub payload: Value,
    pub node_results: Arc<DashMap<String, Value>>,
    pub env: Arc<HashMap<String, Value>>,
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
            node_results: Arc::new(DashMap::new()),
            env: Arc::new(HashMap::new()),
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_env(mut self, env: HashMap<String, Value>) -> Self {
        self.env = Arc::new(env);
        self
    }

    pub fn set_result(&self, node_id: &str, output: Value) {
        self.node_results.insert(node_id.to_string(), output);
    }

    pub fn get_result(&self, node_id: &str) -> Option<Value> {
        self.node_results.get(node_id).map(|v| v.value().clone())
    }

    pub fn resolve_value(&self, value: &Value) -> Result<Value, WorkflowError> {
        crate::utils::resolve_value(self, value)
    }
}

pub struct NodeContext {
    pub node: Node,
    pub flow_context: FlowContext,
    pub event_bus: EventBus,
}

impl NodeContext {
    pub fn send_message(&self, message: impl Into<Value>) {
        self.event_bus.emit(WorkflowEvent::NodeMessage {
            node_id: self.node.id.clone(),
            node_type: self.node.node_type.clone(),
            message: message.into(),
        });
    }
}
