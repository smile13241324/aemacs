use crate::bus::{EventBus, SystemEvent};
use anyhow::Result;
use log::{info, warn};
use std::time::{Duration, Instant};

/// The Sentinel Daemon monitors autonomous agent sanity by tracking status reports.
/// If an agent fails to report functional integrity for over 1 hour, it triggers a rescue notification.
pub async fn spawn_sentinel(bus: EventBus) -> Result<()> {
    let bus_clone = bus.clone();
    spawn_sentinel_internal(
        bus,
        Duration::from_hours(1),
        Duration::from_mins(1),
        move |msg| {
            let tx = bus_clone.tx.clone();
            let _ = tx.send(SystemEvent::Notification(msg));
        },
    )
    .await
}

async fn spawn_sentinel_internal<F>(
    bus: EventBus,
    timeout: Duration,
    check_interval: Duration,
    on_alert: F,
) -> Result<()>
where
    F: Fn(String) + Send + Sync + 'static,
{
    tokio::spawn(async move {
        info!("🛡️ Sentinel Daemon awakened. Monitoring agent sanity...");

        let mut last_report = Instant::now();
        let mut rx = bus.subscribe();

        loop {
            // Non-blocking drain of the event bus to find status reports
            while let Ok(event) = rx.try_recv() {
                if matches!(event, SystemEvent::Signal { ref event_type, .. } if event_type == "StatusReport") {
                    last_report = Instant::now();
                    info!("🛡️ Sentinel: Agent check-in received. Sanity confirmed.");
                }
            }

            // Check for silence
            if last_report.elapsed() > timeout {
                warn!(
                    "🚨 [SENTINEL ALERT] Agent silence exceeds {timeout:?}! Sanity compromised."
                );

                on_alert(
                    "Sentinel Alert: Agent Sanity Compromised. Automated Rescue Protocol required."
                        .to_string(),
                );

                // Reset timer to avoid spamming alerts
                last_report = Instant::now();
            }

            tokio::time::sleep(check_interval).await;
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use super::*;
    use crate::bus::EventBus;
    use std::sync::{Arc, Mutex};

    #[tokio::test]
    async fn test_sentinel_vigilance_quest() -> Result<()> {
        let bus = EventBus::new();
        let timeout = Duration::from_millis(300);
        let check_interval = Duration::from_millis(50);

        let alert_triggered = Arc::new(Mutex::new(false));
        let alert_clone = alert_triggered.clone();

        // Quest: Spawn the Sentinel with a short fuse and a local observer
        spawn_sentinel_internal(bus.clone(), timeout, check_interval, move |_| {
            *alert_clone.lock().expect("Should not fail in test") = true;
        })
        .await?;

        // 1. Verify Silence Detection
        tokio::time::sleep(Duration::from_millis(500)).await;

        assert!(*alert_triggered.lock().expect("Should not fail in test"), "The Sentinel slept while the agent was silent!");

        // 2. Verify Reset Logic
        // Reset the alert flag
        *alert_triggered.lock().expect("Should not fail in test") = false;

        // Send a report
        bus.tx.send(SystemEvent::Signal {
            source: "Test".to_string(),
            event_type: "StatusReport".to_string(),
            payload: "I am alive!".to_string(),
        })?;

        // Wait a bit for the Sentinel to process the signal
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Now wait less than timeout from the report time
        tokio::time::sleep(Duration::from_millis(200)).await;

        assert!(
            !*alert_triggered.lock().expect("Should not fail in test"),
            "The Sentinel sounded the alarm despite a recent check-in!"
        );

        // 3. Verify it triggers again after another silence
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(*alert_triggered.lock().expect("Should not fail in test"), "The Sentinel failed to trigger a second time!");

        Ok(())
    }
}
