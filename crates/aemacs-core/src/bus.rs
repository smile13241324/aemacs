use gpui::Global;
use std::path::PathBuf;
use tokio::sync::broadcast;

/// Represents all possible events that can be broadcast across the system.
/// This includes file changes, user notifications, agent transitions, and sensory signals.
#[derive(Debug, Clone)]
pub enum SystemEvent {
    /// A physical file on disk has been modified.
    FileModified(PathBuf),
    /// A high-level notification for the user.
    Notification(String),
    /// A request to open a specific file in the editor.
    OpenFile(PathBuf),
    /// A new project plan has been initialized.
    PlanCreated(Vec<String>),
    /// A task within the current plan has changed its status.
    TaskUpdated {
        index: usize,
        status: crate::task::TaskStatus,
    },
    /// A request to switch the active agent persona.
    PersonaChanged {
        name: String,
        message: Option<String>,
    },
    /// A telemetry signal emitted by an agent or sensory substrate.
    Signal {
        source: String,
        event_type: String,
        payload: String,
    },
}

/// The central communication hub for the Æmacs system.
/// It uses a broadcast channel to allow multiple components to listen for and emit events.
#[derive(Clone)]
pub struct EventBus {
    /// The sending end of the system-wide broadcast channel.
    pub tx: broadcast::Sender<SystemEvent>,
}

impl EventBus {
    /// Creates a new EventBus with a default channel capacity.
    pub fn new() -> Self {
        let (tx, _rx) = broadcast::channel(1024);
        Self { tx }
    }

    /// Returns a new receiver for subscribing to system events.
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
