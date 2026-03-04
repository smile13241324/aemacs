use gpui::Global;
use std::path::PathBuf;
use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub enum SystemEvent {
    FileModified(PathBuf),
    Notification(String),
    OpenFile(PathBuf),
    PlanCreated(Vec<String>),
    TaskUpdated {
        index: usize,
        status: crate::task::TaskStatus,
    },
    PersonaChanged {
        name: String,
        message: Option<String>,
    },
    Signal {
        source: String,
        event_type: String,
        payload: String,
    },
}

#[derive(Clone)]
pub struct EventBus {
    pub tx: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<SystemEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Global for EventBus {}
