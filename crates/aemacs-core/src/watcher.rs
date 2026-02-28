use crate::bus::{EventBus, SystemEvent};
use crate::signals::BufferModifiedSignal;
use anyhow::{Context, Result};
use log::{error, info, warn};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

const DEBOUNCE_DURATION: Duration = Duration::from_millis(500);
const MAX_PAYLOAD_SIZE: usize = 10 * 1024; // 10 KB

/// Spawns a background daemon that watches the workspace for file modifications.
pub async fn spawn_file_watcher(watch_path: PathBuf, bus: EventBus) -> Result<()> {
    let (tx, rx) = async_channel::unbounded();

    // 1. Initialize the watcher
    let mut watcher = RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            if let Ok(event) = res {
                let _ = tx.send_blocking(event);
            }
        },
        Config::default(),
    )
    .context("Failed to initialize watcher")?;

    info!("👁️  Watcher Daemon awakening for: {:?}", watch_path);
    watcher
        .watch(&watch_path, RecursiveMode::Recursive)
        .context("Failed to watch directory")?;

    // 2. The Worker Loop
    tokio::spawn(async move {
        let _watcher = watcher; // Keep it alive by moving it into the task
        let mut last_processed: HashMap<PathBuf, Instant> = HashMap::new();

        while let Ok(event) = rx.recv().await {
            // We only care about explicit data modifications
            if !matches!(
                event.kind,
                EventKind::Modify(notify::event::ModifyKind::Data(_))
            ) {
                continue;
            }

            for path in event.paths {
                if !path.is_file() {
                    continue;
                }

                // Filtering: Ignore noisy directories
                let path_str = path.to_string_lossy();
                if path_str.contains("/.git/")
                    || path_str.contains("/target/")
                    || path_str.contains("/node_modules/")
                    || path_str.contains("/.gemini/")
                {
                    continue;
                }

                // Debouncing
                let now = Instant::now();
                if let Some(last_time) = last_processed.get(&path) {
                    if now.duration_since(*last_time) < DEBOUNCE_DURATION {
                        continue; // Skip if it happened too recently
                    }
                }
                last_processed.insert(path.clone(), now);

                // Read File Content Safely
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Watcher could not read {:?}: {}", path, e);
                        continue;
                    }
                };

                let chunk = if content.len() > MAX_PAYLOAD_SIZE {
                    "[File Too Large - Please Use read_file Tool]".to_string()
                } else {
                    content
                };

                // Construct Signal
                let signal = BufferModifiedSignal {
                    file_path: path_str.into_owned(),
                    diff_chunk: chunk,
                };

                let payload = match serde_json::to_string(&signal) {
                    Ok(s) => s,
                    Err(e) => {
                        error!("Failed to serialize signal: {}", e);
                        continue;
                    }
                };

                // Emit to Event Bus
                let system_event = SystemEvent::Signal {
                    source: "Watcher".to_string(),
                    event_type: "BufferModified".to_string(),
                    payload,
                };

                if bus.tx.send(system_event).await.is_err() {
                    warn!("EventBus disconnected. Watcher shutting down.");
                    return;
                }
            }
        }
    });

    Ok(())
}
