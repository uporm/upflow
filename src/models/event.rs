use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use crate::models::workflow::FlowStatus;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkflowEvent {
    FlowStarted {
        input: Arc<Value>,
        timestamp: DateTime<Utc>,
    },
    NodeStarted {
        node_id: String,
        node_type: String,
        data: Arc<Value>,
        input: Arc<Value>,
    },
    NodeCompleted {
        node_id: String,
        node_type: String,
        data: Arc<Value>,
        output: Arc<Value>,
        duration_ms: u64,
    },
    NodeMessage {
        node_id: String,
        node_type: String,
        data: Arc<Value>,
        message: Arc<Value>,
    },
    NodeError {
        node_id: String,
        node_type: String,
        data: Arc<Value>,
        error: String,
        strategy: String,
    },
    FlowFinished {
        status: FlowStatus,
        output: Option<Arc<Value>>,
        duration_ms: u64,
    },
}
