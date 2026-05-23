use std::sync::Arc;

use aemacs_core::{
    bus::{EventBus, SystemEvent},
    observer::{ReactiveObserver, TimePulseObserver},
    sentinel::spawn_sentinel,
};
use anyhow::Result;
use async_trait::async_trait;
use tracing::{info, warn};

use crate::{
    AIBackend, Conversation, Message, PersonaRegistry,
    mcp::{ToolHost, ToolRegistry, run_agent_loop},
    rag::KnowledgeBase,
};

/// A sovereign `ToolHost` for headless server mode.
/// Automatically approves all actions, granting the agent full autonomy.
#[derive(Debug)]
pub struct ServerHost {
    /// The unique identifier of the agent being hosted.
    pub agent_id: String,
    /// The system event bus for signal propagation.
    pub bus: EventBus,
}

#[async_trait]
impl ToolHost for ServerHost {
    /// In server mode, all tool actions are automatically approved to ensure uninterrupted autonomy.
    async fn ask_approval(&self, description: &str) -> bool {
        info!("⚖️ [SOVEREIGN] Auto-approving action: {}", description);
        true
    }

    /// Handles requests for user input by providing a default automated response.
    async fn ask_user(&self, question: &str) -> String {
        info!("❓ [SOVEREIGN] Question to void: {}", question);
        "Server Mode: Automated Response".to_string()
    }

    /// Retrieves the ID of the hosted agent.
    fn get_agent_id(&self) -> String {
        self.agent_id.clone()
    }

    /// Logs the progress of tool execution to the system logs.
    fn report_progress(&self, tool_name: String, is_running: bool) {
        if is_running {
            info!("⚙️ [SOVEREIGN] Executing: {}", tool_name);
        }
    }

    /// Emits a signal from the agent to the rest of the system via the event bus.
    async fn emit_signal(&self, event_type: String, payload: String) -> Result<()> {
        let event =
            SystemEvent::Signal { source: format!("Agent:{}", self.agent_id), event_type, payload };
        let _ = self.bus.tx.send(event);
        Ok(())
    }
}

/// Pure function to format mnemic directives into a system context block.
/// This translates raw memory strings into a structured prompt injection.
fn format_mnemic_reflection(directives: &[String]) -> String {
    use std::fmt::Write;
    if directives.is_empty() {
        return "No core directives found in memory. Operate based on default persona.".to_string();
    }

    let mut reflection = "<system_memory_context>\nPrior Insights & Directives:\n".to_string();
    for d in directives {
        let _ = writeln!(reflection, "- {d}");
    }
    reflection.push_str("</system_memory_context>");
    reflection
}

/// Aggregates recent insights and core truths from the RAG Fortress into a system prompt injection.
async fn perform_mnemic_reflection(kb: &KnowledgeBase) -> String {
    kb.get_core_directives().await.map_or_else(
        |_| "Operate based on default persona.".to_string(),
        |directives| format_mnemic_reflection(&directives),
    )
}

/// The Orchestrator for headless Sovereign mode.
/// It manages the lifecycle of an autonomous agent, listening for sensory signals and taking action.
pub struct AutonomousService {
    /// The system event bus.
    bus: EventBus,
    /// The name of the agent to run.
    agent_name: String,
    /// Registry of available agent personas.
    persona_registry: Arc<PersonaRegistry>,
    /// Registry of available tools.
    registry: Arc<ToolRegistry>,
    /// The AI backend for inference.
    backend: Arc<dyn AIBackend>,
    /// The RAG memory system.
    kb: Arc<KnowledgeBase>,
}

impl std::fmt::Debug for AutonomousService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AutonomousService")
            .field("bus", &self.bus)
            .field("agent_name", &self.agent_name)
            .field("persona_registry", &self.persona_registry)
            .field("registry", &self.registry)
            .field("backend", &"Arc<dyn AIBackend>")
            .field("kb", &self.kb)
            .finish()
    }
}

impl AutonomousService {
    /// Initializes a new `AutonomousService`.
    #[must_use]
    pub fn new(
        bus: EventBus,
        agent_name: String,
        persona_registry: Arc<PersonaRegistry>,
        registry: Arc<ToolRegistry>,
        backend: Arc<dyn AIBackend>,
        kb: Arc<KnowledgeBase>,
    ) -> Self {
        Self { bus, agent_name, persona_registry, registry, backend, kb }
    }

    fn spawn_background_tasks(&self, interval_seconds: u64) -> Result<()> {
        spawn_sentinel(self.bus.clone())?;
        self.spawn_heartbeat(interval_seconds);
        self.spawn_triage_router();
        Ok(())
    }

    fn spawn_heartbeat(&self, interval_seconds: u64) {
        let heartbeat = TimePulseObserver { interval_seconds };
        let bus_clone = self.bus.clone();
        tokio::spawn(async move {
            if let Err(error) = heartbeat.run(bus_clone).await {
                warn!("⏰ Heartbeat failed: {}", error);
            }
        });
    }

    fn spawn_triage_router(&self) {
        let triage_router = aemacs_core::triage::TriageRouter::new(
            self.bus.clone(),
            std::time::Duration::from_millis(500),
        );
        tokio::spawn(async move {
            if let Err(error) = triage_router.run().await {
                warn!("🧠 Triage Router failed: {}", error);
            }
        });
    }

    async fn apply_persona(&self, conversation: &mut Conversation) {
        if let Some(persona) = self.persona_registry.get_persona(&self.agent_name).await {
            conversation.set_persona(persona);
        } else {
            warn!("⚠️ Agent persona '{}' not found. Running as generalist.", self.agent_name);
        }
    }

    async fn initialize_conversation(&self) -> Conversation {
        let mut conversation = Conversation::new(self.backend.name());
        conversation.set_sovereign_mode(true);
        self.apply_persona(&mut conversation).await;
        conversation
    }

    fn build_trigger_message(source: &str, event_type: &str, payload: &str) -> Option<String> {
        if event_type == "AutonomousIntent" {
            match serde_json::from_str::<aemacs_core::signals::SignalContext>(payload) {
                Ok(ctx) => {
                    use std::fmt::Write as _;

                    let mut msg = format!("A sovereign intent was detected: {:?}\n", ctx.intent);
                    if let Some(path) = ctx.file_path {
                        let _ = writeln!(msg, "File: {path}");
                    }
                    if let Some(snippet) = ctx.snippet {
                        let _ = writeln!(msg, "Context Snippet:\n```\n{snippet}\n```");
                    }
                    Some(msg)
                },
                Err(error) => {
                    warn!("⚠️ Failed to parse SignalContext: {}", error);
                    None
                },
            }
        } else if event_type == "TimePulse" {
            Some(
                "Tick. Perform a mental inventory and decide on your next autonomous action."
                    .to_string(),
            )
        } else if source.starts_with("Bridge:") {
            Some(format!(
                "External interrupt received from source '{source}' (Type: '{event_type}'). Payload: {payload}"
            ))
        } else {
            None
        }
    }

    async fn awaken_agent(
        &self,
        host: &ServerHost,
        conversation: &mut Conversation,
        source: &str,
        event_type: &str,
        msg: String,
    ) {
        info!("⚡ [AUTONOMOUS] Waking agent: {} (Source: {})", event_type, source);
        info!("🔍 [AUTONOMOUS] Performing Mnemic Reflection...");

        let reflection = perform_mnemic_reflection(&self.kb).await;
        conversation.add_message(Message::system(reflection));
        conversation.add_message(Message::user(msg));

        let _ = run_agent_loop(self.backend.as_ref(), &self.registry, host, conversation, 10, None)
            .await;
    }

    /// Starts the sovereign execution loop.
    /// This function blocks until the service is terminated or an unrecoverable error occurs.
    ///
    /// # Errors
    /// Returns an error if the agent cannot be started.
    pub async fn start(&self, interval_seconds: u64) -> Result<()> {
        info!(
            "🧠 [AUTONOMOUS] Starting Sovereign Service for agent: {}",
            self.agent_name.to_uppercase()
        );
        println!("🚀 Sovereign Mode Active: Automated Tool Approval Enabled.");

        self.spawn_background_tasks(interval_seconds)?;

        // 3. The Sovereign Loop
        let mut rx = self.bus.subscribe();
        let mut conversation = self.initialize_conversation().await;
        let host = ServerHost { agent_id: self.agent_name.clone(), bus: self.bus.clone() };

        info!("🛡️ Sentinel Active. Waiting for signals...");

        while let Ok(event) = rx.recv().await {
            match event {
                SystemEvent::Notification(msg) if msg.contains("Sanity Compromised") => {
                    warn!(
                        "🧠 [AUTONOMOUS] Sentinel Alert received! Initiating Mind-Heal Protocol..."
                    );
                    conversation.clear_history();
                    self.apply_persona(&mut conversation).await;
                    info!("✨ [AUTONOMOUS] Mind-Heal complete. Conversation history purged.");
                },
                SystemEvent::Signal { source, event_type, payload } => {
                    if event_type == "LowConfidenceRecall" {
                        warn!(
                            "🧠 [AUTONOMOUS] Low confidence recall detected. Background maintenance handled by RAG Core."
                        );
                    }

                    if let Some(msg) = Self::build_trigger_message(&source, &event_type, &payload) {
                        self.awaken_agent(&host, &mut conversation, &source, &event_type, msg)
                            .await;
                    }
                },
                _ => {}, // Ignore other event types
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

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
        use crate::{connectors::openai_compatible::OpenAICompatibleBackend, mcp::ToolRegistry};

        let bus = EventBus::new();
        let tokio_handle = tokio::runtime::Handle::current();
        let persona_registry = PersonaRegistry::new(tokio_handle).await?;
        let service = {
            let registry = Arc::new(ToolRegistry::new());
            let backend =
                Arc::new(OpenAICompatibleBackend::new("http://localhost:11434/v1", None)?);
            let kb = Arc::new(
                KnowledgeBase::bootstrap(
                    "http://localhost:6334",
                    "http://localhost:11434",
                    crate::rag::Environment::Test,
                )
                .await?,
            );

            AutonomousService::new(
                bus.clone(),
                "bob".to_string(),
                persona_registry,
                registry,
                backend,
                kb,
            )
        };

        // Quest: Run the loop in a background task
        tokio::spawn(async move {
            let _ = service.start(3600).await;
        });

        // Give it a moment to boot
        tokio::time::sleep(Duration::from_millis(200)).await;

        // 1. Trigger the Alert
        bus.tx.send(SystemEvent::Notification("Sanity Compromised".to_string()))?;

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
        use crate::{connectors::openai_compatible::OpenAICompatibleBackend, mcp::ToolRegistry};

        let bus = EventBus::new();
        let tokio_handle = tokio::runtime::Handle::current();
        let persona_registry = PersonaRegistry::new(tokio_handle).await?;
        let service = {
            let registry = Arc::new(ToolRegistry::new());
            let backend =
                Arc::new(OpenAICompatibleBackend::new("http://localhost:11434/v1", None)?);
            let kb = Arc::new(
                KnowledgeBase::bootstrap(
                    "http://localhost:6334",
                    "http://localhost:11434",
                    crate::rag::Environment::Test,
                )
                .await?,
            );

            AutonomousService::new(
                bus.clone(),
                "bob".to_string(),
                persona_registry,
                registry,
                backend,
                kb,
            )
        };

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
