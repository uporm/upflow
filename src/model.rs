use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorkflowDefinition {
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    pub data: Value,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Edge {
    pub source: String,
    #[serde(rename = "sourceHandle")]
    pub source_handle: Option<String>,
    pub target: String,
}

#[derive(Debug, Default)]
pub struct NodeOutput {
    pub next_handle: Option<String>,
    pub updated_vars: HashMap<String, Value>,
}