use gpui::Global;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum SystemEvent {
    FileModified(PathBuf),
    Notification(String),
    OpenFile(PathBuf),
}

#[derive(Clone)]
pub struct EventBus {
    pub tx: async_channel::Sender<SystemEvent>,
    pub rx: async_channel::Receiver<SystemEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, rx) = async_channel::unbounded();
        Self { tx, rx }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

impl Global for EventBus {}
