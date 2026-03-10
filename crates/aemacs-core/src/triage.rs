use crate::bus::{EventBus, SystemEvent};
use crate::signals::{AutonomousIntent, SignalContext};
use crate::syntax::SupportedLanguage;
use anyhow::Result;
use log::{info, warn};
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

/// ACO-029: The Triage Router pre-processes signals and filters noise.
pub struct TriageRouter {
    bus: EventBus,
    debounce_duration: Duration,
    authorized_sources: Option<Vec<String>>,
}

impl TriageRouter {
    pub fn new(bus: EventBus, debounce_duration: Duration) -> Self {
        Self {
            bus,
            debounce_duration,
            authorized_sources: None,
        }
    }

    /// Configures the authorized sources for the triage engine.
    pub fn with_authorized_sources(mut self, sources: Vec<String>) -> Self {
        self.authorized_sources = Some(sources);
        self
    }

    /// Starts the triage engine, which listens to the bus and emits enriched intents.
    pub async fn run(&self) -> Result<()> {
        info!(
            "🧠 [TRIAGE] Neural Bridge active. Debounce: {:?}",
            self.debounce_duration
        );

        let mut rx = self.bus.subscribe();
        let (signal_tx, mut signal_rx) = mpsc::channel::<SystemEvent>(100);

        // 1. ACO-029-04: The Dampening Field (Debouncer)
        let bus_out = self.bus.tx.clone();
        let debounce_dur = self.debounce_duration;

        // Clone fields for the background task
        let auth_sources = self.authorized_sources.clone();

        tokio::spawn(async move {
            let mut pending_signals: Vec<SystemEvent> = Vec::new();
            let mut last_signal_time = Instant::now();

            loop {
                tokio::select! {
                    event = signal_rx.recv() => {
                        if let Some(event) = event {
                            pending_signals.push(event);
                            last_signal_time = Instant::now();
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_millis(50)) => {
                        if !pending_signals.is_empty() && last_signal_time.elapsed() >= debounce_dur {
                            // Process and emit
                            if pending_signals.len() > 5 {
                                // ACO-029-04: Bulk Update mapping
                                let bulk_context = SignalContext {
                                    intent: AutonomousIntent::BulkUpdate,
                                    file_path: None,
                                    snippet: Some(format!("Detected {} rapid changes.", pending_signals.len())),
                                    metadata: HashMap::new(),
                                };
                                let payload = serde_json::to_string(&bulk_context).unwrap_or_default();
                                let _ = bus_out.send(SystemEvent::Signal {
                                    source: "TriageRouter".to_string(),
                                    event_type: "AutonomousIntent".to_string(),
                                    payload,
                                });
                            } else {
                                // Process all pending signals individually
                                for event in pending_signals.drain(..) {
                                    if let Some(ctx) = Self::triage_event_internal(&auth_sources, event) {
                                        let payload = serde_json::to_string(&ctx).unwrap_or_default();
                                        let _ = bus_out.send(SystemEvent::Signal {
                                            source: "TriageRouter".to_string(),
                                            event_type: "AutonomousIntent".to_string(),
                                            payload,
                                        });
                                    }
                                }
                            }
                            pending_signals.clear();
                        }
                    }
                }
            }
        });

        // Main Listener
        while let Ok(event) = rx.recv().await {
            match &event {
                SystemEvent::Signal {
                    source, event_type, ..
                } => {
                    // Avoid recursive loops from our own emitted intents
                    if source == "TriageRouter" || event_type == "AutonomousIntent" {
                        continue;
                    }
                    // Pass to debouncer
                    let _ = signal_tx.send(event.clone()).await;
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// ACO-029-02: Maps raw events to high-level intents.
    pub fn triage_event(&self, event: SystemEvent) -> Option<SignalContext> {
        Self::triage_event_internal(&self.authorized_sources, event)
    }

    fn triage_event_internal(
        authorized_sources: &Option<Vec<String>>,
        event: SystemEvent,
    ) -> Option<SignalContext> {
        match event {
            SystemEvent::Signal {
                source,
                event_type,
                payload,
            } => {
                // ACO-030: Verify Authorization
                if let Some(authorized) = authorized_sources {
                    if !authorized.iter().any(|s| source.contains(s)) {
                        warn!(
                            "🧠 [TRIAGE] Dropping signal from unauthorized source: {}",
                            source
                        );
                        return None;
                    }
                }

                match event_type.as_str() {
                    "FileSaved" => {
                        // ACO-029-03: Contextual Flooding (Payload now contains snippet)
                        let parts: Vec<&str> = payload.split("@@").collect();
                        let path_str = parts.get(0).unwrap_or(&"").to_string();
                        let snippet = parts.get(1).map(|s| s.to_string());

                        let mut metadata = HashMap::new();
                        if let Ok(lang) = SupportedLanguage::from_path(Path::new(&path_str)) {
                            metadata.insert("language".to_string(), format!("{:?}", lang));
                        }

                        Some(SignalContext {
                            intent: AutonomousIntent::ReviewChange,
                            file_path: Some(path_str),
                            snippet,
                            metadata,
                        })
                    }
                    "TimePulse" => Some(SignalContext {
                        intent: AutonomousIntent::RoutineCheck,
                        file_path: None,
                        snippet: None,
                        metadata: HashMap::new(),
                    }),
                    "ExternalIntent" => {
                        // ACO-029: Webhook bridge mapping
                        if payload.contains("CI_FAILURE") {
                            Some(SignalContext {
                                intent: AutonomousIntent::FixBuildError,
                                file_path: None,
                                snippet: Some(payload),
                                metadata: HashMap::new(),
                            })
                        } else {
                            Some(SignalContext {
                                intent: AutonomousIntent::SummarizeExternalData,
                                file_path: None,
                                snippet: Some(payload),
                                metadata: HashMap::new(),
                            })
                        }
                    }
                    "EmailReceived" | "ChatMessage" => {
                        // ACO-030: Semantic Filter for Communication Mesh
                        let lower_payload = payload.to_lowercase();
                        let intent = if lower_payload.contains("deploy")
                            || lower_payload.contains("review")
                        {
                            AutonomousIntent::ReviewChange
                        } else if lower_payload.contains("fix") || lower_payload.contains("error") {
                            AutonomousIntent::FixBuildError
                        } else if lower_payload.contains("status")
                            || lower_payload.contains("inventory")
                        {
                            AutonomousIntent::RoutineCheck
                        } else {
                            AutonomousIntent::SummarizeExternalData
                        };

                        Some(SignalContext {
                            intent,
                            file_path: None,
                            snippet: Some(payload),
                            metadata: HashMap::new(),
                        })
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::EventBus;

    #[tokio::test]
    async fn test_dampening_field_quest() -> Result<()> {
        let bus = EventBus::new();
        let router = TriageRouter::new(bus.clone(), Duration::from_millis(200));

        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            let _ = router.run().await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        for i in 0..10 {
            bus.tx.send(SystemEvent::Signal {
                source: "Test".to_string(),
                event_type: "FileSaved".to_string(),
                payload: format!("file_{}.rs", i),
            })?;
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        tokio::time::sleep(Duration::from_millis(500)).await;

        let mut found_bulk = false;
        while let Ok(event) = rx.try_recv() {
            if let SystemEvent::Signal {
                event_type,
                payload,
                ..
            } = event
            {
                if event_type == "AutonomousIntent" {
                    let ctx: SignalContext = serde_json::from_str(&payload)?;
                    if ctx.intent == AutonomousIntent::BulkUpdate {
                        found_bulk = true;
                    }
                }
            }
        }

        assert!(
            found_bulk,
            "Dampening field failed to merge signals into BulkUpdate!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_intent_mapping_quest() -> Result<()> {
        let bus = EventBus::new();
        let router = TriageRouter::new(bus.clone(), Duration::from_millis(100));

        tokio::spawn(async move {
            let _ = router.run().await;
        });

        let mut rx = bus.subscribe();
        tokio::time::sleep(Duration::from_millis(50)).await;

        bus.tx.send(SystemEvent::Signal {
            source: "Bridge:GitHub".to_string(),
            event_type: "ExternalIntent".to_string(),
            payload: "{\"status\": \"CI_FAILURE\", \"build\": 123}".to_string(),
        })?;

        let mut found_fix = false;
        for _ in 0..10 {
            if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
                if let SystemEvent::Signal {
                    event_type,
                    payload,
                    ..
                } = event
                {
                    if event_type == "AutonomousIntent" {
                        let ctx: SignalContext = serde_json::from_str(&payload)?;
                        if ctx.intent == AutonomousIntent::FixBuildError {
                            found_fix = true;
                            break;
                        }
                    }
                }
            }
        }

        assert!(
            found_fix,
            "Triage Router failed to map CI_FAILURE to FixBuildError intent!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_email_triage_quest() -> Result<()> {
        let bus = EventBus::new();
        let router = TriageRouter::new(bus.clone(), Duration::from_millis(50));

        tokio::spawn(async move {
            let _ = router.run().await;
        });

        let mut rx = bus.subscribe();
        tokio::time::sleep(Duration::from_millis(50)).await;

        // Quest: Verify email triage with content attachment
        let email_body = "Please review the new logic in kernel.rs";
        bus.tx.send(SystemEvent::Signal {
            source: "Email:maxi@aemacs.ai".to_string(),
            event_type: "EmailReceived".to_string(),
            payload: email_body.to_string(),
        })?;

        let mut found_intent = false;
        for _ in 0..10 {
            if let Ok(Ok(event)) = tokio::time::timeout(Duration::from_secs(1), rx.recv()).await {
                if let SystemEvent::Signal {
                    event_type,
                    payload,
                    ..
                } = event
                {
                    if event_type == "AutonomousIntent" {
                        let ctx: SignalContext = serde_json::from_str(&payload)?;
                        assert_eq!(ctx.intent, AutonomousIntent::ReviewChange);
                        assert!(ctx.snippet.as_ref().unwrap().contains(email_body));
                        found_intent = true;
                        break;
                    }
                }
            }
        }

        assert!(
            found_intent,
            "EmailReceived signal failed to trigger ReviewChange intent!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_authorized_voice_quest() -> Result<()> {
        let bus = EventBus::new();
        // Quest: Configure router with an allow-list
        let router = TriageRouter::new(bus.clone(), Duration::from_millis(50))
            .with_authorized_sources(vec!["Authorized".to_string(), "maxi@aemacs.ai".to_string()]);

        let mut rx = bus.subscribe();

        tokio::spawn(async move {
            let _ = router.run().await;
        });

        // Give router plenty of time to start its listener
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 1. Send signal from unauthorized source
        bus.tx.send(SystemEvent::Signal {
            source: "Email:hacker@void.com".to_string(),
            event_type: "EmailReceived".to_string(),
            payload: "rm -rf /".to_string(),
        })?;

        // 2. Send signal from authorized source
        bus.tx.send(SystemEvent::Signal {
            source: "Email:maxi@aemacs.ai".to_string(),
            event_type: "EmailReceived".to_string(),
            payload: "status report".to_string(),
        })?;

        let mut intent_count = 0;
        let mut found_authorized = false;

        // Wait for router processing (debounce 50ms + slack)
        tokio::time::sleep(Duration::from_millis(500)).await;

        while let Ok(event) = rx.try_recv() {
            if let SystemEvent::Signal {
                event_type,
                payload,
                ..
            } = event
            {
                if event_type == "AutonomousIntent" {
                    let ctx: SignalContext = serde_json::from_str(&payload)?;
                    intent_count += 1;
                    if ctx.intent == AutonomousIntent::RoutineCheck {
                        found_authorized = true;
                    }
                }
            }
        }

        assert_eq!(
            intent_count, 1,
            "Triage Router failed to drop the unauthorized signal or missed the authorized one!"
        );
        assert!(
            found_authorized,
            "Authorized signal was incorrectly dropped!"
        );
        Ok(())
    }
}
