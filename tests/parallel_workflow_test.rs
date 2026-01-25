use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::time::timeout;
use uflow::prelude::*;

struct PrintNode;

#[async_trait]
impl NodeExecutor for PrintNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!(
            "PrintNode [{}] 线程号: {:?}",
            ctx.node.id,
            thread::current().id()
        );

        if let Some(sleep_ms) = resolved.get("sleep_ms").and_then(|v| v.as_u64()) {
            if sleep_ms > 0 {
                // println!("PrintNode [{}] sleeping for {}ms", ctx.node.id, sleep_ms);
                tokio::time::sleep(Duration::from_millis(sleep_ms)).await;
            }
        }

        println!("打印节点输出: {:?}", resolved);
        Ok(resolved)
    }
}

struct OutputNode;

#[async_trait]
impl NodeExecutor for OutputNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!("OutputNode 线程号: {:?}", thread::current().id());
        Ok(resolved)
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test_parallel_workflow() {
    println!("主线程号: {:?}", thread::current().id());
    let engine = WorkflowEngine::global();
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();
    engine.register("print", PrintNode);
    engine.register("output", OutputNode);

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/complex_parallel_workflow.json");

    let json_content = fs::read_to_string(path).expect("Failed to read workflow file");

    let workflow_id = "complex-parallel-workflow";
    engine
        .load(workflow_id, &json_content)
        .expect("Failed to load workflow");

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let listener_handle = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            // println!("收到事件: {:?}", event);
            events_clone.lock().unwrap().push(event.clone());
            if let WorkflowEvent::FlowFinished { .. } = event {
                break;
            }
        }
    });

    let start_time = std::time::Instant::now();
    let result = engine
        .run_with_event(workflow_id, event_bus)
        .await
        .expect("Failed to run workflow");
    let elapsed = start_time.elapsed();

    timeout(Duration::from_secs(5), listener_handle)
        .await
        .expect("Listener timed out")
        .expect("Listener task failed");

    assert_eq!(result.status, FlowStatus::Succeeded);

    let output = result.output.expect("Workflow should have output");
    assert_eq!(output["a"], "Process A2");
    assert_eq!(output["b"], "Process B1");
    assert_eq!(output["c"], "Process C2");

    let collected_events = events.lock().unwrap();

    let node_started_count = collected_events
        .iter()
        .filter(|e| matches!(e, WorkflowEvent::NodeStarted { .. }))
        .count();
    let node_completed_count = collected_events
        .iter()
        .filter(|e| matches!(e, WorkflowEvent::NodeCompleted { .. }))
        .count();

    // 3 starts + 5 prints + 1 output = 9 nodes
    assert_eq!(node_started_count, 9);
    assert_eq!(node_completed_count, 9);

    println!("Total execution time: {:?}ms", elapsed.as_millis());

    // Serial execution calculation (considering only print nodes have sleep):
    // A: 100(a1) + 100(a2) = 200ms
    // B: 200(b1) = 200ms
    // C: 50(c1) + 50(c2) = 100ms
    // Total serial wait time: 500ms.
    // Parallel critical path: max(200, 200, 100) = 200ms.
    // Adding some buffer for overhead.
    assert!(
        elapsed.as_millis() < 450,
        "Execution took {}ms, expected < 450ms. Likely serial execution.",
        elapsed.as_millis()
    );
}
