use aemacs_ai::rag::KnowledgeBase;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    println!("🏛️  Initializing RAG Demo...");

    // 1. Connect to Infrastructure
    // Assumption: Qdrant at localhost:6333, Ollama at localhost:11434
    let qdrant_url = "http://localhost:6333";
    let ollama_url = "http://localhost:11434";

    let kb = match KnowledgeBase::new(qdrant_url, ollama_url) {
        Ok(kb) => kb,
        Err(e) => {
            eprintln!("❌ Failed to initialize KnowledgeBase: {}", e);
            eprintln!("   Ensure Qdrant is running on port 6333 and Ollama on 11434.");
            return Ok(());
        }
    };

    println!("✅ Connected to Qdrant & Ollama.");

    // 2. Setup Collection
    // 'nomic-embed-text' output dimension is 768.
    let collection_name = "aemacs_docs";
    println!("⚙️  Ensuring collection '{}' exists...", collection_name);
    kb.ensure_collection(collection_name, 768).await?;

    // 3. Ingest Data
    println!("📚 Ingesting knowledge...");
    let docs = vec![
        "Æmacs is an AI-native editor written in Rust.",
        "The core philosophy of Æmacs is the Iron Core: performance and safety.",
        "Bob is the Architect agent who designs the system blueprints.",
        "Kairon is the Forge Master who implements the Rust core.",
    ];

    for doc in docs {
        print!("   -> Indexing: '{}' ... ", doc);
        kb.add_document(collection_name, doc, None).await?;
        println!("Done.");
    }

    // 4. Search
    let query = "Who designs the blueprints?";
    println!(
        "
🔍 Searching for: '{}'",
        query
    );

    let results = kb.search(collection_name, query, 3).await?;

    println!("--- Results ---");
    for (i, result) in results.iter().enumerate() {
        println!("{}. {}", i + 1, result);
    }
    println!("---------------");

    Ok(())
}
