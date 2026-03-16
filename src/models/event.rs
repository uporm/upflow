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
        instance_id: String,
        node_id: String,
        node_type: String,
        data: Arc<Value>,
    },
    NodeCompleted {
        instance_id: String,
        node_id: String,
        node_type: String,
        data: Arc<Value>,
        output: Arc<Value>,
        duration_ms: u64,
    },
    NodeMessage {
        instance_id: String,
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
        instance_id: String,
        output: Option<Arc<Value>>,
        duration_ms: u64,
    },
}
