use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(default)]
    pub parent_id: Option<String>,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub data: Value,
    #[serde(default)]
    pub retry_policy: Option<RetryPolicy>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    #[serde(rename = "sourceHandle")]
    #[serde(default)]
    pub source_handle: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FlowStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct WorkflowResult {
    pub instance_id: u64,
    pub status: FlowStatus,
    pub output: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub interval_ms: u64,
}
