use crate::model::node_trait::{NodeExecutor, NodeOutput};
use crate::model::{workflow::Node, context::WorkflowContext};
use crate::engine::engine::WorkflowEngine;
use async_trait::async_trait;
use serde_json::json;
use anyhow::{Result, anyhow};
use uuid::Uuid;

pub struct SubflowNode;

impl SubflowNode {}

#[async_trait]
impl NodeExecutor for SubflowNode {
    async fn execute(&self, node: &Node, parent_ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        let wf_id = node.data.get("workflowId").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("subflow missing workflowId"))?;

        let wf_registry = parent_ctx.get_wf_registry().ok_or_else(|| anyhow!("engine env missing workflow registry"))?;
        let node_registry = parent_ctx.get_node_registry().ok_or_else(|| anyhow!("engine env missing node registry"))?;
        let event_bus = parent_ctx.get_event_bus().ok_or_else(|| anyhow!("engine env missing event bus"))?;

        let workflow = wf_registry.get(wf_id)
            .ok_or_else(|| anyhow!("subflow {} not found", wf_id))?;

        // 子上下文
        let child_ctx = parent_ctx.isolated_clone();

        // 输入映射
        if let Some(map) = node.data.get("inputMapping").and_then(|v| v.as_object()) {
            for (child, parent) in map {
                if let Some(v) = parent_ctx.get_var(parent.as_str().unwrap()) {
                    child_ctx.set_var(child, v);
                }
            }
        }

        // 执行子流程
        let run = WorkflowEngine::builder()
            .id(Uuid::new_v4().to_string())
            .flow(workflow)
            .executors(node_registry.clone())
            .flows(wf_registry.clone())
            .event_bus(event_bus.clone())
            .build();

        let child_ctx = run.start_with_ctx(child_ctx).await?;

        // 输出映射
        if let Some(map) = node.data.get("outputMapping").and_then(|v| v.as_object()) {
            for (parent, child) in map {
                if let Some(v) = child_ctx.get_var(child.as_str().unwrap()) {
                    parent_ctx.set_var(parent, v);
                }
            }
        }

        Ok(NodeOutput {
            output: json!({"subflow": "ok"}),
            matched_handles: None,
        })
    }
}
