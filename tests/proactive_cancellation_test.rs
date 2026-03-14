use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use upflow::prelude::*;
use upflow::utils::id::Id;

struct SleepNode;

#[async_trait]
impl NodeExecutor for SleepNode {
    async fn execute(&self, _ctx: NodeContext) -> Result<Value, WorkflowError> {
        sleep(Duration::from_millis(1000)).await;
        Ok(Value::Null)
    }
}

#[tokio::test]
async fn test_workflow_proactive_cancellation() {
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

    let workflow_id = "proactive_cancel_flow";
    engine.load(workflow_id, workflow_json).unwrap();

    // Create an ID beforehand
    let instance_id = Id::next_id().unwrap();
    let instance_id_str = instance_id.to_string();

    // Spawn execution
    let handle = tokio::spawn(async move {
        engine
            .run_with_instance_id(
                instance_id,
                workflow_id,
                Arc::new(FlowContext::new()),
                EventBus::default(),
            )
            .await
    });

    // Wait a bit then cancel using the known ID
    sleep(Duration::from_millis(100)).await;
    let success = engine.stop(&instance_id_str);
    assert!(success, "Termination request should return true");

    let result = handle.await.expect("Task failed");

    match result {
        Err(WorkflowError::Cancelled) => println!("Workflow cancelled successfully"),
        Ok(_) => panic!("Workflow should have been cancelled"),
        Err(e) => panic!("Workflow failed with unexpected error: {:?}", e),
    }
}
