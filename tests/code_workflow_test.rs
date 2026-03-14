use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use tokio::time::{Duration, sleep};
use upflow::prelude::*;

struct CodeNode;

#[async_trait]
impl NodeExecutor for CodeNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!(
            ">>> Executing CodeNode [{}] on thread {:?} <<<",
            ctx.node.id,
            thread::current().id()
        );

        // Resolve data to handle variable substitution
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;

        // Parse inputs from the "input" array in the node data
        let inputs = resolved.get("input").and_then(|v| v.as_array());

        let mut arg1 = String::new();
        let mut arg2 = String::new();

        if let Some(input_array) = inputs {
            for item in input_array {
                if let (Some(name), Some(value)) = (
                    item.get("name").and_then(|v| v.as_str()),
                    item.get("value").and_then(|v| v.as_str()),
                ) {
                    match name {
                        "arg1" => arg1 = value.to_string(),
                        "arg2" => arg2 = value.to_string(),
                        _ => {}
                    }
                }
            }
        }

        println!("CodeNode Inputs: arg1 = '{}', arg2 = '{}'", arg1, arg2);

        // Simulate the code execution: var output1 = arg1 + ' ' + arg2;
        let output1 = format!("{} {}", arg1, arg2);
        println!("CodeNode Result: output1 = '{}'", output1);

        // Return the result
        // We also include output2 to satisfy the "required" rule in the schema, just in case
        Ok(serde_json::json!({
            "output1": output1,
            "output2": "dummy"
        }))
    }
}

struct OutputNode;

#[async_trait]
impl NodeExecutor for OutputNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        println!(
            ">>> Executing OutputNode [{}] on thread {:?} <<<",
            ctx.node.id,
            thread::current().id()
        );
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!("OutputNode Resolved Data: {:?}", resolved);
        Ok(resolved)
    }
}

#[tokio::test]
async fn test_code_workflow() {
    // 1. Initialize Engine
    let engine = WorkflowEngine::global();

    // 2. Register Custom Nodes
    engine.register("code", CodeNode);
    engine.register("output", OutputNode);

    // 3. Load Workflow
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/code_workflow.json");

    let json_content = fs::read_to_string(path).expect("Failed to read workflow file");
    let workflow_id = "code-workflow";
    engine
        .load(workflow_id, &json_content)
        .expect("Failed to load workflow");

    let instance_id = "code-workflow-instance-1";
    // 4. Setup Event Listener
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();

    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            println!("收到事件: {:?}", event);
            println!("instance_id: {}", instance_id)
        }
    });

    // 5. Run Workflow
    let payload = serde_json::json!({
        "username": "Jason",
        "sex": "Male"
    });
    let flow_context = Arc::new(FlowContext::new().with_payload(payload));

    let result = WorkflowEngine::global()
        .run_with_ctx_event(workflow_id, flow_context, event_bus)
        .await
        .expect("Failed to run workflow");

    // 6. Verify Result
    assert_eq!(result.status, FlowStatus::Succeeded);

    let output = result.output.expect("Workflow should have output");
    // node-output data is { "out1": "{{node-code.output1}}" }
    // So we expect output["out1"] to be "Jason Male"
    assert_eq!(output["out1"], "Jason Male");

    println!("测试完成，休眠 5 秒...");
    sleep(Duration::from_secs(5)).await;
}
