use std::sync::{Arc, RwLock};
use std::collections::HashMap;
use crate::model::WorkflowDefinition;
use crate::nodes::{ start::StartNode, decision::DecisionNode, subflow::SubflowNode, end::EndNode};
use crate::error::WorkflowError;
use crate::executor::WorkflowRunner;
use crate::context::WorkflowContext;
use crate::nodes::node_trait::NodeExecutor;

pub struct WorkflowEngine {
    defs: RwLock<HashMap<String, Arc<WorkflowDefinition>>>,
    executors: RwLock<HashMap<String, Arc<dyn NodeExecutor>>>,
}

impl WorkflowEngine {
    pub fn global() -> &'static Self {
        static INSTANCE: std::sync::OnceLock<WorkflowEngine> = std::sync::OnceLock::new();
        INSTANCE.get_or_init(|| {
            let mut execs: HashMap<String, Arc<dyn NodeExecutor>> = HashMap::new();
            execs.insert("start".to_string(), Arc::new(StartNode));
            execs.insert("decision".to_string(), Arc::new(DecisionNode));
            execs.insert("subflow".to_string(), Arc::new(SubflowNode));
            execs.insert("end".to_string(), Arc::new(EndNode));

            Self {
                defs: RwLock::new(HashMap::new()),
                executors: RwLock::new(execs),
            }
        })
    }

    pub fn register(&self, n_type: &str, exec: impl NodeExecutor + 'static) {
        self.executors.write().unwrap().insert(n_type.to_string(), Arc::new(exec));
    }

    pub fn load(&self, id: &str, json: &str) -> Result<(), WorkflowError> {
        let def: WorkflowDefinition = serde_json::from_str(json)?;
        
        // 校验节点类型
        for node in &def.nodes {
            if self.get_executor(&node.node_type).is_none() {
                return Err(WorkflowError::RuntimeError(format!("Unknown node type: {}", node.node_type)));
            }
        }

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
}