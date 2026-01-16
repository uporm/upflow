use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum NodeState {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct NodeRunState {
    pub state: NodeState,
    pub remaining: usize,
}
