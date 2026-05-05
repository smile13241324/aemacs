use aemacs_ai::connectors::local::LocalBackend;
use aemacs_ai::{AIBackend, AIRequest, Message, Role};
use futures::StreamExt;
use std::error::Error;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    println!("🔌 Testing LocalBackend STREAMING...");

    // Lets use "echo" again, but maybe later simulate something longer.
    let backend = LocalBackend::new("echo");

    let request = AIRequest {
        model: "test-model".to_string(),
        messages: vec![Message::new(Role::User, "Hallo Æmacs! I am streaming now.")],
        temperature: 0.7,
        stream: true,
        options: None,
        tools: None,
    };

    println!("🚀 Sending Request & Opening Stream...");

    // Here we get the stream (Pin<Box<dyn Stream...>>)
    let mut stream = backend.stream(request).await?;

    println!("--- STREAM START ---");

    // We read the tokens live from the stream as they arrive
    while let Some(result) = stream.next().await {
        match result {
            Ok(event) => {
                if let aemacs_ai::StreamEvent::Content(content) = event {
                    print!("{content}");
                    io::stdout().flush()?;
                }
            }
            Err(e) => {
                eprintln!("\n❌ Stream Error: {e}");
                break;
            }
        }
    }

    println!("--- STREAM END ---");
    Ok(())
}
