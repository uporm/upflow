use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use tokio_util::sync::CancellationToken;

pub use self::event_bus::EventBus;
use self::graph::WorkflowStore;
use self::scheduler::Scheduler;
use crate::context::FlowContext;
use crate::model::error::WorkflowError;
use crate::model::workflow::WorkflowResult;
use crate::nodes::executor::NodeExecutor;
use crate::utils::id::Id;

pub mod event_bus;
pub mod graph;
pub mod scheduler;

static INSTANCE: OnceLock<WorkflowEngine> = OnceLock::new();
const DEFAULT_EVENT_BUS_CAPACITY: usize = 100;

pub struct WorkflowEngine {
    node_executors: Arc<RwLock<HashMap<String, Arc<dyn NodeExecutor>>>>,
    workflow_store: WorkflowStore,
}

impl WorkflowEngine {
    pub fn global() -> &'static WorkflowEngine {
        INSTANCE.get_or_init(|| WorkflowEngine {
            node_executors: Arc::new(RwLock::new(HashMap::new())),
            workflow_store: WorkflowStore::new(),
        })
    }

    pub fn register(&self, node_type: &str, executor: impl NodeExecutor + 'static) {
        let mut registry = self.node_executors.write().unwrap();
        registry.insert(node_type.to_string(), Arc::new(executor));
    }

    pub fn scheduler(&self) -> Scheduler {
        Scheduler {
            registry: self.node_executors.clone(),
        }
    }

    pub fn load(&self, workflow_id: &str, json: &str) -> Result<(), WorkflowError> {
        self.workflow_store.load(workflow_id, json, |def| {
            for node in &def.nodes {
                let executor = self.scheduler().get_executor(&node.node_type)?;
                executor.validate(&node.data)?;
            }
            Ok(())
        })
    }

    pub async fn run(&self, workflow_id: &str) -> Result<WorkflowResult, WorkflowError> {
        let event_bus = EventBus::new(DEFAULT_EVENT_BUS_CAPACITY);
        let flow_context = FlowContext::new();
        self.run_with_ctx_event(workflow_id, flow_context, event_bus)
            .await
    }

    pub async fn run_with_event(
        &self,
        workflow_id: &str,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        let flow_context = FlowContext::new();
        self.run_with_ctx_event(workflow_id, flow_context, event_bus)
            .await
    }

    pub async fn run_with_ctx(
        &self,
        workflow_id: &str,
        flow_context: FlowContext,
    ) -> Result<WorkflowResult, WorkflowError> {
        let event_bus = EventBus::new(DEFAULT_EVENT_BUS_CAPACITY);
        self.run_with_ctx_event(workflow_id, flow_context, event_bus)
            .await
    }

    pub async fn run_with_ctx_event(
        &self,
        workflow_id: &str,
        flow_context: FlowContext,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        let (workflow, graph) = self.workflow_store.get(workflow_id)?;
        let instance_id = self.next_instance_id()?;
        let token = CancellationToken::new();
        let scheduler = Scheduler {
            registry: self.node_executors.clone(),
        };
        scheduler
            .execute(
                workflow,
                graph,
                event_bus,
                flow_context,
                token.clone(),
                instance_id,
            )
            .await
    }

    fn next_instance_id(&self) -> Result<String, WorkflowError> {
        let id = Id::next_id()?;
        Ok(format!("{}", id))
    }
}
