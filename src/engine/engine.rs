use crate::engine::graph::WorkflowGraph;
use crate::engine::scheduler::WorkflowScheduler;
use crate::models::context::FlowContext;
use crate::models::error::WorkflowError;
use crate::models::event_bus::EventBus;
use crate::models::workflow::WorkflowResult;
use crate::nodes::{DecisionNode, NodeExecutor, StartNode, SubflowNode};
use dashmap::DashMap;
use std::sync::{Arc, OnceLock};

static INSTANCE: OnceLock<WorkflowEngine> = OnceLock::new();


pub struct WorkflowEngine {
    workflow_graphs: DashMap<String, Arc<WorkflowGraph>>,
    workflow_scheduler: WorkflowScheduler,
}

impl WorkflowEngine {
    pub fn global() -> &'static WorkflowEngine {
        INSTANCE.get_or_init(|| {
            let scheduler = WorkflowScheduler::new();
            scheduler.register("start", StartNode);
            scheduler.register("switch", DecisionNode);
            scheduler.register("subflow", SubflowNode);

            WorkflowEngine {
                workflow_graphs: DashMap::new(),
                workflow_scheduler: scheduler,
            }
        })
    }

    pub fn register(&self, node_type: &str, executor: impl NodeExecutor + 'static) {
        self.workflow_scheduler.register(node_type, executor);
    }

    pub fn load(&self, workflow_id: &str, json: &str) -> Result<(), WorkflowError> {
        let mut graph = WorkflowGraph::new();
        graph.load(json)?;
        self.workflow_graphs
            .insert(workflow_id.to_string(), Arc::new(graph));
        Ok(())
    }

    pub async fn run(&self, workflow_id: &str) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_ctx_event(workflow_id, FlowContext::new(), EventBus::default())
            .await
    }

    pub async fn run_with_event(
        &self,
        workflow_id: &str,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_ctx_event(workflow_id, FlowContext::new(), event_bus)
            .await
    }

    pub async fn run_with_ctx(
        &self,
        workflow_id: &str,
        flow_context: FlowContext,
    ) -> Result<WorkflowResult, WorkflowError> {
        self.run_with_ctx_event(workflow_id, flow_context, EventBus::default())
            .await
    }

    pub async fn run_with_ctx_event(
        &self,
        workflow_id: &str,
        flow_context: FlowContext,
        event_bus: EventBus,
    ) -> Result<WorkflowResult, WorkflowError> {
        let graph = {
            self.workflow_graphs
                .get(workflow_id)
                .map(|entry| entry.value().clone())
                .ok_or_else(|| WorkflowError::WorkflowNotFound(workflow_id.to_string()))?
        };

        self.workflow_scheduler
            .execute(flow_context, event_bus, graph)
            .await
    }
}
