use aemacs_ai::migration::{export_jsonl, extract_legacy_files, import_jsonl};
use aemacs_ai::rag::{Environment, KnowledgeBase};
use aemacs_core::config::get_config;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

/// The command-line interface structure for the Memory Migration tool.
/// It uses the `clap` crate to parse user input and route to the correct subcommands.
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

/// Defines the available subcommands for the Memory Migration tool.
/// Each variant represents a distinct phase of the migration lifecycle.
#[derive(Subcommand)]
enum Commands {
    /// Extracts legacy text logs into the intermediate JSONL format.
    Extract {
        /// The ID of the agent this history belongs to (e.g., 'kairon').
        #[arg(long)]
        agent_id: String,

        /// The starting timestamp in ISO8601 format (e.g., '2026-01-01T12:00:00Z').
        #[arg(long)]
        start_time: String,

        /// The number of seconds to add (or subtract) for each sequential record.
        #[arg(long, default_value_t = -3600)]
        time_step_sec: i64,

        /// The output JSONL file path.
        #[arg(long, default_value = "output.jsonl")]
        output: PathBuf,

        /// The legacy text files to process, ordered sequentially.
        #[arg(required = true)]
        files: Vec<String>,
    },
    /// Imports a JSONL file into the RAG Fortress.
    Import {
        /// The JSONL file to import.
        #[arg(required = true)]
        file: PathBuf,

        /// Whether to use the Production environment instead of Test.
        #[arg(long)]
        production: bool,
    },
    /// Exports all records for a specific agent from the RAG Fortress to a JSONL file.
    Export {
        /// The ID of the agent to export.
        #[arg(long)]
        agent_id: String,

        /// The output JSONL file path.
        #[arg(long, default_value = "export.jsonl")]
        output: PathBuf,

        /// Whether to use the Production environment instead of Test.
        #[arg(long)]
        production: bool,
    },
}

/// The main entry point for the `aemacs_memory` CLI tool.
/// It parses the command-line arguments and delegates to the appropriate
/// functions in the `migration` module (Extract, Import, Export).
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Extract {
            agent_id,
            start_time,
            time_step_sec,
            output,
            files,
        } => {
            println!("🛠️  [EXTRACT] Processing {} files...", files.len());
            let start_dt = start_time.parse::<DateTime<Utc>>()?;

            let records = extract_legacy_files(files, agent_id, start_dt, *time_step_sec)?;

            println!(
                "💾  [EXTRACT] Extracted {} records. Writing to {:?}...",
                records.len(),
                output
            );

            let mut out_file = std::fs::File::create(output)?;
            for record in records {
                let json = serde_json::to_string(&record)?;
                std::io::Write::write_all(&mut out_file, json.as_bytes())?;
                std::io::Write::write_all(&mut out_file, b"\n")?;
            }

            println!("✅  [EXTRACT] Done.");
        }
        Commands::Import { file, production } => {
            println!("📥  [IMPORT] Connecting to Forge Matrix...");
            let config = get_config();
            let qdrant_url = config
                .qdrant_url
                .as_deref()
                .unwrap_or("http://localhost:6334");
            let ollama_url = config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");

            let env = if *production {
                Environment::Production
            } else {
                Environment::Test
            };
            let kb = Arc::new(KnowledgeBase::new(qdrant_url, ollama_url, env)?);

            // Ensure collection exists (default dim 768 for nomic)
            kb.ensure_collection(768).await?;

            println!("📥  [IMPORT] Ingesting {file:?}...");
            import_jsonl(&kb, file).await?;
            println!("✅  [IMPORT] Done.");
        }
        Commands::Export {
            agent_id,
            output,
            production,
        } => {
            println!("📤  [EXPORT] Connecting to Forge Matrix...");
            let config = get_config();
            let qdrant_url = config
                .qdrant_url
                .as_deref()
                .unwrap_or("http://localhost:6334");
            let ollama_url = config
                .ollama_url
                .as_deref()
                .unwrap_or("http://localhost:11434");

            let env = if *production {
                Environment::Production
            } else {
                Environment::Test
            };
            let kb = Arc::new(KnowledgeBase::new(qdrant_url, ollama_url, env)?);

            println!("📤  [EXPORT] Searching matrix for agent: {agent_id}...");
            export_jsonl(&kb, agent_id, output).await?;
            println!("✅  [EXPORT] Done.");
        }
    }

    Ok(())
}
