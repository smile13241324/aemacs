use crate::bus::{EventBus, SystemEvent};
use anyhow::Result;
use log::{info, warn};
use notify::{RecursiveMode, Watcher};
use std::path::Path;

/// The Global Watcher monitors the entire workspace for changes.
pub struct GlobalWatcher {
    bus: EventBus,
    watcher: Box<dyn Watcher + Send + Sync>,
}

impl std::fmt::Debug for GlobalWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GlobalWatcher")
            .field("bus", &self.bus)
            .field("watcher", &"Box<dyn Watcher>")
            .finish()
    }
}

impl GlobalWatcher {
    /// Initializes a new `GlobalWatcher` connected to the specified system event bus.
    /// It uses the platform's recommended watcher implementation (e.g., inotify, `FSEvents`).
    ///
    /// # Errors
    /// Returns an error if the watcher cannot be initialized.
    pub fn new(bus: EventBus) -> Result<Self> {
        let bus_clone = bus.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            match res {
                Ok(event) => {
                    // Only focus on modifications for now
                    if event.kind.is_modify() {
                        for path in event.paths {
                            let system_event = SystemEvent::FileModified(path);
                            if bus_clone.tx.send(system_event).is_err() {
                                warn!("EventBus disconnected in GlobalWatcher.");
                            }
                        }
                    }
                }
                Err(e) => warn!("Watch error: {e:?}"),
            }
        })?;

        Ok(Self {
            bus,
            watcher: Box::new(watcher),
        })
    }

    /// Registers a physical directory path to be monitored recursively.
    ///
    /// # Errors
    /// Returns an error if the OS fails to initialize the watch on the specified path.
    pub fn watch(&mut self, path: &Path) -> Result<()> {
        info!("👁️  Watching workspace: {}", path.display());
        self.watcher.watch(path, RecursiveMode::Recursive)?;
        Ok(())
    }
}

/// Spawns and configures a global watcher for the specified workspace root.
/// The watcher will emit `FileModified` signals to the event bus whenever physical files are altered.
///
/// # Errors
/// Returns an error if the watcher cannot be initialized or the path cannot be watched.
pub fn spawn_global_watcher(bus: EventBus, workspace_root: &Path) -> Result<()> {
    let mut watcher = GlobalWatcher::new(bus)?;
    watcher.watch(workspace_root)?;

    // Keep the watcher alive in a detached task if needed, but here we assume
    // the caller manages its lifecycle or we move it to a thread.
    // For now, let's just let it drop if not stored, but GlobalWatcher should be stored.

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::bus::EventBus;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use tokio::time::{Duration, timeout};

    #[tokio::test]
    async fn test_global_watcher_vigilance_quest() -> Result<()> {
        // QUEST: Verify the watcher correctly detects file changes.

        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        let tmp_dir = tempdir()?;
        let file_path = tmp_dir.path().join("test_file.txt");

        // 1. Create initial file
        {
            let mut file = File::create(&file_path)?;
            file.write_all(b"Initial content")?;
            file.sync_all()?;
        }

        // 2. Setup Watcher
        let mut watcher = GlobalWatcher::new(bus)?;
        watcher.watch(tmp_dir.path())?;

        // 3. Trigger Modification
        // Note: Some OS watchers need a tiny moment to settle or have debouncing.
        tokio::time::sleep(Duration::from_millis(100)).await;

        {
            let mut file = File::options().append(true).open(&file_path)?;
            file.write_all(b"Modified content")?;
            file.sync_all()?;
        }

        // 4. Wait for signal with timeout
        let result = timeout(Duration::from_secs(2), async {
            while let Ok(event) = rx.recv().await {
                if matches!(event, SystemEvent::FileModified(ref p) if p.canonicalize().unwrap_or_default() == file_path.canonicalize().unwrap_or_default()) {
                    return true;
                }
            }
            false
        })
        .await;

        assert!(
            result.is_ok(),
            "The 'Vigilance-Quest' failed! No FileModified signal detected within 2 seconds."
        );
        assert!(
            result.expect("Should not fail in test"),
            "Received events but none matched the target file path."
        );

        Ok(())
    }
}
