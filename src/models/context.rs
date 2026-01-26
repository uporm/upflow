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
    pub node_results: DashMap<String, Arc<Value>>,
    pub env: HashMap<String, Value>,
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
            env: HashMap::new(),
        }
    }

    pub fn with_payload(mut self, payload: Value) -> Self {
        self.payload = payload;
        self
    }

    pub fn with_env(mut self, env: HashMap<String, Value>) -> Self {
        self.env = env;
        self
    }

    pub fn set_result(&self, node_id: &str, output: Arc<Value>) {
        self.node_results.insert(node_id.to_string(), output);
    }

    pub fn get_result(&self, node_id: &str) -> Option<Arc<Value>> {
        self.node_results.get(node_id).map(|v| Arc::clone(v.value()))
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
