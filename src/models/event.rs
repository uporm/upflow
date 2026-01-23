use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::models::workflow::FlowStatus;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkflowEvent {
    FlowStarted {
        id: u64,
        input: Value,
        timestamp: DateTime<Utc>,
    },
    NodeStarted {
        node_id: String,
        node_type: String,
        input: Value,
    },
    NodeCompleted {
        node_id: String,
        node_type: String,
        output: Value,
        duration_ms: u64,
    },
    NodeMessage {
        node_id: String,
        node_type: String,
        message: Value,
    },
    NodeError {
        node_id: String,
        node_type: String,
        error: String,
        strategy: String,
    },
    FlowFinished {
        id: u64,
        status: FlowStatus,
        output: Option<Value>,
    },
}
