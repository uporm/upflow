use crate::context::WorkflowContext;
use crate::error::WorkflowError;
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use crate::model::NodeOutput;
use crate::nodes::node_trait::NodeExecutor;

pub struct EndNode;

#[async_trait]
impl NodeExecutor for EndNode {
    async fn execute(&self, _ctx: &WorkflowContext, _data: &Value) -> Result<NodeOutput, WorkflowError> {
        Ok(NodeOutput {
            next_handle: None,
            updated_vars: HashMap::new(),
        })
    }
}
