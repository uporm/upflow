use crate::model::{node_trait::{NodeExecutor, NodeOutput}, workflow::Node, context::WorkflowContext};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use serde_json::json;

pub struct CaseNode;

#[async_trait]
impl NodeExecutor for CaseNode {
    async fn execute(&self, node: &Node, ctx: &mut WorkflowContext) -> Result<NodeOutput> {
        let var = node.data.get("var").and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("case missing var"))?;
        let current = ctx.get_var(var);
        let mut matched: Vec<String> = Vec::new();

        if let Some(cases) = node.data.get("cases").and_then(|v| v.as_object()) {
            if let Some(cur) = current.as_ref() {
                for (handle, expected) in cases {
                    if expected == cur { matched.push(handle.clone()); }
                }
            }
        }

        if matched.is_empty() {
            if let Some(def) = node.data.get("default").and_then(|v| v.as_str()) {
                matched.push(def.to_string());
            }
        }

        let handles = if matched.is_empty() { None } else { Some(matched) };
        Ok(NodeOutput { output: json!({"case": "ok"}), matched_handles: handles })
    }
}
