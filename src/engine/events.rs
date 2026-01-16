use serde::Serialize;
use tokio::sync::broadcast;
use crate::model::state::NodeState;

/// 事件模型（序列化以便外部监听）
#[derive(Clone, Debug, Serialize)]
pub enum EngineEvent {
    RunStarted { run_id: String },
    NodeStateChanged { run_id: String, node_id: String, state: NodeState, message: Option<String> },
    RunFinished { run_id: String, success: bool },
}

#[derive(Clone)]
pub struct EventBus {
    pub sender: broadcast::Sender<EngineEvent>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(100)
    }
}

impl EventBus {
    pub fn new(buffer: usize) -> Self {
        let (sender, _) = broadcast::channel(buffer);
        Self { sender }
    }

    pub fn publish(&self, ev: EngineEvent) {
        let _ = self.sender.send(ev);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<EngineEvent> {
        self.sender.subscribe()
    }
    
    pub fn run_started(&self, run_id: &str) {
        self.publish(EngineEvent::RunStarted { run_id: run_id.to_string() });
    }

    pub fn run_finished(&self, run_id: &str, success: bool) {
        self.publish(EngineEvent::RunFinished { run_id: run_id.to_string(), success });
    }

    pub fn node_running(&self, run_id: &str, node_id: &str) {
        self.publish(EngineEvent::NodeStateChanged {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            state: NodeState::Running,
            message: None,
        });
    }

    pub fn node_success(&self, run_id: &str, node_id: &str) {
        self.publish(EngineEvent::NodeStateChanged {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            state: NodeState::Success,
            message: None,
        });
    }

    pub fn node_failed(&self, run_id: &str, node_id: &str, message: &str) {
        self.publish(EngineEvent::NodeStateChanged {
            run_id: run_id.to_string(),
            node_id: node_id.to_string(),
            state: NodeState::Failed,
            message: Some(message.to_string()),
        });
    }
}
