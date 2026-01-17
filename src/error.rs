use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum WorkflowError {
    #[error("Node not found: {0}")]
    NodeNotFound(String),
    #[error("Workflow definition '{0}' not found")]
    WorkflowNotFound(String),
    #[error("Config error: {0}")]
    ConfigError(String),
    #[error("Runtime error: {0}")]
    RuntimeError(String),
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl From<serde_json::Error> for WorkflowError {
    fn from(e: serde_json::Error) -> Self {
        Self::SerializationError(e.to_string())
    }
}