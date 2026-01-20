use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

pub use crate::engine::event_bus::EventBus;
use crate::engine::scheduler::Scheduler;
use crate::model::{Workflow, WorkflowError, WorkflowResult};
use crate::nodes::executor::NodeExecutor;

pub mod event_bus;
pub mod scheduler;

struct WorkflowCache {
    hash: String,
    def: Arc<Workflow>,
}

pub struct WorkflowEngine {
    registry: Arc<RwLock<HashMap<String, Arc<dyn NodeExecutor>>>>,
    workflows: RwLock<HashMap<String, WorkflowCache>>,
    instance_tokens: Mutex<HashMap<String, CancellationToken>>,
    counter: AtomicU64,
}

impl WorkflowEngine {
    pub fn global() -> &'static WorkflowEngine {
        static INSTANCE: OnceLock<WorkflowEngine> = OnceLock::new();
        INSTANCE.get_or_init(|| WorkflowEngine {
            registry: Arc::new(RwLock::new(HashMap::new())),
            workflows: RwLock::new(HashMap::new()),
            instance_tokens: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(1),
        })
    }

    pub fn register(&self, node_type: &str, executor: impl NodeExecutor + 'static) {
        let mut registry = self.registry.write().unwrap();
        registry.insert(node_type.to_string(), Arc::new(executor));
    }

    pub fn executor(&self, node_type: &str) -> Result<Arc<dyn NodeExecutor>, WorkflowError> {
        let registry = self.registry.read().unwrap();
        registry
            .get(node_type)
            .cloned()
            .ok_or_else(|| WorkflowError::NodeExecutorNotFound(node_type.to_string()))
    }

    pub fn load(&self, workflow_id: &str, json: &str) -> Result<(), WorkflowError> {
        let hash = compute_hash(json);
        let mut workflows = self.workflows.write().unwrap();
        if let Some(cache) = workflows.get(workflow_id) {
            if cache.hash == hash {
                return Ok(());
            }
        }
        let def: Workflow =
            serde_json::from_str(json).map_err(|e| WorkflowError::ParseError(e.to_string()))?;
        workflows.insert(
            workflow_id.to_string(),
            WorkflowCache {
                hash,
                def: Arc::new(def),
            },
        );
        Ok(())
    }

    pub async fn run(
        &self,
        workflow_id: &str,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_input(workflow_id, event_bus, Value::Null)
            .await
    }

    pub async fn run_with_input(
        &self,
        workflow_id: &str,
        event_bus: EventBus,
        input: Value,
    ) -> Result<WorkflowResult, WorkflowError> {
        let (instance_id, handle) = self.start(workflow_id, event_bus, input)?;
        let result = handle.await.map_err(|e| WorkflowError::RuntimeError(e.to_string()))?;
        let mut map = self.instance_tokens.lock().unwrap();
        map.remove(&instance_id);
        result
    }

    pub fn start(
        &self,
        workflow_id: &str,
        event_bus: EventBus,
        input: Value,
    ) -> Result<(String, tokio::task::JoinHandle<Result<WorkflowResult, WorkflowError>>), WorkflowError>
    {
        let def = {
            let workflows = self.workflows.read().unwrap();
            workflows
                .get(workflow_id)
                .map(|c| c.def.clone())
                .ok_or_else(|| WorkflowError::WorkflowNotFound(workflow_id.to_string()))?
        };
        let instance_id = self.next_instance_id();
        let token = CancellationToken::new();
        {
            let mut map = self.instance_tokens.lock().unwrap();
            map.insert(instance_id.clone(), token.clone());
        }
        let scheduler = Scheduler {
            registry: self.registry.clone(),
        };
        let instance_id_clone = instance_id.clone();
        let handle = tokio::spawn(async move {
            scheduler
                .execute(def, event_bus, input, token.clone(), instance_id_clone)
                .await
        });
        Ok((instance_id, handle))
    }

    pub fn stop(&self, instance_id: &str) {
        let map = self.instance_tokens.lock().unwrap();
        if let Some(token) = map.get(instance_id) {
            token.cancel();
        }
    }

    fn next_instance_id(&self) -> String {
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        format!("wf-instance-{}", id)
    }
}

fn compute_hash(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let out = hasher.finalize();
    hex::encode(out)
}
