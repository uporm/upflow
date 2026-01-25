use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use upflow::prelude::*;

struct PrintNode;

#[async_trait]
impl NodeExecutor for PrintNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        // 解析数据（包含变量如 ${node-start.message}）
        // 注意：ctx.node.data 是 Arc<Value>，所以我们使用 as_ref()
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;

        // 在实际打印节点中，我们可能会打印到标准输出或日志
        println!("PrintNode 线程号: {:?}", thread::current().id());
        if let Some(content) = resolved.get("print_content") {
            println!("打印节点内容: {}", content);
        } else {
            println!("打印节点解析结果: {:?}", resolved);
        }

        // 返回解析的数据作为输出，供下游节点使用
        Ok(resolved)
    }
}

struct OutputNode;

#[async_trait]
impl NodeExecutor for OutputNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!("OutputNode 线程号: {:?}", thread::current().id());
        println!("OutputNode resolved: {:?}", resolved);
        Ok(resolved)
    }
}

#[tokio::test]
async fn test_simple_workflow() {
    // 1. Initialize Engine
    let engine = WorkflowEngine::global();

    // 2. Register Custom Nodes
    // The engine might already have some nodes registered, but we add ours.
    engine.register("print", PrintNode);
    engine.register("output", OutputNode);

    // 3. Load Workflow from resources
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/simple_workflow.json");

    println!("Loading workflow from: {:?}", path);
    let json_content = fs::read_to_string(path).expect("Failed to read workflow file");

    let workflow_id = "simple-workflow";
    engine
        .load(workflow_id, &json_content)
        .expect("Failed to load workflow");

    // 4. Run Workflow
    let payload = serde_json::json!({
        "message": "Hello from Start Node"
    });
    let flow_context = Arc::new(FlowContext::new().with_payload(payload));
    
    let result = engine
        .run_with_ctx(workflow_id, flow_context)
        .await
        .expect("Failed to run workflow");

    // 5. Verify Result
    assert_eq!(result.status, FlowStatus::Succeeded);

    // The output of the workflow is the output of the last node (OutputNode)
    // In our JSON, OutputNode data is { "final_result": "${node-print.print_content}" }
    // node-print output is { "print_content": "Hello from Start Node" }
    // So OutputNode output should be { "final_result": "Hello from Start Node" }

    let output = result.output.expect("Workflow should have output");
    println!("Workflow Result: {:?}", output);

    assert_eq!(output["final_result"], "Hello from Start Node");
}
