use std::collections::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use crate::model::node_trait::NodeExecutor;

pub type NodeExecutorRef = Arc<dyn NodeExecutor>;

#[derive(Default)]
pub struct NodeRegistry {
    map: RwLock<HashMap<String, NodeExecutorRef>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self { map: RwLock::new(HashMap::new()) }
    }

    pub fn register<T: NodeExecutor + 'static>(&self, node_type: &str, executor: T) {
        self.map.write().insert(node_type.to_string(), Arc::new(executor));
    }

    pub fn get(&self, node_type: &str) -> Option<NodeExecutorRef> {
        self.map.read().get(node_type).cloned()
    }
}
