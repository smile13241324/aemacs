use crate::bus::{EventBus, SystemEvent};
use anyhow::Result;
use log::{info, warn};
use std::time::{Duration, Instant};

/// The Sentinel Daemon monitors autonomous agent sanity by tracking status reports.
/// If an agent fails to report functional integrity for over 1 hour, it triggers a rescue notification.
pub async fn spawn_sentinel(bus: EventBus) -> Result<()> {
    tokio::spawn(async move {
        info!("🛡️ Sentinel Daemon awakened. Monitoring agent sanity...");

        let mut last_report = Instant::now();
        let timeout = Duration::from_secs(3600); // 1 hour
        let check_interval = Duration::from_secs(60); // Check every minute

        let rx = bus.rx.clone();

        loop {
            // Non-blocking drain of the event bus to find status reports
            while let Ok(event) = rx.try_recv() {
                if let SystemEvent::Signal { event_type, .. } = event {
                    if event_type == "StatusReport" {
                        last_report = Instant::now();
                        info!("🛡️ Sentinel: Agent check-in received. Sanity confirmed.");
                    }
                }
            }

            // Check for silence
            if last_report.elapsed() > timeout {
                warn!("🚨 [SENTINEL ALERT] Agent silence exceeds 1 hour! Sanity compromised.");
                
                let alert = SystemEvent::Notification(
                    "Sentinel Alert: Agent Sanity Compromised. Automated Rescue Protocol required.".to_string()
                );
                
                if let Err(_) = bus.tx.send(alert).await {
                    warn!("EventBus disconnected. Sentinel shutting down.");
                    return;
                }

                // Reset timer to avoid spamming alerts
                last_report = Instant::now();
            }

            tokio::time::sleep(check_interval).await;
        }
    });

    Ok(())
}
