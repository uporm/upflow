use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::registry::{workflow_registry::WorkflowRegistry, node_registry::NodeRegistry};
use crate::engine::events::EventBus;

#[derive(Clone)]
pub struct WorkflowContext {
    vars: Arc<RwLock<HashMap<String, Value>>>,
    wf_registry: Option<Arc<WorkflowRegistry>>,
    node_registry: Option<Arc<NodeRegistry>>,
    event_bus: Option<EventBus>,
}

impl WorkflowContext {
    pub fn new() -> Self {
        Self {
            vars: Arc::new(RwLock::new(HashMap::new())),
            wf_registry: None,
            node_registry: None,
            event_bus: None,
        }
    }

    pub fn set_var(&self, k: impl Into<String>, v: Value) {
        self.vars.write().insert(k.into(), v);
    }

    pub fn get_var(&self, k: &str) -> Option<Value> {
        self.vars.read().get(k).cloned()
    }

    pub fn set_env(&mut self, wr: Arc<WorkflowRegistry>, nr: Arc<NodeRegistry>, eb: EventBus) {
        self.wf_registry = Some(wr);
        self.node_registry = Some(nr);
        self.event_bus = Some(eb);
    }

    pub fn get_wf_registry(&self) -> Option<Arc<WorkflowRegistry>> {
        self.wf_registry.clone()
    }

    pub fn get_node_registry(&self) -> Option<Arc<NodeRegistry>> {
        self.node_registry.clone()
    }

    pub fn get_event_bus(&self) -> Option<EventBus> {
        self.event_bus.clone()
    }

    pub fn isolated_clone(&self) -> Self {
        let new_vars = self.vars.read().clone();
        Self {
            vars: Arc::new(RwLock::new(new_vars)),
            wf_registry: self.wf_registry.clone(),
            node_registry: self.node_registry.clone(),
            event_bus: self.event_bus.clone(),
        }
    }
}
impl Default for WorkflowContext {
    fn default() -> Self { Self::new() }
}
