use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::time::timeout;
use upflow::prelude::*;

struct PrintNode;

#[async_trait]
impl NodeExecutor for PrintNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!("PrintNode 线程号: {:?}", thread::current().id());
        println!("打印节点输出: {:?}", resolved);
        Ok(resolved)
    }
}

struct BranchOutputNode;

#[async_trait]
impl NodeExecutor for BranchOutputNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!("BranchOutputNode 线程号: {:?}", thread::current().id());
        Ok(resolved)
    }
}

#[tokio::test]
async fn test_branch_workflow() {
    let engine = WorkflowEngine::global();
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();
    engine.register("print", PrintNode);
    engine.register("output", BranchOutputNode);

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/branch_workflow.json");

    let json_content = fs::read_to_string(path).expect("Failed to read workflow file");

    let workflow_id = "branch-workflow";
    engine
        .load(workflow_id, &json_content)
        .expect("Failed to load workflow");

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let listener_handle = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            println!("收到事件: {:?}", event);
            events_clone.lock().unwrap().push(event.clone());
            if let WorkflowEvent::FlowFinished { .. } = event {
                break;
            }
        }
    });

    let payload = serde_json::json!({
        "route": "A"
    });
    let flow_context = Arc::new(FlowContext::new().with_payload(payload));

    let result = engine
        .run_with_ctx_event(workflow_id, flow_context, event_bus)
        .await
        .expect("Failed to run workflow");

    timeout(Duration::from_secs(2), listener_handle)
        .await
        .expect("Listener timed out")
        .expect("Listener task failed");

    assert_eq!(result.status, FlowStatus::Succeeded);

    let output = result.output.expect("Workflow should have output");
    // "Anull" because node-print-b-2 is skipped, resolving to null -> "null" string
    assert_eq!(output["selected"], "Anull");

    let collected_events = events.lock().unwrap();
    let has_flow_started = collected_events
        .iter()
        .any(|e| matches!(e, WorkflowEvent::FlowStarted { .. }));
    let has_flow_finished = collected_events
        .iter()
        .any(|e| matches!(e, WorkflowEvent::FlowFinished { .. }));
    assert!(has_flow_started, "缺少 FlowStarted 事件");
    assert!(has_flow_finished, "缺少 FlowFinished 事件");
}
