use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use crate::model::workflow::Workflow;
use anyhow::Result;

#[derive(Default)]
pub struct WorkflowRegistry {
    map: RwLock<HashMap<String, Arc<Workflow>>>,
}

impl WorkflowRegistry {
    pub fn new() -> Self {
        Self { map: RwLock::new(HashMap::new()) }
    }

    pub fn register(&self, id: impl Into<String>, wf: Arc<Workflow>) {
        self.map.write().insert(id.into(), wf);
    }

    pub fn register_str(&self, id: impl Into<String>, json: &str) -> Result<()> {
        let wf: Workflow = serde_json::from_str(json)?;
        self.register(id, Arc::new(wf));
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<Arc<Workflow>> {
        self.map.read().get(id).cloned()
    }
}
