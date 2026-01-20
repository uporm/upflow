use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use serde_json::{json, Value};
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout, Duration};

use crate::context::FlowContext;
use crate::engine::{EventBus, WorkflowEngine};
use crate::model::{Node, NodePolicy, OnErrorStrategy, WorkflowError, WorkflowEvent};
use crate::nodes::executor::{NodeContext, NodeExecutor};

pub struct IterationNode;

struct ChildRunResult {
    output: Value,
    failed: bool,
}

#[async_trait]
impl NodeExecutor for IterationNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.resolved_input;
        let raw = ctx.node.data;
        let list = resolved
            .get("iterator_selector")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let concurrency = resolved
            .get("config")
            .and_then(|v| v.get("concurrency"))
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        let children = raw
            .get("children")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let engine = WorkflowEngine::global();
        let semaphore = Arc::new(Semaphore::new(concurrency as usize));
        let mut outputs = vec![Value::Null; list.len()];
        let mut failed = 0usize;

        let event_bus = ctx.event_bus.clone();
        if concurrency <= 1 {
            for (i, item) in list.iter().enumerate() {
                let mut sys_vars = std::collections::HashMap::new();
                sys_vars.insert("item".to_string(), item.clone());
                let sub_ctx = FlowContext::new(json!({})).with_sys_vars(sys_vars);
                let mut last = Value::Null;
                for child in &children {
                    let node_def: Node =
                        serde_json::from_value(child.clone()).map_err(|e| WorkflowError::ParseError(e.to_string()))?;
                    match execute_child_node(
                        node_def.clone(),
                        &sub_ctx,
                        event_bus.clone(),
                        engine,
                    )
                    .await
                    {
                        Ok(result) => {
                            last = result.output;
                            if result.failed {
                                failed += 1;
                            }
                        }
                        Err(err) => return Err(err),
                    }
                }
                outputs[i] = last;
            }
        } else {
            let mut handles = Vec::new();
            for (i, item) in list.iter().enumerate() {
                let children_inner = children.clone();
                let engine_ref = WorkflowEngine::global();
                let sem = semaphore.clone();
                let item = item.clone();
                let event_bus = event_bus.clone();
                handles.push(tokio::spawn(async move {
                    let _permit = sem.acquire().await.map_err(|_| WorkflowError::Cancelled)?;
                    let mut sys_vars = std::collections::HashMap::new();
                    sys_vars.insert("item".to_string(), item);
                    let sub_ctx = FlowContext::new(json!({})).with_sys_vars(sys_vars);
                    let mut last = Value::Null;
                    let mut failed = 0usize;
                    for child in &children_inner {
                        let node_def: Node =
                            serde_json::from_value(child.clone())
                                .map_err(|e| WorkflowError::ParseError(e.to_string()))?;
                        match execute_child_node(
                            node_def.clone(),
                            &sub_ctx,
                            event_bus.clone(),
                            engine_ref,
                        )
                        .await
                        {
                            Ok(result) => {
                                last = result.output;
                                if result.failed {
                                    failed += 1;
                                }
                            }
                            Err(err) => return Err(err),
                        }
                    }
                    Ok::<(usize, Value, usize), WorkflowError>((i, last, failed))
                }));
            }
            let mut error: Option<WorkflowError> = None;
            for handle in handles {
                match handle.await {
                    Ok(Ok((i, out, f))) => {
                        outputs[i] = out;
                        failed += f;
                    }
                    Ok(Err(err)) => {
                        if error.is_none() {
                            error = Some(err);
                        }
                    }
                    Err(_) => {
                        if error.is_none() {
                            error = Some(WorkflowError::Cancelled);
                        }
                    }
                }
            }
            if let Some(err) = error {
                return Err(err);
            }
        }
        Ok(json!({
            "results": outputs,
            "failed_count": failed
        }))
    }
}

async fn execute_child_node(
    node: Node,
    ctx: &FlowContext,
    event_bus: EventBus,
    engine: &WorkflowEngine,
) -> Result<ChildRunResult, WorkflowError> {
    let policy = NodePolicy::from_value(&node.data);
    let resolved_input = ctx.resolve_value(&node.data)?;
    let mut attempts = 0u32;
    let max_attempts = policy.max_attempts().max(1);
    let interval = policy.interval_ms();
    let executor = engine.executor(&node.node_type)?;
    loop {
        attempts += 1;
        event_bus.emit(WorkflowEvent::NodeStarted {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            input: resolved_input.clone(),
        });
        let started = std::time::Instant::now();
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
                return Ok(ChildRunResult {
                    output,
                    failed: false,
                });
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
                if policy.on_error() == OnErrorStrategy::Continue {
                    return Ok(ChildRunResult {
                        output: json!({ "error": err.to_string() }),
                        failed: true,
                    });
                }
                return Err(err);
            }
        }
    }
}
