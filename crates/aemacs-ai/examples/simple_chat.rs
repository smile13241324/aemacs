use std::error::Error;

use aemacs_ai::{AIBackend, AIRequest, Message, Role, connectors::local::LocalBackend};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt::init();

    println!("🔌 Testing LocalBackend Plumbing...");

    let backend = LocalBackend::new("echo");

    println!("Checking Health...");
    backend.health_check().await?;
    println!("✅ Backend is ready!");

    let request = AIRequest {
        model: "test-model".to_string(),
        messages: vec![Message::new(Role::User, "Hallo Æmacs! Thats a test.")],
        temperature: 0.7,
        stream: false,
        options: None,
        tools: None,
    };

    println!("🚀 Sending Request...");
    let response = backend.complete(request).await?;

    println!("--- RESPONSE ---");
    println!("{response:?}");
    println!("----------------");

    Ok(())
}
