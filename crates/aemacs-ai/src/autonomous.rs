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
}

/// Pure function to format mnemic directives into a system context block.
fn format_mnemic_reflection(directives: &[String]) -> String {
    if directives.is_empty() {
        return "No core directives found in memory. Operate based on default persona.".to_string();
    }

    let mut context = String::from("<system_memory_context>\nPrior Insights & Directives:\n");
    for directive in directives {
        context.push_str(&format!("- {}\n", directive));
    }
    context.push_str("</system_memory_context>");
    context
}

/// Helper function to extract recent core directives and format them as context.
async fn perform_mnemic_reflection(kb: &KnowledgeBase) -> String {
    match kb.get_core_directives("aemacs_memory").await {
        Ok(directives) => format_mnemic_reflection(&directives),
        Err(e) => {
            warn!("⚠️ Failed to fetch core directives: {}", e);
            "Failed to access long-term memory. Relying on default programming.".to_string()
        }
    }
}

/// Coordinates the life-cycle of an autonomous agent in headless mode.
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

        // 2. Ignite Sensory Substrate
        let heartbeat = TimePulseObserver { interval_seconds };
        let bus_clone = self.bus.clone();
        tokio::spawn(async move {
            if let Err(e) = heartbeat.run(bus_clone).await {
                warn!("⏰ Heartbeat failed: {}", e);
            }
        });

        // 3. The Sovereign Loop
        let rx = self.bus.rx.clone();
        let mut conversation = Conversation::new(self.backend.name());

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
        };

        info!("🛡️ Sentinel Active. Waiting for signals...");

        while let Ok(event) = rx.recv().await {
            match event {
                SystemEvent::Signal {
                    source,
                    event_type,
                    payload,
                } => {
                    let trigger_message = if event_type == "TimePulse" {
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
                            None,
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

    #[test]
    fn test_format_mnemic_reflection_populated_quest() {
        let directives = vec![
            "Rust is Law".to_string(),
            "Never panic".to_string(),
        ];
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
}

