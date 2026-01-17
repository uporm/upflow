use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkflowDefinition {
    pub nodes: Vec<NodeConfig>,
    pub edges: Vec<EdgeConfig>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NodeConfig {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub data: Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct EdgeConfig {
    pub source: String,
    #[serde(rename = "sourceHandle")]
    pub source_handle: Option<String>,
    pub target: String,
}