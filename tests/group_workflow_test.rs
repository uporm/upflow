use async_trait::async_trait;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use upflow::prelude::*;

struct PrintNode;

#[async_trait]
impl NodeExecutor for PrintNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        Ok(resolved)
    }
}

struct OutputNode;

#[async_trait]
impl NodeExecutor for OutputNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        Ok(resolved)
    }
}

struct EndNode;

#[async_trait]
impl NodeExecutor for EndNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        Ok(resolved)
    }
}

#[tokio::test]
async fn test_group_workflow() {
    let engine = WorkflowEngine::global();
    engine.register("print", PrintNode);
    engine.register("output", OutputNode);
    engine.register("end", EndNode);
    engine.register("group__test", GroupNode);

    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/resources/group_workflow.json");

    let json_content = fs::read_to_string(path).expect("Failed to read workflow file");
    let workflow_id = "group-workflow";

    engine
        .load(workflow_id, &json_content)
        .expect("Failed to load workflow");

    // 设置事件监听
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let listener_handle = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            // println!("收到事件: {:?}", event);
            events_clone.lock().unwrap().push(event.clone());
            if let WorkflowEvent::FlowFinished { .. } = event {
                // 主流程结束时退出。
            }
        }
    });

    let payload = serde_json::json!({
        "message": "开始",
        "inner_msg": "内部"
    });
    let flow_context = Arc::new(FlowContext::new().with_payload(payload));

    let result = engine
        .run_with_ctx_event(workflow_id, flow_context, event_bus)
        .await
        .expect("Failed to run workflow");

    // 等待一小段时间让所有事件被处理（虽然 FlowFinished 后应该没多少了，但为了保险）
    // 或者直接取消 listener
    tokio::time::sleep(Duration::from_millis(100)).await;
    listener_handle.abort();

    assert_eq!(result.status, FlowStatus::Succeeded);

    let output = result.output.expect("Workflow should have output");
    println!("Workflow Result: {:?}", output);

    assert_eq!(output["final_result"], "开始 -> 内部");

    // 验证事件
    let collected_events = events.lock().unwrap();
    println!("总共收到 {} 个事件", collected_events.len());

    // 打印所有事件以便调试
    for e in collected_events.iter() {
        println!("Event: {:?}", e);
    }

    // 验证是否包含组内节点的事件
    let has_inner_start = collected_events.iter().any(|e| {
        if let WorkflowEvent::NodeStarted { node_id, .. } = e {
            node_id == "inner-start"
        } else {
            false
        }
    });
    assert!(has_inner_start, "应该收到内部节点 inner-start 的启动事件");

    // 验证是否包含组节点的事件
    let has_group_node = collected_events.iter().any(|e| {
        if let WorkflowEvent::NodeStarted { node_id, .. } = e {
            node_id == "node-group"
        } else {
            false
        }
    });
    assert!(has_group_node, "应该收到组节点 node-group 的启动事件");
}
