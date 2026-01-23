use crate::models::event::WorkflowEvent;
use tokio::sync::broadcast;

const DEFAULT_EVENT_BUS_CAPACITY: usize = 500;

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

impl Default for EventBus {
    fn default() -> Self {
        Self::new(DEFAULT_EVENT_BUS_CAPACITY)
    }
}
