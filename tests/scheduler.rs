use async_trait::async_trait;
use uflow::engine::engine::WorkflowEngine;
use uflow::models::context::{FlowContext, NodeContext};
use uflow::models::error::WorkflowError;
use uflow::models::event_bus::EventBus;
use uflow::models::workflow::FlowStatus;
use uflow::nodes::NodeExecutor;
use serde_json::{json, Value};

struct SimpleExecutor;

#[async_trait]
impl NodeExecutor for SimpleExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved_input = ctx.flow_context.resolve_value(&ctx.node.data)?;
        Ok(resolved_input)
    }
}

struct SwitchExecutor;

#[async_trait]
impl NodeExecutor for SwitchExecutor {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError> {
        let resolved_input = ctx.flow_context.resolve_value(&ctx.node.data)?;
        let cases = resolved_input.get("cases").and_then(|v| v.as_array());
        if let Some(cases) = cases {
            for case in cases {
                let conditions = case.get("conditions").and_then(|v| v.as_array());
                if let Some(conds) = conditions
                    && let Some(cond) = conds.first()
                {
                    let val = cond.get("varId").and_then(|v| v.as_str()).unwrap_or("");
                    let target = cond.get("value").and_then(|v| v.as_str()).unwrap_or("");

                    if val == target {
                        let handle = case
                            .get("sourceHandle")
                            .and_then(|v| v.as_str())
                            .unwrap_or("default");
                        let mut map = serde_json::Map::new();
                        map.insert(handle.to_string(), json!({}));
                        return Ok(Value::Object(map));
                    }
                }
            }
        }
        let mut map = serde_json::Map::new();
        map.insert("default".to_string(), json!({}));
        Ok(Value::Object(map))
    }
}

#[tokio::test]
async fn test_execute_workflow() {
    let engine = WorkflowEngine::global();
    engine.register("start", SimpleExecutor);
    engine.register("print", SimpleExecutor);
    engine.register("case", SwitchExecutor);
    engine.register("end", SimpleExecutor);

    let json_data = r#"{
   "nodes": [ 
     { "id": "start_node", "type": "start", "data": { "name": "张三" } },
     { "id": "node1", "type": "print", "data": {} },
     { "id": "node2", "type": "print", "data": {} },
     { "id": "switch_node", "type": "case", "data": { 
         "cases": [ 
           { 
             "sourceHandle": "zhangsan", 
             "conditions": [ { "varId": "{{start_node.name}}", "value": "张三" } ]
           } 
         ], 
         "else": { "sourceHandle": "default" } 
       } 
     },
     { "id": "print1", "type": "print", "data": { "msg": "Hello Zhangsan" } },
     { "id": "print2", "type": "print", "data": { "msg": "Hello Default" } },
     { "id": "end-node", "type": "end", "data": {} }
   ], 
   "edges": [ 
     { "source": "start_node", "target": "node1" },
     { "source": "start_node", "target": "node2" },
     { "source": "node1", "target": "switch_node" },
     { "source": "node2", "target": "switch_node" },
     { "source": "switch_node", "target": "print1", "sourceHandle": "zhangsan" },
     { "source": "switch_node", "target": "print2", "sourceHandle": "default" },
     { "source": "print1", "target": "end-node" },
     { "source": "print2", "target": "end-node" }
   ] 
 }"#;

    let workflow_obj: Value = serde_json::from_str(json_data).unwrap();
    let final_json = json!({
        "id": "test_wf_scheduler",
        "nodes": workflow_obj["nodes"],
        "edges": workflow_obj["edges"]
    })
    .to_string();

    let workflow_id = "test_wf_scheduler";
    engine.load(workflow_id, &final_json).expect("Failed to load graph");

    let flow_context = FlowContext::new();
    let event_bus = EventBus::new(100);

    let result = engine
        .run_with_ctx_event(workflow_id, flow_context.clone(), event_bus)
        .await;

    if let Err(e) = &result {
        println!("Execution failed: {:?}", e);
    }
    assert!(result.is_ok());
    let res = result.unwrap();
    assert_eq!(res.status, FlowStatus::Succeeded);

    let print1_res = flow_context.get_result("print1");
    assert!(print1_res.is_some(), "print1 should have run");

    let print2_res = flow_context.get_result("print2");
    assert!(print2_res.is_none(), "print2 should be skipped");
}
