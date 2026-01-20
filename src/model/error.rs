use thiserror::Error;

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
