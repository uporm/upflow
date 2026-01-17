use crate::context::WorkflowContext;
use crate::error::WorkflowError;
use async_trait::async_trait;
use serde_json::Value;
use crate::model::NodeOutput;
use crate::nodes::node_trait::NodeExecutor;

pub struct StartNode;

#[async_trait]
impl NodeExecutor for StartNode {
    async fn execute(&self, _ctx: &WorkflowContext, _data: &Value) -> Result<NodeOutput, WorkflowError> {
        // 生产级：这里可以增加输入 Schema 的校验逻辑
        Ok(NodeOutput::default())
    }
}