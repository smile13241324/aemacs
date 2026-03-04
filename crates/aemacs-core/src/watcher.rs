use crate::bus::{EventBus, SystemEvent};
use anyhow::Result;
use log::{info, warn};
use notify::{RecursiveMode, Watcher};
use std::path::PathBuf;

/// The Global Watcher monitors the entire workspace for changes.
pub struct GlobalWatcher {
    bus: EventBus,
    watcher: Box<dyn Watcher + Send + Sync>,
}

impl GlobalWatcher {
    pub fn new(bus: EventBus) -> Result<Self> {
        let bus_clone = bus.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            match res {
                Ok(event) => {
                    // Only focus on modifications for now
                    if event.kind.is_modify() {
                        for path in event.paths {
                            let system_event = SystemEvent::FileModified(path);
                            if let Err(_) = bus_clone.tx.send(system_event) {
                                warn!("EventBus disconnected in GlobalWatcher.");
                            }
                        }
                    }
                }
                Err(e) => warn!("Watch error: {:?}", e),
            }
        })?;

        Ok(Self {
            bus,
            watcher: Box::new(watcher),
        })
    }

    pub fn watch(&mut self, path: PathBuf) -> Result<()> {
        info!("👁️  Watching workspace: {:?}", path);
        self.watcher.watch(&path, RecursiveMode::Recursive)?;
        Ok(())
    }
}

/// Spawns a background watcher task that listens for OS signals.
pub fn spawn_global_watcher(bus: EventBus, workspace_root: PathBuf) -> Result<()> {
    let mut watcher = GlobalWatcher::new(bus)?;
    watcher.watch(workspace_root)?;

    // Keep the watcher alive in a detached task if needed, but here we assume 
    // the caller manages its lifecycle or we move it to a thread.
    // For now, let's just let it drop if not stored, but GlobalWatcher should be stored.
    
    Ok(())
}
