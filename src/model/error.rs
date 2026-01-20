use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorkflowError {
    #[error("workflow not found: {0}")]
    WorkflowNotFound(String),
    #[error("workflow node executor not found: {0}")]
    NodeExecutorNotFound(String),
    #[error("workflow invalid graph: {0}")]
    InvalidGraph(String),
    #[error("workflow node execution failed: {0}")]
    NodeExecutionFailed(String),
    #[error("workflow node panicked: {0}")]
    NodePanicked(String),
    #[error("workflow timeout: {0}")]
    Timeout(String),
    #[error("workflow cancelled")]
    Cancelled,
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("workflow runtime error: {0}")]
    RuntimeError(String),
    #[error("workflow validation error: {0}")]
    ValidationError(String),
    #[error("workflow instance id generation failed: {0}")]
    IdGenerateFailed(#[from] sonyflake::Error),
}
