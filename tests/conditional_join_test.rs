use std::sync::Arc;
use uflow::{WorkflowEngine, WorkflowContext, WorkflowError};
use serde_json::{json, Value};
use async_trait::async_trait;
use uflow::model::NodeOutput;
use uflow::nodes::node_trait::NodeExecutor;

struct TrapNode;

#[async_trait]
impl NodeExecutor for TrapNode {
    async fn execute(&self, _ctx: &WorkflowContext, _data: &Value) -> Result<NodeOutput, WorkflowError> {
        panic!("TrapNode executed! This path should have been skipped.");
    }
}

#[tokio::test]
async fn test_conditional_join_skip() {
    let engine = WorkflowEngine::global();
    engine.register("trap", TrapNode);
    
    // Flow:
    // Start -> Decision
    // Decision -> A (handle "a") -> End
    // Decision -> B (handle "b") -> Trap -> End
    // 
    // We set variable so Decision chooses "a".
    // Path B should be skipped. Trap should NOT run.
    
    let flow_json = json!({
        "nodes": [
            { "id": "start", "type": "start", "data": {} },
            { "id": "decision", "type": "decision", "data": {
                "cases": [
                    {
                        "id": "path_a",
                        "opr": "eq",
                        "conditions": [
                            { "varId": "input.val", "opr": "eq", "value": "a" }
                        ]
                    },
                    {
                        "id": "path_b",
                        "opr": "eq",
                        "conditions": [
                            { "varId": "input.val", "opr": "eq", "value": "b" }
                        ]
                    }
                ]
            }},
            { "id": "node_a", "type": "decision", "data": {} }, // Dummy node
            { "id": "node_b", "type": "trap", "data": {} },     // Should not execute
            { "id": "end", "type": "end", "data": {} }
        ],
        "edges": [
            { "source": "start", "target": "decision", "sourceHandle": null },
            { "source": "decision", "target": "node_a", "sourceHandle": "path_a" },
            { "source": "decision", "target": "node_b", "sourceHandle": "path_b" },
            { "source": "node_a", "target": "end", "sourceHandle": null },
            { "source": "node_b", "target": "end", "sourceHandle": null }
        ]
    });
    
    engine.load("conditional_flow", &flow_json.to_string()).expect("Failed to load flow");
    
    let ctx = Arc::new(WorkflowContext::new(0));
    ctx.set_var("input".to_string(), json!({ "val": "a" }));
    
    // This should NOT panic
    let result = engine.run_with_context("conditional_flow", ctx).await;
    assert!(result.is_ok());
}
