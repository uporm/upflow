use std::collections::HashMap;
use serde_json::json;
use uflow::{EndNode, FlowContext, StartNode, WorkflowEngine, WorkflowError};

#[test]
fn test_start_node_validation() {
    let engine = WorkflowEngine::global();
    engine.register("start", StartNode);

    let workflow_json = json!({
        "id": "test_flow",
        "nodes": [
            {
                "id": "start",
                "type": "start",
                "data": {
                    "inputs": "invalid_inputs" 
                }
            }
        ],
        "edges": []
    });

    let result = engine.load("test_flow", &workflow_json.to_string());
    
    assert!(result.is_err());
    match result {
        Err(WorkflowError::ValidationError(msg)) => {
            assert_eq!(msg, "inputs must be an array");
        }
        _ => panic!("Expected ValidationError, got {:?}", result),
    }
}

#[test]
fn test_start_node_validation_item() {
    let engine = WorkflowEngine::global();
    engine.register("start", StartNode);

    let workflow_json = json!({
        "id": "test_flow_2",
        "nodes": [
            {
                "id": "start",
                "type": "start",
                "data": {
                    "inputs": [
                        { "no_var": "something" }
                    ]
                }
            }
        ],
        "edges": []
    });

    let result = engine.load("test_flow_2", &workflow_json.to_string());
    
    assert!(result.is_err());
    match result {
        Err(WorkflowError::ValidationError(msg)) => {
            assert_eq!(msg, "input item must have a 'var' field");
        }
        _ => panic!("Expected ValidationError, got {:?}", result),
    }
}

#[tokio::test]
async fn test_sys_vars_resolution() -> Result<(), Box<dyn std::error::Error>> {
    let engine = WorkflowEngine::global();
    engine.register("end", EndNode);

    let workflow_json = json!({
        "id": "sys_flow",
        "nodes": [
            {
                "id": "end",
                "type": "end",
                "data": {
                    "value": "{{sys.request_id}}"
                }
            }
        ],
        "edges": []
    });

    engine.load("sys_flow", &workflow_json.to_string())?;
    let mut sys_vars = HashMap::new();
    sys_vars.insert("request_id".to_string(), json!("abc"));
    let flow_context = FlowContext::new().with_sys_vars(sys_vars);
    let result = engine.run_with_ctx("sys_flow", flow_context).await?;
    assert_eq!(result.output.unwrap(), json!({ "value": "abc" }));
    Ok(())
}
