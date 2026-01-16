use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use anyhow::{Result, anyhow};
use tokio::time::{timeout, Duration};

use crate::model::{
    workflow::Workflow,
    context::WorkflowContext,
    state::{NodeRunState, NodeState},
    node_trait::NodeOutput,
};
use crate::registry::node_registry::NodeRegistry;
use crate::registry::workflow_registry::WorkflowRegistry;
use crate::engine::events::EventBus;

pub struct WorkflowEngine {
    pub id: String,
    pub flow: Arc<Workflow>,
    pub executors: Arc<NodeRegistry>,
    pub flows: Arc<WorkflowRegistry>,
    pub event_bus: EventBus,
}

pub struct WorkflowRunBuilder {
    id: Option<String>,
    flow: Option<Arc<Workflow>>,
    executors: Option<Arc<NodeRegistry>>,
    flows: Option<Arc<WorkflowRegistry>>,
    event_bus: Option<EventBus>,
}

impl WorkflowEngine {
    pub fn builder() -> WorkflowRunBuilder {
        WorkflowRunBuilder {
            id: None,
            flow: None,
            executors: None,
            flows: None,
            event_bus: None,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let _ = self.start_with_ctx(WorkflowContext::new()).await?;
        Ok(())
    }

    pub async fn start_with_ctx(&self, mut ctx: WorkflowContext) -> Result<WorkflowContext> {
        ctx.set_env(self.flows.clone(), self.executors.clone(), self.event_bus.clone());
        self.event_bus.run_started(&self.id);

        let (mut states, mut ready) = self.init_states();

        while let Some(node_id) = ready.pop_front() {
            states.get_mut(&node_id).unwrap().state = NodeState::Running;
            
            let result = self.execute_node(&node_id, &mut ctx).await;

            let out = match result {
                Ok(v) => {
                    self.event_bus.node_success(&self.id, &node_id);
                    states.get_mut(&node_id).unwrap().state = NodeState::Success;
                    v
                }
                Err(e) => {
                    self.event_bus.node_failed(&self.id, &node_id, &e.to_string());
                    states.get_mut(&node_id).unwrap().state = NodeState::Failed;
                    self.event_bus.run_finished(&self.id, false);
                    return Err(e);
                }
            };

            self.process_edges(&node_id, &out, &mut states, &mut ready);
        }

        self.event_bus.run_finished(&self.id, true);
        Ok(ctx)
    }

    fn init_states(&self) -> (HashMap<String, NodeRunState>, VecDeque<String>) {
        let mut states: HashMap<String, NodeRunState> = HashMap::new();
        let mut indegree: HashMap<String, usize> = HashMap::new();

        for n in &self.flow.nodes {
            indegree.insert(n.id.clone(), 0);
        }
        for e in &self.flow.edges {
            *indegree.entry(e.target.clone()).or_insert(0) += 1;
        }

        for (id, deg) in &indegree {
            states.insert(id.clone(), NodeRunState {
                state: NodeState::Pending,
                remaining: *deg,
            });
        }

        let mut ready = VecDeque::new();
        for (id, s) in &states {
            if s.remaining == 0 {
                ready.push_back(id.clone());
            }
        }
        (states, ready)
    }

    async fn execute_node(&self, node_id: &str, ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        let node = self.flow.nodes.iter().find(|n| n.id == node_id).unwrap().clone();
        let exec = self.executors
            .get(&node.node_type)
            .ok_or_else(|| anyhow!("no executor for {}", node.node_type))?;

        self.event_bus.node_running(&self.id, node_id);

        let timeout_secs = node.data.get("timeout").and_then(|v| v.as_u64()).unwrap_or(30);
        let retries = node.data.get("retries").and_then(|v| v.as_u64()).unwrap_or(0);

        let mut attempt = 0;
        loop {
            attempt += 1;
            let fut = exec.execute(&node, ctx);
            let res = timeout(Duration::from_secs(timeout_secs), fut).await;

            match res {
                Ok(Ok(v)) => return Ok(v),
                _ if attempt <= retries => continue,
                _ => return Err(anyhow!("node {} failed", node_id)),
            }
        }
    }

    fn process_edges(&self, node_id: &str, out: &NodeOutput, states: &mut HashMap<String, NodeRunState>, ready: &mut VecDeque<String>) {
        for e in self.flow.edges.iter().filter(|e| e.source == node_id) {
            if let Some(handles) = &out.matched_handles {
                if let Some(h) = &e.sourceHandle {
                    if !handles.contains(h) {
                        states.get_mut(&e.target).unwrap().state = NodeState::Skipped;
                        continue;
                    }
                }
            }

            let st = states.get_mut(&e.target).unwrap();
            if st.remaining > 0 {
                st.remaining -= 1;
            }
            if st.remaining == 0 && st.state == NodeState::Pending {
                ready.push_back(e.target.clone());
            }
        }
    }
}

impl WorkflowRunBuilder {
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }
    pub fn flow(mut self, flow: Arc<Workflow>) -> Self {
        self.flow = Some(flow);
        self
    }
    pub fn flow_json(mut self, json: &str) -> Self {
        let wf: Workflow = serde_json::from_str(json).expect("workflow parse failed");
        self.flow = Some(Arc::new(wf));
        self
    }
    pub fn executors(mut self, executors: Arc<NodeRegistry>) -> Self {
        self.executors = Some(executors);
        self
    }
    pub fn flows(mut self, flows: Arc<WorkflowRegistry>) -> Self {
        self.flows = Some(flows);
        self
    }
    pub fn event_bus(mut self, event_bus: EventBus) -> Self {
        self.event_bus = Some(event_bus);
        self
    }
    pub fn build(self) -> WorkflowEngine {
        WorkflowEngine {
            id: self.id.expect("id required"),
            flow: self.flow.expect("flow required"),
            executors: self.executors.expect("executors required"),
            flows: self.flows.expect("flows required"),
            event_bus: self.event_bus.unwrap_or_default(),
        }
    }
}
