use aemacs_ai::connectors::local::LocalBackend;
use aemacs_ai::{AIBackend, AIRequest, Message, Role};
use std::error::Error;

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
    };

    println!("🚀 Sending Request...");
    let response = backend.complete(request).await?;

    println!("--- RESPONSE ---");
    println!("{}", response);
    println!("----------------");

    Ok(())
}
