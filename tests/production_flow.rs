use std::time::Duration;

use serde_json::{json, Value};
use uflow::nodes::{DecisionNode, EndNode, IterationNode, StartNode, SubflowNode};
use uflow::nodes::executor::{NodeContext, NodeExecutor};
use uflow::model::WorkflowError;
use uflow::{EventBus, FlowStatus, WorkflowEngine, WorkflowEvent};
use async_trait::async_trait;

struct MockNode;

#[async_trait]
impl NodeExecutor for MockNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let url = ctx
            .resolved_input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if url.contains("panic") {
            panic!("simulated panic");
        }
        if url.contains("timeout") {
            tokio::time::sleep(std::time::Duration::from_millis(2_500)).await;
        }
        Ok(json!({ "url": url, "status": 200 }))
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn production_flow() -> Result<(), Box<dyn std::error::Error>> {
    let engine = WorkflowEngine::global();
    engine.register("start", StartNode);
    engine.register("end", EndNode);
    engine.register("decision", DecisionNode);
    engine.register("http_request", MockNode);
    engine.register("iteration", IterationNode);
    engine.register("subflow", SubflowNode);

    let wf_json = include_str!("./example_workflow.json");
    engine.load("wf1", wf_json)?;

    let event_bus = EventBus::new(100);
    let mut logger = event_bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = logger.recv().await {
            println!("{:?}", event);
            if matches!(event, WorkflowEvent::FlowFinished { .. }) {
                break;
            }
        }
    });

    let mut collector = event_bus.subscribe();
    let collect_handle = tokio::spawn(async move {
        let mut events = Vec::new();
        while let Ok(event) = collector.recv().await {
            let done = matches!(event, WorkflowEvent::FlowFinished { .. });
            events.push(event);
            if done {
                break;
            }
        }
        events
    });

    let input = json!({
        "urls": ["timeout://slow", "panic://boom", "https://ok"]
    });
    let result = engine.run_with_input("wf1", event_bus.clone(), input).await?;
    let events = collect_handle.await?;
    assert!(events.iter().any(|e| matches!(e, WorkflowEvent::NodeError { .. })));
    assert!(matches!(
        result.status,
        FlowStatus::Succeeded | FlowStatus::Failed | FlowStatus::Cancelled
    ));

    let stop_bus = EventBus::new(100);
    let (instance_id, handle) = engine.start(
        "wf1",
        stop_bus,
        json!({ "urls": ["timeout://1", "timeout://2", "timeout://3"] }),
    )?;
    tokio::time::sleep(Duration::from_millis(200)).await;
    engine.stop(&instance_id);
    let stopped = handle.await??;
    assert!(matches!(stopped.status, FlowStatus::Cancelled));

    Ok(())
}
