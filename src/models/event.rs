use crate::models::workflow::Node;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkflowEvent {
    FlowStarted {
        instance_id: String,
        payload: Arc<Value>,
        nodes: Arc<Vec<Node>>,
        timestamp: DateTime<Utc>,
    },
    NodeStarted {
        node_id: String,
        node_type: String,
        data: Arc<Value>,
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
    FlowStopped {
        instance_id: String,
        timestamp: DateTime<Utc>,
    },
    FlowFinished {
        output: Option<Arc<Value>>,
        duration_ms: u64,
    },
}
