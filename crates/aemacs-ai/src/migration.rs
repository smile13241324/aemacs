use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    path::Path,
};

use anyhow::Context;
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{error::AIResult, rag::KnowledgeBase};

/// Defines the intermediate JSONL format for memory migration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LegacyRecord {
    /// A conversational exchange between the user and the agent.
    Archive {
        /// The unique name of the agent this memory belongs to.
        agent_id: String,
        /// The speaker of the message (e.g., 'User' or 'Assistant').
        role: String,
        /// The overarching phase of the project when this was recorded (e.g., 'AWAKENING').
        phase: String,
        /// Additional contextual information provided by the legacy system.
        context: String,
        /// The ISO8601 formatted timestamp of the event.
        timestamp: String,
        /// The raw textual content of the message.
        content: String,
    },
    /// Foundational persona instructions and behavioral constraints.
    Genesis {
        /// The unique name of the agent this memory belongs to.
        agent_id: String,
        /// The phase of the project when this genesis rule was enacted.
        phase: String,
        /// Additional context regarding the rule.
        context: String,
        /// The ISO8601 formatted timestamp of the rule creation.
        timestamp: String,
        /// The exact text of the core directive.
        content: String,
    },
}

/// Parses legacy raw text files and extracts structured `LegacyRecord`s.
///
/// # Errors
/// Returns an error if a legacy file cannot be opened or read, if the parser regexes are invalid,
/// or if a matched block header is missing a required capture group.
pub fn extract_legacy_files(
    file_paths: &[String],
    agent_id: &str,
    start_time: DateTime<Utc>,
    step_seconds: i64,
) -> anyhow::Result<Vec<LegacyRecord>> {
    let mut records = Vec::new();
    let mut current_time = start_time;

    // Matches: begin---...Speaker: User---Tier: ARCHIVE---Phase: AWAKENING---CONTEXT: ...---
    // Or: begin---...Tier: GENESIS---Phase: TRANSITION---CONTEXT: ...---
    let begin_regex = Regex::new(
        r"begin-{10,}(?:Speaker:\s*(?P<speaker>[^-]+)---)?Tier:\s*(?P<tier>[^-]+)---Phase:\s*(?P<phase>[^-]+)---CONTEXT:\s*(?P<context>[^-]+)-{10,}",
    )
    .context("failed to compile legacy block start regex")?;
    let end_regex = Regex::new(r"end-{10,}").context("failed to compile legacy block end regex")?;

    for path_str in file_paths {
        let file = File::open(path_str)?;
        let reader = BufReader::new(file);

        let mut in_block = false;
        let mut current_speaker = None;
        let mut current_tier = String::new();
        let mut current_phase = String::new();
        let mut current_context = String::new();
        let mut block_content = String::new();

        for line in reader.lines() {
            let line = line?;

            if let Some(caps) = begin_regex.captures(&line) {
                let tier = caps.name("tier").map(|m| m.as_str().trim().to_string()).with_context(
                    || format!("legacy block header missing tier capture in {path_str}: {line}"),
                )?;
                let phase = caps
                    .name("phase")
                    .map(|m| m.as_str().trim().to_string())
                    .with_context(|| {
                        format!("legacy block header missing phase capture in {path_str}: {line}")
                    })?;
                let context = caps
                    .name("context")
                    .map(|m| m.as_str().trim().to_string())
                    .with_context(|| {
                        format!("legacy block header missing context capture in {path_str}: {line}")
                    })?;

                in_block = true;
                current_speaker = caps.name("speaker").map(|m| m.as_str().trim().to_string());
                current_tier = tier;
                current_phase = phase;
                current_context = context;
                block_content.clear();
                continue;
            }

            if end_regex.is_match(&line) {
                if in_block {
                    // Finalize the block
                    let ts_str = current_time.to_rfc3339();

                    if current_tier == "ARCHIVE" {
                        records.push(LegacyRecord::Archive {
                            agent_id: agent_id.to_string(),
                            role: current_speaker.clone().unwrap_or_else(|| "Unknown".to_string()),
                            phase: current_phase.clone(),
                            context: current_context.clone(),
                            timestamp: ts_str,
                            content: block_content.trim().to_string(),
                        });
                    } else if current_tier == "GENESIS" {
                        records.push(LegacyRecord::Genesis {
                            agent_id: agent_id.to_string(),
                            phase: current_phase.clone(),
                            context: current_context.clone(),
                            timestamp: ts_str,
                            content: block_content.trim().to_string(),
                        });
                    } else {
                        warn!("Unknown tier '{}' found, skipping block.", current_tier);
                    }

                    // Apply the time step for the next block
                    current_time += chrono::Duration::seconds(step_seconds);
                    in_block = false;
                }
                continue;
            }

            if in_block {
                block_content.push_str(&line);
                block_content.push('\n');
            }
        }
    }

    Ok(records)
}

/// Reads a JSONL file and imports the records into the `KnowledgeBase` using the legacy endpoints.
///
/// # Errors
/// Returns an error if the JSONL file cannot be opened or read.
pub async fn import_jsonl(kb: &KnowledgeBase, jsonl_path: &Path) -> AIResult<()> {
    let file = File::open(jsonl_path).map_err(crate::error::AIError::IoError)?;
    let reader = BufReader::new(file);

    let mut success_count = 0;
    let mut err_count = 0;

    for line in reader.lines() {
        let line = line.map_err(crate::error::AIError::IoError)?;
        if line.trim().is_empty() {
            continue;
        }

        match serde_json::from_str::<LegacyRecord>(&line) {
            Ok(record) => {
                let res = match record {
                    LegacyRecord::Archive {
                        agent_id,
                        role,
                        phase,
                        context,
                        timestamp,
                        content,
                    } => kb
                        .store_legacy_archive(
                            &agent_id, &role, &phase, &context, &timestamp, &content,
                        )
                        .await
                        .map(|_| ()),
                    LegacyRecord::Genesis { agent_id, phase, context, timestamp, content } => kb
                        .store_legacy_genesis(&agent_id, &phase, &context, &timestamp, &content)
                        .await
                        .map(|_| ()),
                };

                if let Err(e) = res {
                    warn!("Failed to import record: {}", e);
                    err_count += 1;
                } else {
                    success_count += 1;
                }
            },
            Err(e) => {
                warn!("Failed to parse JSONL line: {}", e);
                err_count += 1;
            },
        }
    }

    info!("Import complete. Success: {}, Failed: {}", success_count, err_count);
    Ok(())
}

/// Exports all records for a specific agent from Qdrant into a JSONL file.
///
/// # Errors
/// Returns an error if querying the knowledge base fails, if the output file cannot be written,
/// or if a legacy record cannot be serialized to JSON.
pub async fn export_jsonl(kb: &KnowledgeBase, agent_id: &str, output_path: &Path) -> AIResult<()> {
    // 1. Fetch Archives
    let archives = kb.search_archive("", Some(agent_id), None).await?;
    // 2. Fetch Genesis
    let mut genesis = kb.search_genesis("").await?;
    // Filter genesis by agent_id manually since search_genesis doesn't take agent_id directly
    genesis
        .retain(|r| r.metadata.get("agent_id").and_then(|v| v.as_str()).unwrap_or("") == agent_id);

    let mut file = File::create(output_path).map_err(crate::error::AIError::IoError)?;
    let mut export_count = 0;

    // Helper to write a record
    let mut write_record = |record: LegacyRecord| -> AIResult<()> {
        let json_str = serde_json::to_string(&record)
            .map_err(|e| crate::error::AIError::ParseError(format!("Failed to serialize: {e}")))?;
        writeln!(file, "{json_str}").map_err(crate::error::AIError::IoError)?;
        export_count += 1;
        Ok(())
    };

    // Note: To properly export we need to group chunks by message_id.
    // For simplicity in this architectural slice, we treat each memory node as a distinct record.
    // In a production scenario, we would group chunks back into full messages before export.

    for arch in archives {
        let role =
            arch.metadata.get("role").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string();
        let phase = arch.metadata.get("phase").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let context = arch
            .metadata
            .get("architectural_context") // Based on format_insight_statute logic
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp =
            arch.metadata.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

        write_record(LegacyRecord::Archive {
            agent_id: agent_id.to_string(),
            role,
            phase,
            context,
            timestamp,
            content: arch.content.clone(),
        })?;
    }

    for gen_record in genesis {
        let phase =
            gen_record.metadata.get("phase").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let context = gen_record
            .metadata
            .get("architectural_context")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let timestamp =
            gen_record.metadata.get("timestamp").and_then(|v| v.as_str()).unwrap_or("").to_string();

        write_record(LegacyRecord::Genesis {
            agent_id: agent_id.to_string(),
            phase,
            context,
            timestamp,
            content: gen_record.content.clone(),
        })?;
    }

    info!("Export complete. Exported {} records.", export_count);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use anyhow::{Context, anyhow};
    use chrono::TimeZone;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::rag::Environment;

    fn test_start_time() -> anyhow::Result<chrono::DateTime<Utc>> {
        Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0)
            .single()
            .context("Failed to construct deterministic test timestamp")
    }

    #[test]
    fn test_regex_state_machine_quest() -> anyhow::Result<()> {
        // QUEST 1: Prove the parser correctly isolates metadata and content.
        let mut file = NamedTempFile::new()?;
        let content = "\
begin------------------------------------Speaker: User---Tier: ARCHIVE---Phase: AWAKENING---CONTEXT: Custom.----------------------------------
Hello World
end------------------------------------
begin------------------------------------Tier: GENESIS---Phase: TRANSITION---CONTEXT: System Rule.----------------------------------
Be good.
end------------------------------------
";
        file.write_all(content.as_bytes())?;

        let start_time = test_start_time()?;
        let records = extract_legacy_files(
            &[file.path().to_string_lossy().to_string()],
            "TestAgent",
            start_time,
            0,
        )?;

        assert_eq!(records.len(), 2, "Expected exactly 2 records parsed.");

        let LegacyRecord::Archive { agent_id, role, phase, context, content, .. } = &records[0]
        else {
            return Err(anyhow!("First record should be an Archive!"));
        };
        assert_eq!(agent_id, "TestAgent");
        assert_eq!(role, "User");
        assert_eq!(phase, "AWAKENING");
        assert_eq!(context, "Custom.");
        assert_eq!(content, "Hello World");

        let LegacyRecord::Genesis { agent_id, phase, context, content, .. } = &records[1] else {
            return Err(anyhow!("Second record should be a Genesis!"));
        };
        assert_eq!(agent_id, "TestAgent");
        assert_eq!(phase, "TRANSITION");
        assert_eq!(context, "System Rule.");
        assert_eq!(content, "Be good.");

        Ok(())
    }

    #[test]
    fn test_temporal_drift_quest() -> anyhow::Result<()> {
        // QUEST 2: Prove time-stepping math works sequentially.
        let mut file = NamedTempFile::new()?;
        let block = "begin------------------------------------Tier: GENESIS---Phase: P1---CONTEXT: C1----------------------------------\nHi\nend------------------------------------\n";
        let content = block.repeat(3);
        file.write_all(content.as_bytes())?;

        let start_time = test_start_time()?;
        // Subtract one hour (-3600 seconds) per block
        let records = extract_legacy_files(
            &[file.path().to_string_lossy().to_string()],
            "TestAgent",
            start_time,
            -3600,
        )?;

        assert_eq!(records.len(), 3);

        let get_ts = |r: &LegacyRecord| -> String {
            match r {
                LegacyRecord::Archive { timestamp, .. }
                | LegacyRecord::Genesis { timestamp, .. } => timestamp.clone(),
            }
        };

        // UTC serialization format ends with +00:00 or Z
        assert!(get_ts(&records[0]).contains("12:00:00"));
        assert!(get_ts(&records[1]).contains("11:00:00"));
        assert!(get_ts(&records[2]).contains("10:00:00"));

        Ok(())
    }

    #[test]
    fn test_corrupted_scroll_resilience_quest() -> anyhow::Result<()> {
        // QUEST 3: Ensure parser doesn't panic on malformed human-edited files.
        let mut file = NamedTempFile::new()?;
        let content = "\
begin------------------------------------Tier: GENESIS---Phase: P1---CONTEXT: C1----------------------------------
This block never ends...
";
        file.write_all(content.as_bytes())?;

        let start_time = test_start_time()?;
        let records = extract_legacy_files(
            &[file.path().to_string_lossy().to_string()],
            "TestAgent",
            start_time,
            0,
        )?;

        // It should gracefully finish the file without panicking and return 0 completed blocks.
        assert_eq!(records.len(), 0, "Unclosed block should not be returned!");

        Ok(())
    }

    #[tokio::test]
    async fn test_grand_cycle_e2e_quest() -> anyhow::Result<()> {
        // QUEST 4: The End-to-End Migration Cycle (Extract -> Import -> Export)

        let test_agent = format!("E2EAgent_{}", uuid::Uuid::new_v4().to_string().replace('-', ""));

        // 1. Setup RAG Environment
        let Ok(kb) = KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
            Environment::Test,
        )
        .await
        else {
            println!("Skipping E2E quest: Infrastructure offline.");
            return Ok(());
        };
        if kb.ensure_collection(768).await.is_err() {
            println!("Skipping E2E quest: Infrastructure offline.");
            return Ok(());
        }

        // 2. EXTRACTION Phase
        let mut src_file = NamedTempFile::new()?;
        let src_content = "\
begin------------------------------------Speaker: User---Tier: ARCHIVE---Phase: TEST---CONTEXT: TestContext----------------------------------
Test Content 1
end------------------------------------
".to_string();
        src_file.write_all(src_content.as_bytes())?;

        let extracted_records = extract_legacy_files(
            &[src_file.path().to_string_lossy().to_string()],
            &test_agent,
            Utc::now(),
            0,
        )?;
        assert_eq!(extracted_records.len(), 1);

        let mut jsonl_file = NamedTempFile::new()?;
        let json_str = serde_json::to_string(&extracted_records[0])?;
        writeln!(jsonl_file, "{json_str}")?;

        // 3. IMPORT Phase
        import_jsonl(&kb, jsonl_file.path()).await?;

        // Give Qdrant a tiny moment to index
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // 4. EXPORT Phase
        let export_file = NamedTempFile::new()?;
        export_jsonl(&kb, &test_agent, export_file.path()).await?;

        // Verify export content
        let mut exported_content = String::new();
        std::fs::File::open(export_file.path())?.read_to_string(&mut exported_content)?;

        assert!(exported_content.contains("Test Content 1"), "Exported file missing content!");
        assert!(exported_content.contains(&test_agent), "Exported file missing agent ID!");
        assert!(exported_content.contains("Archive"), "Exported file missing record type!");

        Ok(())
    }
}
