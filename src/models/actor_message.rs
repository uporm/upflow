use serde_json::Value;
use std::sync::Arc;

pub enum ActorMessage {
    Execute { node_id: String, spawn: bool },
    NodeCompleted { node_id: String, output: Arc<Value> },
    NodeSkipped { node_id: String },
    NodeFailed { node_id: String, error: String },
}
