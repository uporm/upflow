use std::sync::Arc;
use uflow::engine::{engine::WorkflowEngine, events::{EventBus, EngineEvent}};
use uflow::model::context::WorkflowContext;
use uflow::nodes::{start::StartNode, end::EndNode, assign::AssignNode, subflow::SubflowNode};
use uflow::registry::{workflow_registry::WorkflowRegistry, node_registry::NodeRegistry};

#[tokio::test]
async fn main_subflow_runs() {
    let event_bus = EventBus::new(64);
    let wf_registry = Arc::new(WorkflowRegistry::new());
    let node_registry = Arc::new(NodeRegistry::new());

    let nr_start = node_registry.clone();
    nr_start.register("start", StartNode);
    let nr_end = node_registry.clone();
    nr_end.register("end", EndNode);
    let nr_assign = node_registry.clone();
    nr_assign.register("assign", AssignNode);
    node_registry.register("subflow", SubflowNode);

    let sub_json = std::fs::read_to_string("tests/resources/subflow.json").unwrap();
    wf_registry.register_str("user_flow", &sub_json).unwrap();

    let main_json = std::fs::read_to_string("tests/resources/main_flow.json").unwrap();

    let run = WorkflowEngine::builder()
        .id("run1".to_string())
        .flow_json(&main_json)
        .flows(wf_registry.clone())
        .executors(node_registry.clone())
        .event_bus(event_bus.clone())
        .build();
    let mut rx = event_bus.subscribe();
    let _ = run.start().await;
    let mut ok = false;
    loop {
        match rx.recv().await.unwrap() {
            EngineEvent::RunFinished { run_id, success } if run_id == "run1" => { ok = success; break; }
            _ => {}
        }
    }
    assert!(ok);
}

#[tokio::test]
async fn subflow_input_output_mapping() {
    let event_bus = EventBus::new(64);
    let wf_registry = Arc::new(WorkflowRegistry::new());
    let node_registry = Arc::new(NodeRegistry::new());

    let nr_start = node_registry.clone();
    nr_start.register("start", StartNode);
    let nr_end = node_registry.clone();
    nr_end.register("end", EndNode);
    let nr_assign = node_registry.clone();
    nr_assign.register("assign", AssignNode);
    node_registry.register("subflow", SubflowNode);

    let sub_json = std::fs::read_to_string("tests/resources/subflow.json").unwrap();
    wf_registry.register_str("user_flow", &sub_json).unwrap();

    let main_json = r#"{
        "nodes": [
          { "id": "start", "type": "start", "data": {} },
          { "id": "sub", "type": "subflow", "data": { "workflowId": "user_flow", "inputMapping": {"x": "p_in"}, "outputMapping": {"p_out": "x"} } },
          { "id": "end", "type": "end", "data": {} }
        ],
        "edges": [
          { "source": "start", "target": "sub" },
          { "source": "sub", "target": "end" }
        ]
    }"#;
    let main_wf_json = main_json;

    let run = WorkflowEngine::builder()
        .id("run2".to_string())
        .flow_json(main_wf_json)
        .flows(wf_registry.clone())
        .executors(node_registry.clone())
        .event_bus(event_bus.clone())
        .build();
    let parent_ctx = WorkflowContext::new();
    parent_ctx.set_var("p_in", serde_json::json!(50));
    let ctx = run.start_with_ctx(parent_ctx).await.unwrap();
    let out = ctx.get_var("p_out").unwrap();
    assert_eq!(out, serde_json::json!(100));
}
