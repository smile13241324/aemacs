use std::{path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{info, warn};
use notify::{RecursiveMode, Watcher};

use crate::{
    bus::{EventBus, SystemEvent},
    signals::TimePulseSignal,
};

/// Defines the generic interface for all sensory and autonomous trigger modules.
#[async_trait]
pub trait ReactiveObserver: Send + Sync {
    /// Returns the unique name of the observer.
    fn name(&self) -> &str;

    /// Executes the observer's logic, typically an infinite loop emitting signals.
    async fn run(&self, bus: EventBus) -> Result<()>;
}

/// A periodic heartbeat observer that wakes up autonomous agents at fixed intervals.
#[derive(Debug)]
pub struct TimePulseObserver {
    pub interval_seconds: u64,
}

#[async_trait]
impl ReactiveObserver for TimePulseObserver {
    fn name(&self) -> &'static str {
        "TimePulseObserver"
    }

    async fn run(&self, bus: EventBus) -> Result<()> {
        info!("⏰ [{}] Heartbeat awakening. Interval: {}s", self.name(), self.interval_seconds);

        let mut tick_count: u64 = 0;
        let mut interval = tokio::time::interval(Duration::from_secs(self.interval_seconds));

        loop {
            interval.tick().await;
            tick_count += 1;

            let signal = TimePulseSignal { tick_count, interval_seconds: self.interval_seconds };

            let payload =
                serde_json::to_string(&signal).context("Failed to serialize TimePulseSignal")?;

            let system_event = SystemEvent::Signal {
                source: self.name().to_string(),
                event_type: "TimePulse".to_string(),
                payload,
            };

            if bus.tx.send(system_event).is_err() {
                warn!("EventBus disconnected. {} shutting down.", self.name());
                break;
            }
        }

        Ok(())
    }
}

/// ACO-026: Watches the filesystem for changes and emits signals to wake the agent.
#[derive(Debug)]
pub struct FileWatcherObserver {
    pub path: PathBuf,
}

#[async_trait]
impl ReactiveObserver for FileWatcherObserver {
    fn name(&self) -> &'static str {
        "FileWatcherObserver"
    }

    async fn run(&self, bus: EventBus) -> Result<()> {
        info!("👁️  [{}] Watching path: {:?}", self.name(), self.path);

        let (tx, mut rx) = tokio::sync::mpsc::channel(10);

        let mut watcher =
            notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    // Only trigger on data modifications/saves
                    if event.kind.is_modify() {
                        let _ = tx.blocking_send(event);
                    }
                }
            })?;

        watcher.watch(&self.path, RecursiveMode::Recursive)?;

        while let Some(event) = rx.recv().await {
            for path in event.paths {
                let rel_path = path.strip_prefix(&self.path).unwrap_or(&path);

                // ACO-029-03: Contextual Flooding (Quick Read)
                let snippet = if path.is_file() {
                    std::fs::read_to_string(&path).map_or(None, |content| {
                        let lines: Vec<&str> = content.lines().take(50).collect();
                        Some(lines.join("\n"))
                    })
                } else {
                    None
                };

                let payload = snippet.map_or_else(
                    || rel_path.to_string_lossy().to_string(),
                    |s| format!("{}@@{}", rel_path.to_string_lossy(), s),
                );

                let system_event = SystemEvent::Signal {
                    source: "FileSystem".to_string(),
                    event_type: "FileSaved".to_string(),
                    payload,
                };

                if bus.tx.send(system_event).is_err() {
                    warn!("EventBus disconnected. {} shutting down.", self.name());
                    return Ok(());
                }
            }
        }

        Ok(())
    }
}

/// ACO-030: Monitors an IMAP mailbox for new task intents from authorized senders.
#[derive(Debug)]
pub struct EmailObserver {
    pub server: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub authorized_senders: Vec<String>,
}

#[async_trait]
impl ReactiveObserver for EmailObserver {
    fn name(&self) -> &'static str {
        "EmailObserver"
    }

    async fn run(&self, _bus: EventBus) -> Result<()> {
        info!(
            "📬 [{}] Stub active. Real-time Email listening requires crate API alignment.",
            self.name()
        );

        // Placeholder loop to keep the observer alive
        loop {
            tokio::time::sleep(Duration::from_hours(1)).await;
        }
    }
}

/// ACO-030: Monitors a Matrix room for mentions and direct messages.
#[derive(Debug)]
pub struct ChatObserver {
    pub homeserver: String,
    pub access_token: String,
}

#[async_trait]
impl ReactiveObserver for ChatObserver {
    fn name(&self) -> &'static str {
        "ChatObserver"
    }

    async fn run(&self, _bus: EventBus) -> Result<()> {
        info!(
            "💬 [{}] Stub active. Real-time Chat listening requires crate API alignment.",
            self.name()
        );

        // Placeholder loop to keep the observer alive
        loop {
            tokio::time::sleep(Duration::from_hours(1)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::EventBus;

    #[tokio::test]
    async fn test_time_pulse_emission_quest() -> Result<()> {
        let bus = EventBus::new();
        let observer = TimePulseObserver { interval_seconds: 1 };

        // Quest: Spawn the pulse in the background
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let _ = observer.run(bus_clone).await;
        });

        // 1. Wait for the first tick (tokio interval ticks immediately on first call)
        let mut rx = bus.subscribe();

        // Check for the first tick signal
        let event = tokio::time::timeout(Duration::from_millis(500), rx.recv()).await??;

        if let SystemEvent::Signal { source, event_type, .. } = event {
            assert_eq!(source, "TimePulseObserver");
            assert_eq!(event_type, "TimePulse");
        } else {
            return Err(anyhow::anyhow!("Received unexpected event type from TimePulseObserver!"));
        }

        // 2. Wait for second tick
        let event = tokio::time::timeout(Duration::from_millis(1500), rx.recv()).await??;
        if let SystemEvent::Signal { event_type, .. } = event {
            assert_eq!(event_type, "TimePulse");
        } else {
            return Err(anyhow::anyhow!("Second tick failed to arrive!"));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_file_watcher_signal_quest() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let bus = EventBus::new();
        let observer = FileWatcherObserver { path: temp_dir.path().to_path_buf() };

        // Quest: Spawn watcher
        let bus_clone = bus.clone();
        tokio::spawn(async move {
            let _ = observer.run(bus_clone).await;
        });

        // Give it a moment to initialize
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 1. Trigger a file modification
        let test_file = temp_dir.path().join("test.rs");
        let content = "pub fn truth() -> bool { true }";
        std::fs::write(&test_file, content)?;

        // 2. Wait for signal
        let mut rx = bus.subscribe();

        // We use a loop because other signals (like TimePulse) might be on the bus in a real app,
        // but here it's a fresh bus. However, notify might emit multiple events.
        let mut found_content = false;
        for _ in 0..10 {
            if let Ok(Ok(SystemEvent::Signal { source, event_type, payload })) =
                tokio::time::timeout(Duration::from_secs(1), rx.recv()).await
                && source == "FileSystem"
                && event_type == "FileSaved"
                && payload.contains("@@")
                && payload.contains(content)
            {
                found_content = true;
                break;
            }
        }

        assert!(found_content, "FileWatcherObserver failed to flood the signal with file context!");

        Ok(())
    }
}
