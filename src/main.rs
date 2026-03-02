use aemacs_ai::PersonaRegistry;
use aemacs_ai::autonomous::AutonomousService;
use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::mcp::ToolRegistry;
use aemacs_ai::rag::KnowledgeBase;
use aemacs_core::bus::EventBus;
use anyhow::Result;
use log::info;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize the logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🚀 [APP] Æmacs Boot Sequence initiated.");

    let args: Vec<String> = std::env::args().collect();

    // --- Headless Sovereign Detection (ACO-011) ---
    if args.iter().any(|arg| arg == "--server") {
        info!("🌑 [SERVER] Headless Sovereign Mode detected.");

        let agent_name = args
            .iter()
            .position(|arg| arg == "--agent")
            .and_then(|i| args.get(i + 1))
            .cloned()
            .unwrap_or_else(|| "bob".to_string());

        let interval: u64 = args
            .iter()
            .position(|arg| arg == "--interval")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(3600);

        let bridge_port: u16 = args
            .iter()
            .position(|arg| arg == "--bridge")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        // A. Core System Init
        aemacs_core::init()?;
        let bus = EventBus::new();

        // B. AI Infrastructure
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
        )?);
        let tokio_handle = tokio::runtime::Handle::current();
        let persona_registry = PersonaRegistry::new(tokio_handle).await?;
        let registry = Arc::new(ToolRegistry::with_core_tools(
            kb.clone(),
            persona_registry.clone(),
            Some(bus.tx.clone()),
        ));
        let backend = Arc::new(OpenAICompatibleBackend::new(
            "http://localhost:11434/v1",
            None,
        ));

        // C. Optional External Bridge (ACO-007-03)
        if bridge_port > 0 {
            aemacs_bridge::spawn_intent_bridge(bus.clone(), bridge_port).await?;
        }

        // D. Launch Sovereign Orchestrator
        let service = AutonomousService::new(bus, agent_name, persona_registry, registry, backend);
        service.start(interval).await?;

        return Ok(());
    }

    // --- GUI Mode (Default) ---
    let file_to_open = if args.len() > 1 {
        let path = std::path::PathBuf::from(&args[1]);
        info!("📂 [CLI] Requesting to open file: {:?}", path);
        Some(path)
    } else {
        None
    };

    aemacs_gpui::run_app(file_to_open);

    Ok(())
}
