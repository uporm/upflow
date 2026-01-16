use uuid::Uuid;
use tokio::sync::broadcast::error::RecvError;
use uflow::engine::engine::WorkflowEngine;
use uflow::engine::events::EventBus;
use uflow::nodes::assign::AssignNode;
use uflow::nodes::case::CaseNode;
use uflow::nodes::end::EndNode;
use uflow::nodes::start::StartNode;
use uflow::nodes::subflow::SubflowNode;
use uflow::registry::node_registry::NodeRegistry;
use uflow::registry::workflow_registry::WorkflowRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = std::sync::Arc::new(NodeRegistry::new());
    let wf_registry = std::sync::Arc::new(WorkflowRegistry::new());
    registry.register("start", StartNode);
    registry.register("case", CaseNode);
    registry.register("assign", AssignNode);
    registry.register("subflow", SubflowNode);
    registry.register("end", EndNode);

    let sub_json = include_str!("resources/subflow.json");
    wf_registry.register_str("user_flow", sub_json).unwrap();
    let json = include_str!("resources/main_flow.json");

    let event_bus = EventBus::new(100);
    let mut rx = event_bus.subscribe();

    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(ev) => println!("EVENT => {:?}", ev),
                Err(RecvError::Closed) => break,
                Err(RecvError::Lagged(n)) => println!("lagged {} messages", n),
            }
        }
    });

    let run_id = Uuid::new_v4().to_string();
    let run = WorkflowEngine::builder()
        .id(run_id)
        .flow_json(json)
        .flows(wf_registry)
        .executors(registry)
        .event_bus(event_bus)
        .build();
    run.start().await?;

    Ok(())
}
