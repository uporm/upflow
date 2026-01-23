mod start_node;
mod subflow_node;
mod switch_node;

pub use start_node::StartNode;
pub use subflow_node::SubflowNode;
pub use switch_node::DecisionNode;

use crate::models::context::NodeContext;
use crate::models::error::WorkflowError;
use async_trait::async_trait;
use serde_json::Value;

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, ctx: NodeContext) -> Result<Value, WorkflowError>;
    fn validate(&self, _data: &Value) -> Result<(), WorkflowError> {
        Ok(())
    }
}
