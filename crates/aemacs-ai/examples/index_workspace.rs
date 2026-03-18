use aemacs_ai::rag::KnowledgeBase;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::Path;

// Nomic embedding dimension
const EMBEDDING_DIM: u64 = 768;
const COLLECTION_NAME: &str = "aemacs_codebase";
// Simple chunk size (characters)
const CHUNK_SIZE: usize = 2000;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    println!("=== Æmacs Grand Archivist (ACO-024) ===");

    // Use current directory as workspace root
    let workspace_root = env::current_dir().context("Failed to get current directory")?;
    println!("Indexing workspace: {:?}", workspace_root);

    let qdrant_url = "http://localhost:6334";
    let ollama_url = "http://localhost:11434/v1";

    println!("Initializing KnowledgeBase...");
    let kb = KnowledgeBase::new(qdrant_url, ollama_url, aemacs_ai::rag::Environment::Production)?;

    let mut files_indexed = 0;
    let mut chunks_indexed = 0;

    // Hardcoded list of directories to ignore
    let ignore_dirs = vec![".git", "target", "node_modules", ".gemini"];
    // Allowed extensions
    let allowed_exts = vec!["rs", "md", "toml", "sh"];

    let mut paths_to_visit = vec![workspace_root.clone()];

    while let Some(dir) = paths_to_visit.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("Warning: Failed to read dir {:?}: {}", dir, e);
                continue;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();

            if path.is_dir() {
                if !ignore_dirs.iter().any(|&d| d == file_name) {
                    paths_to_visit.push(path);
                }
            } else if path.is_file() {
                let ext = path.extension().unwrap_or_default().to_string_lossy();
                if allowed_exts.iter().any(|&e| e == ext) {
                    println!(
                        "Indexing: {:?}",
                        path.strip_prefix(&workspace_root).unwrap_or(&path)
                    );
                    match index_file(&kb, &path, &workspace_root).await {
                        Ok(chunks) => {
                            files_indexed += 1;
                            chunks_indexed += chunks;
                        }
                        Err(e) => eprintln!("Failed to index {:?}: {}", path, e),
                    }
                }
            }
        }
    }

    println!("=== Archiving Complete ===");
    println!(
        "Indexed {} files into {} chunks.",
        files_indexed, chunks_indexed
    );

    Ok(())
}

async fn index_file(kb: &KnowledgeBase, file_path: &Path, root: &Path) -> Result<usize> {
    let content = fs::read_to_string(file_path)?;
    if content.trim().is_empty() {
        return Ok(0);
    }

    let rel_path = file_path
        .strip_prefix(root)
        .unwrap_or(file_path)
        .to_string_lossy()
        .to_string();
    let chunks = chunk_text(&content, CHUNK_SIZE);
    let chunk_count = chunks.len();

    for (i, chunk) in chunks.into_iter().enumerate() {
        let mut metadata = HashMap::new();
        metadata.insert("file_path".to_string(), rel_path.clone());
        metadata.insert("chunk_index".to_string(), i.to_string());
        metadata.insert("total_chunks".to_string(), chunk_count.to_string());

        // Prefix the chunk with context so the embedder knows where it came from
        let contextualized_chunk = format!(
            "File: {} [Chunk {}/{}]

{}",
            rel_path, i, chunk_count, chunk
        );

        kb.store_archive("indexer", "System", "workspace_index", i, &contextualized_chunk).await?;
    }

    Ok(chunk_count)
}

fn chunk_text(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current_chunk = String::new();

    // A very primitive chunking strategy by lines
    for line in text.lines() {
        if current_chunk.len() + line.len() > max_chars && !current_chunk.is_empty() {
            chunks.push(current_chunk.clone());
            current_chunk.clear();
        }
        current_chunk.push_str(line);
        current_chunk.push('\n');
    }

    if !current_chunk.trim().is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}
