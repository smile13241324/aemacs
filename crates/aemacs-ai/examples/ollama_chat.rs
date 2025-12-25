use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::{AIBackend, AIRequest, Message, Role};
use futures::StreamExt;
use std::error::Error;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Optional: Enable logs to see what happens under the hood
    // tracing_subscriber::fmt::init();

    println!("🦙 Connecting to Local Ollama Instance...");

    // 1. Initialize the Backend
    // Ollama typically runs on port 11434.
    // Important: We don't need an API Key (None) for localhost.
    // The '/v1' suffix is crucial for OpenAI compatibility mode!
    let backend = OpenAICompatibleBackend::new("http://localhost:11434/v1", None);

    // Health Check: Is Ollama alive?
    match backend.health_check().await {
        Ok(_) => println!("✅ Ollama is online and healthy!"),
        Err(e) => {
            eprintln!("❌ Could not connect to Ollama. Is 'ollama serve' running?");
            eprintln!("   Error: {}", e);
            return Ok(());
        }
    }

    // 2. Build the Request
    let request = AIRequest {
        // Ensure you ran 'ollama pull mistral' before!
        model: "mistral".to_string(),
        messages: vec![
            Message::new(
                Role::System,
                "You are Æmacs, a helpful coding assistant embedded in a Rust IDE. Be concise.",
            ),
            Message::new(
                Role::User,
                "Hello! Can you explain what a Monad and a Monoid is in detail?",
            ),
        ],
        temperature: 0.7,
        stream: true,
    };

    println!("\n🚀 Sending Prompt to Mistral...\n");
    println!("--- MISTRAL SAYS ---");

    // 3. Streaming (The Monad Pipeline in action!)
    let mut stream = backend.stream(request).await?;

    // We pull tokens from the stream as they arrive (lazy evaluation)
    while let Some(result) = stream.next().await {
        match result {
            Ok(token) => {
                print!("{}", token);
                io::stdout().flush()?; // Flush immediately to see the typing effect
            }
            Err(e) => {
                eprintln!("\n❌ Stream Error: {}", e);
                break;
            }
        }
    }

    println!("\n--------------------");
    println!("✨ Done.");

    Ok(())
}
