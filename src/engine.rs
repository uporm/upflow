use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use crate::model::WorkflowDefinition;
use crate::nodes::{NodeExecutor, start::StartNode, decision::DecisionNode, subflow::SubflowNode, end::EndNode};
use crate::error::WorkflowError;
use crate::executor::WorkflowRunner;
use crate::context::WorkflowContext;

pub struct WorkflowEngine {
    defs: RwLock<HashMap<String, Arc<WorkflowDefinition>>>,
    executors: RwLock<HashMap<String, Arc<dyn NodeExecutor>>>,
}

impl WorkflowEngine {
    pub fn global() -> &'static Self {
        static INSTANCE: once_cell::sync::OnceCell<WorkflowEngine> = once_cell::sync::OnceCell::new();
        INSTANCE.get_or_init(|| {
            let mut execs: HashMap<String, Arc<dyn NodeExecutor>> = HashMap::new();
            execs.insert("start".to_string(), Arc::new(StartNode));
            execs.insert("DECISION".to_string(), Arc::new(DecisionNode));
            execs.insert("subflow".to_string(), Arc::new(SubflowNode));
            execs.insert("end".to_string(), Arc::new(EndNode));

            Self {
                defs: RwLock::new(HashMap::new()),
                executors: RwLock::new(execs),
            }
        })
    }

    pub fn load(&self, id: &str, json: &str) -> Result<(), WorkflowError> {
        let def: WorkflowDefinition = serde_json::from_str(json)?;
        self.defs.write().unwrap().insert(id.to_string(), Arc::new(def));
        Ok(())
    }

    pub async fn run(&self, id: &str) -> Result<(), WorkflowError> {
        self.run_with_context(id, Arc::new(WorkflowContext::new(0))).await
    }

    pub async fn run_with_context(&self, id: &str, ctx: Arc<WorkflowContext>) -> Result<(), WorkflowError> {
        let def = self.defs.read().unwrap().get(id).cloned()
            .ok_or(WorkflowError::WorkflowNotFound(id.to_string()))?;

        let runner = WorkflowRunner::new(def);
        runner.run(ctx).await
    }

    pub fn get_executor(&self, n_type: &str) -> Option<Arc<dyn NodeExecutor>> {
        self.executors.read().unwrap().get(n_type).cloned()
    }

    pub fn register(&self, n_type: &str, exec: Arc<dyn NodeExecutor>) {
        self.executors.write().unwrap().insert(n_type.to_string(), exec);
    }
}