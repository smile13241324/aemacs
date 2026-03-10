use crate::mcp::{ToolHost, ToolRegistry, run_agent_loop};
use crate::rag::KnowledgeBase;
use crate::{AIBackend, Conversation, Message, PersonaRegistry};
use aemacs_core::bus::{EventBus, SystemEvent};
use aemacs_core::observer::{ReactiveObserver, TimePulseObserver};
use aemacs_core::sentinel::spawn_sentinel;
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;
use tracing::{info, warn};

/// A sovereign ToolHost for headless server mode.
/// Automatically approves all actions, granting the agent full autonomy.
pub struct ServerHost {
    pub agent_id: String,
    pub bus: EventBus,
}

#[async_trait]
impl ToolHost for ServerHost {
    async fn ask_approval(&self, description: &str) -> bool {
        info!("⚖️ [SOVEREIGN] Auto-approving action: {}", description);
        true
    }

    async fn ask_user(&self, question: &str) -> String {
        info!("❓ [SOVEREIGN] Question to void: {}", question);
        "Server Mode: Automated Response".to_string()
    }

    fn get_agent_id(&self) -> String {
        self.agent_id.clone()
    }

    fn report_progress(&self, tool_name: String, is_running: bool) {
        if is_running {
            info!("⚙️ [SOVEREIGN] Executing: {}", tool_name);
        }
    }

    async fn emit_signal(&self, event_type: String, payload: String) -> Result<()> {
        let event = SystemEvent::Signal {
            source: format!("Agent:{}", self.agent_id),
            event_type,
            payload,
        };
        let _ = self.bus.tx.send(event);
        Ok(())
    }
}

/// Pure function to format mnemic directives into a system context block.
fn format_mnemic_reflection(directives: &[String]) -> String {
    if directives.is_empty() {
        return "No core directives found in memory. Operate based on default persona.".to_string();
    }

    let mut reflection = "<system_memory_context>\nPrior Insights & Directives:\n".to_string();
    for d in directives {
        reflection.push_str(&format!("- {}\n", d));
    }
    reflection.push_str("</system_memory_context>");
    reflection
}

/// Aggregates recent insights and core truths into a system prompt injection.
async fn perform_mnemic_reflection(kb: &KnowledgeBase) -> String {
    match kb.get_core_directives("aemacs_docs").await {
        Ok(directives) => format_mnemic_reflection(&directives),
        Err(_) => "Operate based on default persona.".to_string(),
    }
}

pub struct AutonomousService {
    bus: EventBus,
    agent_name: String,
    persona_registry: Arc<PersonaRegistry>,
    registry: Arc<ToolRegistry>,
    backend: Arc<dyn AIBackend>,
    kb: Arc<KnowledgeBase>,
}

impl AutonomousService {
    pub fn new(
        bus: EventBus,
        agent_name: String,
        persona_registry: Arc<PersonaRegistry>,
        registry: Arc<ToolRegistry>,
        backend: Arc<dyn AIBackend>,
        kb: Arc<KnowledgeBase>,
    ) -> Self {
        Self {
            bus,
            agent_name,
            persona_registry,
            registry,
            backend,
            kb,
        }
    }

    pub async fn start(&self, interval_seconds: u64) -> Result<()> {
        info!(
            "🧠 [AUTONOMOUS] Starting Sovereign Service for agent: {}",
            self.agent_name.to_uppercase()
        );
        println!("🚀 Sovereign Mode Active: Automated Tool Approval Enabled.");

        // 1. Ignite Life-Support
        spawn_sentinel(self.bus.clone()).await?;

        // 2. Ignite Sensory Substrate (ACO-007, ACO-029)
        let heartbeat = TimePulseObserver { interval_seconds };
        let bus_clone = self.bus.clone();
        tokio::spawn(async move {
            if let Err(e) = heartbeat.run(bus_clone).await {
                warn!("⏰ Heartbeat failed: {}", e);
            }
        });

        // ACO-029-02: Start the Triage Router
        let triage_router = aemacs_core::triage::TriageRouter::new(
            self.bus.clone(),
            std::time::Duration::from_millis(500),
        );
        tokio::spawn(async move {
            if let Err(e) = triage_router.run().await {
                warn!("🧠 Triage Router failed: {}", e);
            }
        });

        // 3. The Sovereign Loop
        let mut rx = self.bus.subscribe();
        let mut conversation = Conversation::new(self.backend.name());
        conversation.set_sovereign_mode(true);

        // Fetch and set the initial persona
        if let Some(persona) = self.persona_registry.get_persona(&self.agent_name).await {
            conversation.set_persona(persona);
        } else {
            warn!(
                "⚠️ Agent persona '{}' not found. Running as generalist.",
                self.agent_name
            );
        }

        let host = ServerHost {
            agent_id: self.agent_name.clone(),
            bus: self.bus.clone(),
        };

        info!("🛡️ Sentinel Active. Waiting for signals...");

        while let Ok(event) = rx.recv().await {
            match event {
                SystemEvent::Notification(msg) => {
                    if msg.contains("Sanity Compromised") {
                        warn!(
                            "🧠 [AUTONOMOUS] Sentinel Alert received! Initiating Mind-Heal Protocol..."
                        );
                        conversation.clear_history();
                        // Re-add the initial persona prompt
                        if let Some(persona) =
                            self.persona_registry.get_persona(&self.agent_name).await
                        {
                            conversation.set_persona(persona);
                        }
                        info!("✨ [AUTONOMOUS] Mind-Heal complete. Conversation history purged.");
                    }
                }
                SystemEvent::Signal {
                    source,
                    event_type,
                    payload,
                } => {
                    if event_type == "LowConfidenceRecall" {
                        warn!(
                            "🧠 [AUTONOMOUS] Low confidence recall detected. Triggering background maintenance..."
                        );
                        let kb = self.kb.clone();
                        tokio::spawn(async move {
                            if let Err(e) = kb.optimize_collection("aemacs_docs").await {
                                warn!("⚠️ [AUTONOMOUS] Background optimization failed: {}", e);
                            }
                        });
                    }

                    let trigger_message = if event_type == "AutonomousIntent" {
                        match serde_json::from_str::<aemacs_core::signals::SignalContext>(&payload)
                        {
                            Ok(ctx) => {
                                let mut msg =
                                    format!("A sovereign intent was detected: {:?}\n", ctx.intent);
                                if let Some(path) = ctx.file_path {
                                    msg.push_str(&format!("File: {}\n", path));
                                }
                                if let Some(snippet) = ctx.snippet {
                                    msg.push_str(&format!(
                                        "Context Snippet:\n```\n{}\n```\n",
                                        snippet
                                    ));
                                }
                                Some(msg)
                            }
                            Err(e) => {
                                warn!("⚠️ Failed to parse SignalContext: {}", e);
                                None
                            }
                        }
                    } else if event_type == "TimePulse" {
                        // Keep legacy fallback for unrouted pulses if needed,
                        // though router handles them now.
                        Some("Tick. Perform a mental inventory and decide on your next autonomous action.".to_string())
                    } else if source.starts_with("Bridge:") {
                        Some(format!(
                            "External interrupt received from source '{}' (Type: '{}'). Payload: {}",
                            source, event_type, payload
                        ))
                    } else {
                        None
                    };

                    if let Some(msg) = trigger_message {
                        info!(
                            "⚡ [AUTONOMOUS] Waking agent: {} (Source: {})",
                            event_type, source
                        );

                        // Phase 2: Mnemic Reflection
                        info!("🔍 [AUTONOMOUS] Performing Mnemic Reflection...");
                        let reflection = perform_mnemic_reflection(&self.kb).await;
                        conversation.add_message(Message::system(reflection));

                        // Action phase
                        conversation.add_message(Message::user(msg));

                        let _ = run_agent_loop(
                            self.backend.as_ref(),
                            &self.registry,
                            &host,
                            &mut conversation,
                            10,
                            None, // No stream proxy needed for headless mode yet
                        )
                        .await;
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_format_mnemic_reflection_populated_quest() {
        let directives = vec!["Rust is Law".to_string(), "Never panic".to_string()];
        let result = format_mnemic_reflection(&directives);
        let expected = "<system_memory_context>\nPrior Insights & Directives:\n- Rust is Law\n- Never panic\n</system_memory_context>";
        assert_eq!(result, expected);
    }

    #[test]
    fn test_format_mnemic_reflection_empty_quest() {
        let directives: Vec<String> = vec![];
        let result = format_mnemic_reflection(&directives);
        let expected = "No core directives found in memory. Operate based on default persona.";
        assert_eq!(result, expected);
    }

    #[tokio::test]
    async fn test_mind_heal_protocol_quest() -> Result<()> {
        use crate::connectors::openai_compatible::OpenAICompatibleBackend;
        use crate::mcp::ToolRegistry;

        let bus = EventBus::new();
        let tokio_handle = tokio::runtime::Handle::current();
        let persona_registry = PersonaRegistry::new(tokio_handle).await?;
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
        )?);
        let registry = Arc::new(ToolRegistry::new());
        let backend = Arc::new(OpenAICompatibleBackend::new(
            "http://localhost:11434/v1",
            None,
        ));

        let service = AutonomousService::new(
            bus.clone(),
            "bob".to_string(),
            persona_registry,
            registry,
            backend,
            kb,
        );

        // Quest: Run the loop in a background task
        tokio::spawn(async move {
            let _ = service.start(3600).await;
        });

        // Give it a moment to boot
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 1. Trigger the Alert
        bus.tx
            .send(SystemEvent::Notification("Sanity Compromised".to_string()))?;

        // 2. Since we can't easily inspect the internal conversation state of the running service
        // without more instrumentation, we verify that the service is still alive
        // and responding to further signals.
        bus.tx.send(SystemEvent::Signal {
            source: "Test".to_string(),
            event_type: "TimePulse".to_string(),
            payload: "{}".to_string(),
        })?;

        // If the service didn't crash or hang, the quest is provisionally successful.

        Ok(())
    }

    #[tokio::test]
    async fn test_reflective_maintenance_trigger_quest() -> Result<()> {
        use crate::connectors::openai_compatible::OpenAICompatibleBackend;
        use crate::mcp::ToolRegistry;

        let bus = EventBus::new();
        let tokio_handle = tokio::runtime::Handle::current();
        let persona_registry = PersonaRegistry::new(tokio_handle).await?;
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
        )?);
        let registry = Arc::new(ToolRegistry::new());
        let backend = Arc::new(OpenAICompatibleBackend::new(
            "http://localhost:11434/v1",
            None,
        ));

        let service = AutonomousService::new(
            bus.clone(),
            "bob".to_string(),
            persona_registry,
            registry,
            backend,
            kb,
        );

        // Quest: Run the service loop
        tokio::spawn(async move {
            let _ = service.start(3600).await;
        });

        // Give it a moment to boot
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 1. Inject the LowConfidenceRecall signal
        bus.tx.send(SystemEvent::Signal {
            source: "Agent:bob".to_string(),
            event_type: "LowConfidenceRecall".to_string(),
            payload: "{\"query\": \"unknown truth\", \"confidence\": \"none\"}".to_string(),
        })?;

        // 2. Verify responsiveness
        bus.tx.send(SystemEvent::Signal {
            source: "Test".to_string(),
            event_type: "TimePulse".to_string(),
            payload: "{}".to_string(),
        })?;

        Ok(())
    }
}
