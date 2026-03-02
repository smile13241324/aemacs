use crate::bus::{EventBus, SystemEvent};
use crate::signals::TimePulseSignal;
use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{info, warn};
use std::time::Duration;

/// Defines the generic interface for all sensory and autonomous trigger modules.
#[async_trait]
pub trait ReactiveObserver: Send + Sync {
    /// Returns the unique name of the observer.
    fn name(&self) -> &str;

    /// Executes the observer's logic, typically an infinite loop emitting signals.
    async fn run(&self, bus: EventBus) -> Result<()>;
}

/// A periodic heartbeat observer that wakes up autonomous agents at fixed intervals.
pub struct TimePulseObserver {
    pub interval_seconds: u64,
}

#[async_trait]
impl ReactiveObserver for TimePulseObserver {
    fn name(&self) -> &str {
        "TimePulseObserver"
    }

    async fn run(&self, bus: EventBus) -> Result<()> {
        info!(
            "⏰ [{}] Heartbeat awakening. Interval: {}s",
            self.name(),
            self.interval_seconds
        );

        let mut tick_count: u64 = 0;
        let mut interval = tokio::time::interval(Duration::from_secs(self.interval_seconds));

        loop {
            interval.tick().await;
            tick_count += 1;

            let signal = TimePulseSignal {
                tick_count,
                interval_seconds: self.interval_seconds,
            };

            let payload =
                serde_json::to_string(&signal).context("Failed to serialize TimePulseSignal")?;

            let system_event = SystemEvent::Signal {
                source: self.name().to_string(),
                event_type: "TimePulse".to_string(),
                payload,
            };

            if let Err(_) = bus.tx.send(system_event).await {
                warn!("EventBus disconnected. {} shutting down.", self.name());
                break;
            }
        }

        Ok(())
    }
}
