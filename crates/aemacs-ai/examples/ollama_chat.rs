use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::{AIBackend, Conversation};
use base64::prelude::*;
use futures::StreamExt;
use std::error::Error;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Optional: Enable logs to see what happens under the hood
    // tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let image_path = if args.len() > 1 { Some(&args[1]) } else { None };

    println!("🦙 Connecting to Local Ollama Instance...");

    let backend = OpenAICompatibleBackend::new("http://localhost:11434/v1", None);

    match backend.health_check().await {
        Ok(_) => println!("✅ Ollama is online and healthy!"),
        Err(e) => {
            eprintln!("❌ Could not connect to Ollama. Is 'ollama serve' running?");
            eprintln!("   Error: {}", e);
            return Ok(());
        }
    }

    let request = if let Some(path) = image_path {
        println!("📸 Image detected: {}", path);
        println!("   Loading and encoding...");

        let image_data = std::fs::read(path)?;
        let b64 = BASE64_STANDARD.encode(image_data);
        let data_url = format!("data:image/jpeg;base64,{}", b64); // Assumption: jpeg/png works with generic header often

        println!("   Model: llama3.2-vision");
        Conversation::new("llama3.2-vision")
            .with_system("You are a vision assistant.")
            .with_user_with_image("Describe this image in detail.", data_url)
            .build()
    } else {
        println!("📝 Text mode.");
        println!("   Model: cognitivecomputations/dolphin-llama3.1");
        Conversation::new("cognitivecomputations/dolphin-llama3.1")
            .with_system(
                "You are Æmacs, a helpful coding assistant embedded in a Rust IDE. Be concise.",
            )
            .with_user("Hello! Can you explain what a Monad and a Monoid is in detail?")
            .build()
    };

    println!("\n🚀 Sending Request...\n");
    println!("--- AI SAYS ---");

    let mut stream = backend.stream(request).await?;

    while let Some(result) = stream.next().await {
        match result {
            Ok(token) => {
                print!("{}", token);
                io::stdout().flush()?;
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
