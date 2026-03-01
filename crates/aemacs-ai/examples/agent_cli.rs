use aemacs_ai::PersonaRegistry;
use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::conversation::Conversation;
use aemacs_ai::mcp::{ToolHost, ToolRegistry, run_agent_loop};
use aemacs_ai::rag::KnowledgeBase;
use async_trait::async_trait;
use std::io::{self, Write};
use std::sync::Arc;

// --- Colors ---
const RESET: &str = "\x1b[0m";
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BLUE: &str = "\x1b[34m";
const CYAN: &str = "\x1b[36m";
const BOLD: &str = "\x1b[1m";

fn color(text: &str, color_code: &str) -> String {
    format!("{}{}{}", color_code, text, RESET)
}

// --- Console Host Implementation ---

pub struct ConsoleHost;

#[async_trait]
impl ToolHost for ConsoleHost {
    async fn ask_approval(&self, description: &str) -> bool {
        println!("\n{}", color("⚠️  APPROVAL REQUEST ⚠️", YELLOW));
        println!("{}", description);
        print!("{} (y/n): ", color("Do you approve this action?", BOLD));
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim().to_lowercase();

        input == "y" || input == "yes"
    }

    async fn ask_user(&self, question: &str) -> String {
        println!("\n{}", color("❓ QUESTION FROM AGENT ❓", CYAN));
        println!("{}", question);
        print!("{}> ", color("", CYAN));
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        input.trim().to_string()
    }

    fn get_agent_id(&self) -> String {
        "cli-agent".to_string()
    }

    fn report_progress(&self, tool_name: String, is_running: bool) {
        if is_running {
            println!("   {} Executing {}...", color("⚒", CYAN), tool_name);
        }
    }
}

// --- Main ---

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("{}", color("🤖 Æmacs Agent CLI (MCP Proving Ground)", BOLD));
    println!("---------------------------------------");

    let ollama_url = "http://localhost:11434/v1";
    let qdrant_url = "http://localhost:6334";

    println!("🔌 Connecting to Ollama at {}", color(ollama_url, BLUE));
    let backend = OpenAICompatibleBackend::new(ollama_url, None);

    println!(
        "📚 Connecting to KnowledgeBase at {}",
        color(qdrant_url, BLUE)
    );
    let kb = match KnowledgeBase::new(qdrant_url, ollama_url) {
        Ok(kb) => Arc::new(kb),
        Err(e) => {
            eprintln!(
                "{} Qdrant not available ({}). Search tool will fail.",
                color("⚠️  Warning:", YELLOW),
                e
            );
            return Err(anyhow::anyhow!("Qdrant not available: {}", e));
        }
    };

    println!("🛠️  Registering Core Tools...");
    let persona_registry = PersonaRegistry::new(tokio::runtime::Handle::current())
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load persona registry: {}", e))?;
    let registry = ToolRegistry::with_core_tools(kb, persona_registry, None);

    // List tools
    let defs = registry.list_definitions();
    print!("   Loaded {} tools: ", defs.len());
    for def in defs {
        // Extract name safely
        if let Some(name) = def["function"]["name"].as_str() {
            print!("{} ", color(name, CYAN));
        }
    }
    println!();

    let host = ConsoleHost;

    let mut history = Conversation::new("hermes3:8b-llama3.1-q4_K_M").with_system(
        "You are an autonomous coding agent. Use tools to inspect and modify the codebase.",
    );

    println!("\n{}", color("✅ Ready. Type 'exit' to quit.", GREEN));

    loop {
        print!("\n{}: ", color("You", GREEN));
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input == "exit" || input == "quit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        history = history.with_user(input);

        println!("{}", color("... Agent is thinking ...", "\x1b[90m")); // Gray

        let (stream_tx, stream_rx) = async_channel::unbounded();

        // Spawn a task to print the stream
        let print_task = tokio::spawn(async move {
            print!("{}: ", color("Agent", BLUE));
            io::stdout().flush().unwrap();
            while let Ok(chunk) = stream_rx.recv().await {
                print!("{}", chunk);
                io::stdout().flush().unwrap();
            }
            println!();
        });

        match run_agent_loop(&backend, &registry, &host, &mut history, 5, Some(stream_tx)).await {
            Ok(_) => {
                let _ = print_task.await;
            }
            Err(e) => {
                eprintln!("\n{}: {}\n", color("❌ Error", RED), e);
            }
        }
    }

    Ok(())
}
