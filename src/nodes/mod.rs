pub mod start;
pub mod decision;
pub mod subflow;
pub mod end;

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use crate::context::WorkflowContext;
use crate::error::WorkflowError;

#[derive(Debug, Default)]
pub struct NodeOutput {
    pub next_handle: Option<String>,
    pub updated_vars: HashMap<String, Value>,
}

#[async_trait]
pub trait NodeExecutor: Send + Sync {
    async fn execute(&self, ctx: &WorkflowContext, data: &Value) -> Result<NodeOutput, WorkflowError>;
}