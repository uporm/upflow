use upflow::prelude::*;
use std::time::Duration;
use tokio::time::sleep;
use async_trait::async_trait;
use serde_json::Value;

struct SleepNode;

#[async_trait]
impl NodeExecutor for SleepNode {
    async fn execute(&self, _ctx: NodeContext) -> Result<Value, WorkflowError> {
        sleep(Duration::from_millis(1000)).await;
        Ok(Value::Null)
    }
}

#[tokio::test]
async fn test_workflow_cancellation() {
    let engine = WorkflowEngine::global();
    engine.register("sleep", SleepNode);

    let workflow_json = r#"{
        "nodes": [
            { "id": "start", "type": "start", "data": {}, "next": "sleep1" },
            { "id": "sleep1", "type": "sleep", "next": "end" },
            { "id": "end", "type": "end" }
        ],
        "edges": [
            { "source": "start", "target": "sleep1" },
            { "source": "sleep1", "target": "end" }
        ]
    }"#;
    
    let workflow_id = "cancel_flow";
    engine.load(workflow_id, workflow_json).unwrap();
    
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();
    let event_bus_clone = event_bus.clone();
    
    // Spawn listener to cancel
    let handle = tokio::spawn(async move {
        let mut stopped_emitted = false;
        while let Ok(event) = rx.recv().await {
            match event {
                WorkflowEvent::FlowStarted { instance_id, .. } => {
                    println!("Got FlowStarted for instance {}, terminating...", instance_id);
                    sleep(Duration::from_millis(100)).await;
                    engine.stop(&instance_id);
                },
                WorkflowEvent::FlowStopped { .. } => {
                    stopped_emitted = true;
                    break; // Exit loop after receiving stopped event
                },
                WorkflowEvent::FlowFinished { .. } => {
                    break; // Exit loop if it finished normally or with error
                }
                _ => {}
            }
        }
        stopped_emitted
    });
    
    let result = engine.run_with_event(workflow_id, event_bus_clone).await;
    
    match result {
        Err(WorkflowError::Cancelled) => println!("Workflow cancelled successfully"),
        Ok(_) => panic!("Workflow should have been cancelled"),
        Err(e) => panic!("Workflow failed with unexpected error: {:?}", e),
    }

    let stopped_emitted = handle.await.unwrap();
    assert!(stopped_emitted, "FlowStopped event should have been emitted");
}
