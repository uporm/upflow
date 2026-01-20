use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, RwLock};
use std::time::Instant;

use futures::FutureExt;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio::time::{sleep, timeout, Duration};
use tokio_util::sync::CancellationToken;

use crate::context::FlowContext;
use crate::engine::EventBus;
use crate::graph::GraphStore;
use crate::model::{
    Edge, FlowStatus, Node, NodePolicy, OnErrorStrategy, Workflow, WorkflowError, WorkflowEvent,
    WorkflowResult,
};
use crate::nodes::executor::{NodeContext, NodeExecutor};

pub struct Scheduler {
    pub registry: Arc<RwLock<HashMap<String, Arc<dyn NodeExecutor>>>>,
}

struct NodeRunResult {
    node_id: String,
    #[allow(dead_code)]
    node_type: String,
    output: Option<Value>,
    error: Option<WorkflowError>,
    strategy: OnErrorStrategy,
    fallback_node_id: Option<String>,
}

/// Manages the execution state of the workflow graph
struct ExecutionState {
    graph: DiGraph<Node, Edge>,
    node_map: HashMap<String, NodeIndex>,

    // Runtime state
    // Number of incoming edges that haven't fired yet for each node
    pending_incoming: HashMap<NodeIndex, usize>,

    // Number of incoming edges that fired with an "active" signal
    active_incoming: HashMap<NodeIndex, usize>,

    ready_queue: VecDeque<NodeIndex>,
    skipped_nodes: HashSet<NodeIndex>,
}

impl ExecutionState {
    fn new(def: &Workflow) -> Result<Self, WorkflowError> {
        let store = GraphStore::build(def)?;
        let mut state = Self {
            graph: store.graph,
            node_map: store.node_map,
            pending_incoming: HashMap::new(),
            active_incoming: HashMap::new(),
            ready_queue: VecDeque::new(),
            skipped_nodes: HashSet::new(),
        };

        state.init();
        Ok(state)
    }

    fn init(&mut self) {
        for idx in self.graph.node_indices() {
            let incoming = self
                .graph
                .neighbors_directed(idx, petgraph::Direction::Incoming)
                .count();
            self.pending_incoming.insert(idx, incoming);
            if incoming == 0 {
                self.ready_queue.push_back(idx);
            }
        }
    }

    fn get_node(&self, idx: NodeIndex) -> Option<&Node> {
        self.graph.node_weight(idx)
    }

    fn pop_ready(&mut self) -> Option<NodeIndex> {
        self.ready_queue.pop_front()
    }

    fn node_completed(&mut self, node_id: &str, output: Option<&Value>) {
        let node_idx = match self.node_map.get(node_id) {
            Some(&idx) => idx,
            None => return,
        };

        let node_type = self.graph[node_idx].node_type.clone();

        let selected_handle = if node_type == "decision" {
            output
                .and_then(|v| v.get("selected"))
                .and_then(|v| v.as_str())
        } else {
            None
        };

        // Collect edges first to avoid borrowing issues
        let mut edges = Vec::new();
        for edge in self.graph.edges(node_idx) {
            edges.push((edge.target(), edge.weight().source_handle.clone()));
        }

        for (target, handle) in edges {
            let is_active = if node_type == "decision" {
                match (selected_handle, &handle) {
                    (Some(sel), Some(h)) => sel == h,
                    (None, Some(_)) => false, // Edge requires handle, but none selected
                    (_, None) => true,        // Edge requires no handle (default path)
                }
            } else {
                true
            };

            self.propagate_edge(target, is_active);
        }
    }

    fn propagate_edge(&mut self, target: NodeIndex, is_active: bool) {
        if let Some(pending) = self.pending_incoming.get_mut(&target) {
            if *pending > 0 {
                *pending -= 1;
            }

            if is_active {
                *self.active_incoming.entry(target).or_insert(0) += 1;
            }

            if *pending == 0 {
                let active_count = self.active_incoming.get(&target).copied().unwrap_or(0);
                if active_count > 0 && !self.skipped_nodes.contains(&target) {
                    self.ready_queue.push_back(target);
                } else {
                    self.skip_node(target);
                }
            }
        }
    }

    fn skip_node(&mut self, node: NodeIndex) {
        if self.skipped_nodes.insert(node) {
            let targets: Vec<NodeIndex> = self.graph.neighbors(node).collect();
            for target in targets {
                self.propagate_edge(target, false);
            }
        }
    }

    fn trigger_fallback(&mut self, fallback_node_id: &str) {
        if let Some(&idx) = self.node_map.get(fallback_node_id) {
            // Force reset state for fallback node to ensure it runs
            // Note: This assumes fallback node is part of the graph but we want to manually trigger it
            // potentially ignoring its original dependencies or if it was skipped.
            self.pending_incoming.insert(idx, 0);
            self.active_incoming.insert(idx, 1);
            self.skipped_nodes.remove(&idx);
            self.ready_queue.push_back(idx);
        }
    }

    fn get_end_output(&self, ctx: &FlowContext) -> Option<Value> {
        self.graph.node_indices().find_map(|idx| {
            let node = &self.graph[idx];
            if node.node_type == "end" {
                ctx.get_output(&node.id)
            } else {
                None
            }
        })
    }
}

impl Scheduler {
    pub async fn execute(
        &self,
        def: Arc<Workflow>,
        event_bus: EventBus,
        input: Value,
        cancellation: CancellationToken,
        instance_id: String,
    ) -> Result<WorkflowResult, WorkflowError> {
        let mut state = ExecutionState::new(&def)?;
        let ctx = FlowContext::new(input.clone());

        event_bus.emit(WorkflowEvent::FlowStarted {
            id: def.id.clone(),
            input: input.clone(),
            timestamp: chrono::Utc::now(),
        });

        let (tx, mut rx) = mpsc::unbounded_channel::<NodeRunResult>();
        let mut running = 0usize;
        let mut has_error = false;

        loop {
            if cancellation.is_cancelled() {
                break;
            }
            while let Some(idx) = state.pop_ready() {
                let node = state.get_node(idx).cloned().unwrap();
                let policy = NodePolicy::from_value(&node.data);
                
                let registry = self.registry.read().unwrap();
                let executor = registry
                    .get(&node.node_type)
                    .cloned()
                    .ok_or_else(|| WorkflowError::NodeExecutorNotFound(node.node_type.clone()))?;
                drop(registry);

                let event_bus = event_bus.clone();
                let tx = tx.clone();
                let ctx = ctx.clone();
                let cancellation = cancellation.clone();

                tokio::spawn(async move {
                    let result = run_node(node, executor, policy, ctx, event_bus, cancellation).await;
                    let _ = tx.send(result);
                });
                running += 1;
            }

            if running == 0 {
                break;
            }

            if let Some(res) = rx.recv().await {
                running = running.saturating_sub(1);
                
                match &res.error {
                    None => {
                        if let Some(output) = &res.output {
                            ctx.set_output(&res.node_id, output.clone());
                        }
                        state.node_completed(&res.node_id, res.output.as_ref());
                    }
                    Some(_) => {
                        if let Some(output) = &res.output {
                            ctx.set_output(&res.node_id, output.clone());
                        }
                        
                        match res.strategy {
                            OnErrorStrategy::FailFast => {
                                cancellation.cancel();
                                has_error = true;
                            }
                            OnErrorStrategy::Fallback => {
                                has_error = false;
                                if let Some(fallback) = &res.fallback_node_id {
                                    state.trigger_fallback(fallback);
                                }
                            }
                            OnErrorStrategy::Continue => {
                                has_error = false;
                                // Treat as completed (possibly with error output)
                                state.node_completed(&res.node_id, res.output.as_ref());
                            }
                        }
                    }
                }
            }
        }

        let status = if cancellation.is_cancelled() {
            FlowStatus::Cancelled
        } else if has_error {
            FlowStatus::Failed
        } else {
            FlowStatus::Succeeded
        };

        let end_output = state.get_end_output(&ctx);

        let result = WorkflowResult {
            instance_id,
            status: status.clone(),
            output: end_output.clone(),
        };

        event_bus.emit(WorkflowEvent::FlowFinished {
            id: def.id.clone(),
            status,
            output: end_output,
        });

        Ok(result)
    }
}

async fn run_node(
    node: Node,
    executor: Arc<dyn NodeExecutor>,
    policy: NodePolicy,
    ctx: FlowContext,
    event_bus: EventBus,
    cancellation: CancellationToken,
) -> NodeRunResult {
    let resolved_input = match ctx.resolve_value(&node.data) {
        Ok(v) => v,
        Err(e) => {
            event_bus.emit(WorkflowEvent::NodeError {
                node_id: node.id.clone(),
                node_type: node.node_type.clone(),
                error: e.to_string(),
                strategy: policy.on_error().as_str().to_string(),
            });
            return NodeRunResult {
                node_id: node.id,
                node_type: node.node_type,
                output: None,
                error: Some(e),
                strategy: policy.on_error(),
                fallback_node_id: policy.fallback_node_id.clone(),
            };
        }
    };

    let mut attempts = 0u32;
    let max_attempts = policy.max_attempts().max(1);
    let interval = policy.interval_ms();

    loop {
        if cancellation.is_cancelled() {
            return NodeRunResult {
                node_id: node.id,
                node_type: node.node_type,
                output: None,
                error: Some(WorkflowError::Cancelled),
                strategy: policy.on_error(),
                fallback_node_id: policy.fallback_node_id.clone(),
            };
        }
        attempts += 1;
        event_bus.emit(WorkflowEvent::NodeStarted {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            input: resolved_input.clone(),
        });
        let started = Instant::now();
        let exec_ctx = NodeContext {
            node: node.clone(),
            resolved_input: resolved_input.clone(),
            flow_context: ctx.clone(),
            event_bus: event_bus.clone(),
        };
        let fut = executor.execute(exec_ctx);
        let guarded = std::panic::AssertUnwindSafe(fut).catch_unwind();
        let result = if let Some(ms) = policy.timeout_ms {
            match timeout(Duration::from_millis(ms), guarded).await {
                Ok(inner) => match inner {
                    Ok(v) => v,
                    Err(_) => Err(WorkflowError::NodePanicked(node.id.clone())),
                },
                Err(_) => Err(WorkflowError::Timeout(node.id.clone())),
            }
        } else {
            match guarded.await {
                Ok(v) => v,
                Err(_) => Err(WorkflowError::NodePanicked(node.id.clone())),
            }
        };
        match result {
            Ok(output) => {
                let duration_ms = started.elapsed().as_millis() as u64;
                event_bus.emit(WorkflowEvent::NodeCompleted {
                    node_id: node.id.clone(),
                    node_type: node.node_type.clone(),
                    output: output.clone(),
                    duration_ms,
                });
                return NodeRunResult {
                    node_id: node.id,
                    node_type: node.node_type,
                    output: Some(output),
                    error: None,
                    strategy: policy.on_error(),
                    fallback_node_id: policy.fallback_node_id.clone(),
                };
            }
            Err(err) => {
                if attempts < max_attempts {
                    if interval > 0 {
                        sleep(Duration::from_millis(interval)).await;
                    }
                    continue;
                }
                event_bus.emit(WorkflowEvent::NodeError {
                    node_id: node.id.clone(),
                    node_type: node.node_type.clone(),
                    error: err.to_string(),
                    strategy: policy.on_error().as_str().to_string(),
                });
                let fallback = if policy.on_error() == OnErrorStrategy::Continue {
                    Some(json!({ "error": err.to_string() }))
                } else {
                    None
                };
                return NodeRunResult {
                    node_id: node.id,
                    node_type: node.node_type,
                    output: fallback,
                    error: Some(err),
                    strategy: policy.on_error(),
                    fallback_node_id: policy.fallback_node_id.clone(),
                };
            }
        }
    }
}
