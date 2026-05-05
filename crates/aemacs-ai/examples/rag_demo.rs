use aemacs_ai::rag::KnowledgeBase;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🏛️  Initializing RAG Demo...");

    // 1. Connect to Infrastructure
    let qdrant_url = "http://localhost:6334";
    let ollama_url = "http://localhost:11434/v1";

    let kb = match KnowledgeBase::new(qdrant_url, ollama_url, aemacs_ai::rag::Environment::Test) {
        Ok(kb) => kb,
        Err(e) => {
            eprintln!("❌ Failed to initialize KnowledgeBase: {e}");
            eprintln!("   Ensure Qdrant is running on port 6334 and Ollama on 11434.");
            return Ok(());
        }
    };

    println!("✅ Connected to Qdrant & Ollama.");

    // 2. Setup Collection
    println!("⚙️  Ensuring collection exists...");

    // 3. Ingest Data
    println!("📚 Ingesting knowledge...");
    let docs = vec![
        "Æmacs is an AI-native editor written in Rust.",
        "The core philosophy of Æmacs is the Iron Core: performance and safety.",
        "Bob is the Architect agent who designs the system blueprints.",
        "Kairon is the Forge Master who implements the Rust core.",
    ];

    for (i, doc) in docs.into_iter().enumerate() {
        print!("   -> Indexing: '{doc}' ... ");
        kb.store_archive("demo_agent", "Assistant", "demo_session", i, doc)
            .await?;
        println!("Done.");
    }

    // 4. Search
    let query = "Who designs the blueprints?";
    println!("\n🔍 Searching for: '{query}'");

    // Search for a good answer with a similarity threshold of 0.7
    let results: Vec<aemacs_ai::rag::MemoryResult> = kb.search_archive(query, None, None).await?;

    println!("--- Results ---");
    for (i, result) in results.iter().enumerate() {
        println!("{}. [ID: {}] {}", i + 1, result.id, result.content);
    }
    println!("---------------");

    Ok(())
}
