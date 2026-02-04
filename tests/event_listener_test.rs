use async_trait::async_trait;
use serde_json::json;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tokio::time::timeout;
use upflow::prelude::*;

struct PrintNode;

#[async_trait]
impl NodeExecutor for PrintNode {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved = ctx.flow_context.resolve_value(ctx.node.data.as_ref())?;
        println!("PrintNode 线程号: {:?}", thread::current().id());
        println!("执行 PrintNode: {:?}", resolved);
        Ok(resolved)
    }
}

#[tokio::test]
async fn test_workflow_with_event_listener() {
    // 1. 初始化引擎和事件总线
    let engine = WorkflowEngine::global();
    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();

    // 2. 注册自定义节点
    engine.register("print", PrintNode);

    // 3. 定义并加载工作流
    // 创建一个简单的线性工作流: Start -> Print
    let workflow_json = json!({
        "nodes": [
            { "id": "node-start", "type": "start", "data": { "message": "Hello Events" } },
            { "id": "node-print", "type": "print", "data": { "content": "{{node-start.message}}" } }
        ],
        "edges": [
            { "source": "node-start", "target": "node-print" }
        ]
    });

    let workflow_id = "event-test-workflow";
    engine
        .load(workflow_id, &workflow_json.to_string())
        .expect("Failed to load workflow");

    // 4. 启动事件监听任务
    // 使用 Arc<Mutex<Vec>> 来在任务间共享收集到的事件
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();

    let listener_handle = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            println!("收到事件: {:?}", event);
            events_clone.lock().unwrap().push(event.clone());
            
            // 如果是 FlowFinished 事件，我们可以退出循环
            if let WorkflowEvent::FlowFinished { .. } = event {
                break;
            }
        }
    });

    // 5. 运行工作流
    println!("开始运行工作流...");
    // 使用 run_with_event 方法传入我们创建的 event_bus
    let result = engine
        .run_with_event(workflow_id, event_bus)
        .await
        .expect("Failed to run workflow");

    // 6. 等待监听器完成
    // 使用 timeout 防止如果未收到 FlowFinished 事件导致的无限等待
    timeout(Duration::from_secs(2), listener_handle)
        .await
        .expect("Listener timed out")
        .expect("Listener task failed");

    // 7. 验证结果
    assert_eq!(result.status, FlowStatus::Succeeded);
    
    let collected_events = events.lock().unwrap();
    println!("总共收到 {} 个事件", collected_events.len());

    // 验证关键事件是否存在
    
    // 检查 FlowStarted
    let has_flow_started = collected_events
        .iter()
        .any(|e| matches!(e, WorkflowEvent::FlowStarted { .. }));
    assert!(has_flow_started, "缺少 FlowStarted 事件");

    let flow_started_nodes = collected_events
        .iter()
        .find_map(|e| {
            if let WorkflowEvent::FlowStarted { nodes, .. } = e {
                Some(nodes.clone())
            } else {
                None
            }
        })
        .expect("缺少 FlowStarted 节点列表");
    let flow_started_ids: Vec<String> = flow_started_nodes
        .iter()
        .map(|node| node.id.clone())
        .collect();
    assert_eq!(
        flow_started_ids,
        vec!["node-start".to_string(), "node-print".to_string()]
    );
    
    // 检查 FlowFinished
    let has_flow_finished = collected_events
        .iter()
        .any(|e| matches!(e, WorkflowEvent::FlowFinished { .. }));
    assert!(has_flow_finished, "缺少 FlowFinished 事件");

    // 检查节点事件
    // 应该有两个节点启动 (start, print) 和两个节点完成
    let node_started_count = collected_events.iter().filter(|e| matches!(e, WorkflowEvent::NodeStarted { .. })).count();
    let node_completed_count = collected_events.iter().filter(|e| matches!(e, WorkflowEvent::NodeCompleted { .. })).count();
    
    assert_eq!(node_started_count, 2, "应该有两个节点启动事件 (start, print)");
    assert_eq!(node_completed_count, 2, "应该有两个节点完成事件 (start, print)");

    // 验证特定节点的事件顺序（简单的检查）
    // 确保 start 节点先于 print 节点
    let mut start_node_seen = false;
    let mut print_node_seen = false;

    for event in collected_events.iter() {
        if let WorkflowEvent::NodeStarted { node_id, .. } = event {
            if node_id == "node-start" {
                start_node_seen = true;
            } else if node_id == "node-print" {
                print_node_seen = true;
                assert!(start_node_seen, "Print 节点启动前 Start 节点应该已经启动");
            }
        }
    }
    assert!(start_node_seen && print_node_seen, "所有节点都应该被执行");
}
