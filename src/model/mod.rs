use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

pub mod event;

pub use event::WorkflowEvent;

pub type NodeId = String;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    #[serde(rename = "sourceHandle")]
    #[serde(default)]
    pub source_handle: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub interval_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodePolicy {
    #[serde(default)]
    pub retry: Option<RetryPolicy>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub on_error: Option<OnErrorStrategy>,
    #[serde(default)]
    pub fallback_node_id: Option<String>,
}

impl NodePolicy {
    pub fn from_value(value: &Value) -> Self {
        let config = value.get("config").cloned().unwrap_or(Value::Null);
        serde_json::from_value(config).unwrap_or(NodePolicy {
            retry: None,
            timeout_ms: None,
            on_error: None,
            fallback_node_id: None,
        })
    }

    pub fn max_attempts(&self) -> u32 {
        self.retry.as_ref().map(|r| r.max_attempts).unwrap_or(1)
    }

    pub fn interval_ms(&self) -> u64 {
        self.retry.as_ref().map(|r| r.interval_ms).unwrap_or(0)
    }

    pub fn on_error(&self) -> OnErrorStrategy {
        self.on_error.clone().unwrap_or(OnErrorStrategy::FailFast)
    }
}


#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnErrorStrategy {
    FailFast,
    Continue,
    Fallback,
}

impl OnErrorStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            OnErrorStrategy::FailFast => "fail_fast",
            OnErrorStrategy::Continue => "continue",
            OnErrorStrategy::Fallback => "fallback",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowStatus {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug)]
pub struct WorkflowResult {
    pub instance_id: String,
    pub status: FlowStatus,
    pub output: Option<Value>,
}

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("workflow not found: {0}")]
    WorkflowNotFound(String),
    #[error("node executor not found: {0}")]
    NodeExecutorNotFound(String),
    #[error("invalid graph: {0}")]
    InvalidGraph(String),
    #[error("node execution failed: {0}")]
    NodeExecutionFailed(String),
    #[error("node panicked: {0}")]
    NodePanicked(String),
    #[error("timeout: {0}")]
    Timeout(String),
    #[error("cancelled")]
    Cancelled,
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("runtime error: {0}")]
    RuntimeError(String),
}
