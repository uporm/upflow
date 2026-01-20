use tokio::sync::broadcast;
use crate::model::event::WorkflowEvent;

#[derive(Clone)]
pub struct EventBus {
    sender: broadcast::Sender<WorkflowEvent>,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self { sender }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WorkflowEvent> {
        self.sender.subscribe()
    }

    pub fn emit(&self, event: WorkflowEvent) {
        let _ = self.sender.send(event);
    }
}
