use async_trait::async_trait;
use serde_json::Value;
use crate::context::WorkflowContext;
use crate::error::WorkflowError;
pub use crate::model::NodeOutput;

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, ctx: &WorkflowContext, data: &Value) -> Result<NodeOutput, WorkflowError>;
}