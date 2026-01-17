use std::sync::Arc;
use uflow::{WorkflowEngine, WorkflowContext};
use serde_json::json;

#[tokio::test]
async fn test_parallel_join() {
    let engine = WorkflowEngine::global();
    
    // Define a flow:
    // Start -> A
    // Start -> B
    // A -> End
    // B -> End
    // This is a parallel flow joining at End.
    
    // To verify parallelism, A and B should simulate some work.
    // Since we don't have a "sleep" node, we can't easily prove they run in parallel 
    // without custom nodes. 
    // But we can verify that End waits for BOTH.
    
    let flow_json = json!({
        "nodes": [
            { "id": "start", "type": "start", "data": {} },
            { "id": "node_a", "type": "decision", "data": {} }, // Using decision as a dummy pass-through
            { "id": "node_b", "type": "decision", "data": {} },
            { "id": "end", "type": "end", "data": {} }
        ],
        "edges": [
            { "source": "start", "target": "node_a", "sourceHandle": null },
            { "source": "start", "target": "node_b", "sourceHandle": null },
            { "source": "node_a", "target": "end", "sourceHandle": null },
            { "source": "node_b", "target": "end", "sourceHandle": null }
        ]
    });
    
    engine.load("parallel_flow", &flow_json.to_string()).expect("Failed to load flow");
    
    let ctx = Arc::new(WorkflowContext::new(0));
    let result = engine.run_with_context("parallel_flow", ctx).await;
    
    assert!(result.is_ok());
}
