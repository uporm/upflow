use async_trait::async_trait;
use serde_json::Value;

use crate::context::FlowContext;
use crate::engine::EventBus;
use crate::model::{Node, WorkflowError, WorkflowEvent};

pub struct NodeContext {
    pub node: Node,
    pub resolved_input: Value,
    pub flow_context: FlowContext,
    pub event_bus: EventBus,
}

impl NodeContext {
    pub fn send_message(&self, message: impl Into<Value>) {
        self.event_bus.emit(WorkflowEvent::NodeMessage {
            node_id: self.node.id.clone(),
            node_type: self.node.node_type.clone(),
            message: message.into(),
        });
    }
}

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError>;
}
