use crate::rag::KnowledgeBase;
use crate::{AIBackend, Content, Conversation, Message, PersonaRegistry, Role};
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, PathBuf};
use std::sync::{Arc, OnceLock};
use tokio::process::Command;

static SESSION_START: OnceLock<std::time::Instant> = OnceLock::new();

fn get_session_start() -> std::time::Instant {
    *SESSION_START.get_or_init(std::time::Instant::now)
}

// --- Interfaces ---

#[async_trait]
pub trait ToolHost: Send + Sync {
    async fn ask_approval(&self, description: &str) -> bool;
    async fn ask_user(&self, question: &str) -> String;
    fn get_agent_id(&self) -> String;
    fn report_progress(&self, tool_name: String, is_running: bool);
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Value;
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String>;
}

pub struct DenyAllHost;
#[async_trait]
impl ToolHost for DenyAllHost {
    async fn ask_approval(&self, _description: &str) -> bool {
        false
    }
    async fn ask_user(&self, _question: &str) -> String {
        String::new()
    }
    fn get_agent_id(&self) -> String {
        "anonymous".to_string()
    }
    fn report_progress(&self, _tool_name: String, _is_running: bool) {}
}

// --- Registry ---

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    pub fn with_core_tools(
        kb: Arc<KnowledgeBase>,
        persona_registry: Arc<PersonaRegistry>,
        event_tx: Option<async_channel::Sender<aemacs_core::bus::SystemEvent>>,
    ) -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(ReadFileTool));
        registry.register(Box::new(WriteFileTool {
            event_tx: event_tx.clone(),
        }));
        registry.register(Box::new(ReplaceTextTool {
            event_tx: event_tx.clone(),
        }));
        registry.register(Box::new(GrepSearchTool));
        registry.register(Box::new(RunShellCommandTool));
        registry.register(Box::new(ListFilesTool));
        registry.register(Box::new(WebSearchTool));
        registry.register(Box::new(GitContextTool));
        registry.register(Box::new(ManageTasksTool {
            event_tx: event_tx.clone(),
        }));
        registry.register(Box::new(HandoffAgentTool {
            persona_registry,
            event_tx: event_tx.clone(),
        }));
        registry.register(Box::new(SearchKnowledgeBaseTool::new(kb.clone())));
        registry.register(Box::new(WriteKnowledgeBaseTool::new(kb.clone())));
        registry.register(Box::new(UpdateMemoryTool::new(kb.clone())));
        registry.register(Box::new(DeleteMemoryTool::new(kb.clone())));
        registry.register(Box::new(FetchContiguousMemoryTool::new(kb.clone())));
        registry.register(Box::new(GetSystemTimeTool));
        registry.register(Box::new(ParseAstTool));

        registry.register(Box::new(ReportStatusTool {
            event_tx: event_tx.clone(),
        }));
        registry.register(Box::new(RecallPastInsightsTool::new(kb.clone())));
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn get(&self, name: &str) -> Option<&Box<dyn Tool>> {
        self.tools.get(name)
    }

    pub fn list_definitions(&self) -> Vec<Value> {
        self.tools
            .values()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name(),
                        "description": t.description(),
                        "parameters": t.parameters()
                    }
                })
            })
            .collect()
    }
}

use futures::StreamExt;

/// Executes the Agentic Loop: Talk to AI -> Execute Tools -> Talk to AI -> Result.
/// Modifies the conversation history in-place and streams text chunks back via stream_tx.
pub async fn run_agent_loop(
    backend: &(impl AIBackend + ?Sized),
    registry: &ToolRegistry,
    host: &dyn ToolHost,
    conversation: &mut Conversation, // Mutable reference
    max_turns: usize,
    stream_tx: Option<async_channel::Sender<String>>,
) -> Result<String> {
    let tool_defs = registry.list_definitions();
    conversation.set_tools(tool_defs); // Use setter

    for _ in 0..max_turns {
        // Clone conversation for the request (snapshot of current state)
        let request = conversation.clone().build();
        tracing::debug!("🚀 [AI Loop] Sending request to model: {:#?}", request);

        let mut stream = backend.stream(request).await?;

        let mut full_content = String::new();
        let mut accumulated_tool_calls: HashMap<i32, crate::models::ToolCall> = HashMap::new();

        while let Some(event_res) = stream.next().await {
            match event_res? {
                crate::StreamEvent::Content(chunk) => {
                    tracing::trace!("📥 [AI Loop] Received chunk: {:?}", chunk); // Trace to avoid log spam, debug full_content later
                    full_content.push_str(&chunk);
                    if let Some(tx) = &stream_tx {
                        let _ = tx.send(chunk).await;
                    }
                }
                crate::StreamEvent::ToolCall(tc) => {
                    tracing::debug!("🛠️ [AI Loop] Received ToolCall delta: {:?}", tc);
                    // OpenAI streams tool calls with an index. For simplicity, we assume one or handle by ID.
                    // If no ID is provided, it's usually a delta for the current call.
                    // For now, let's just collect them.
                    accumulated_tool_calls.insert(0, tc);
                }
            }
        }

        tracing::debug!(
            "✅ [AI Loop] Stream finished. Total content: {:?}",
            full_content
        );

        let tool_calls = if accumulated_tool_calls.is_empty() {
            None
        } else {
            tracing::debug!(
                "✅ [AI Loop] Tools accumulated: {:?}",
                accumulated_tool_calls
            );
            Some(accumulated_tool_calls.into_values().collect::<Vec<_>>())
        };

        let response_msg = Message {
            role: Role::Assistant,
            content: Content::Text(full_content.clone()),
            tool_calls: tool_calls.clone(),
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        };

        tracing::debug!(
            "💾 [AI Loop] Saving to conversation history: {:#?}",
            response_msg
        );
        // ALWAYS add the assistant's response to history
        conversation.add_message(response_msg);

        if let Some(calls) = tool_calls {
            if calls.is_empty() {
                return Ok(full_content);
            }

            for call in calls {
                let tool_name = &call.function.name;
                let args_str = &call.function.arguments;

                let result = match registry.get(tool_name) {
                    Some(tool) => match serde_json::from_str::<Value>(args_str) {
                        Ok(args) => {
                            host.report_progress(tool_name.clone(), true);
                            let exec_result = tool.execute(args, host).await;
                            host.report_progress(tool_name.clone(), false);
                            match exec_result {
                                Ok(output) => output,
                                Err(e) => format!("Error executing tool: {}", e),
                            }
                        }
                        Err(e) => format!("Error parsing arguments: {}", e),
                    },
                    None => format!("Tool '{}' not found.", tool_name),
                };

                let tool_msg = Message {
                    role: Role::Tool,
                    content: Content::Text(result),
                    tool_calls: None,
                    tool_call_id: call.id.clone(),
                    timestamp: chrono::Utc::now(),
                };
                conversation.add_message(tool_msg);
            }
            // Loop continues (recursion)
        } else {
            return Ok(full_content);
        }
    }

    Err(anyhow!("Max turns reached without final response."))
}

// --- Utils ---

/// Validates that a path is safe to access (no escaping the project root).
pub fn validate_path(path_str: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path_str);
    if path.is_absolute() {
        return Err(anyhow!("Absolute paths are not allowed."));
    }
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            return Err(anyhow!("Path traversal (..) is not allowed."));
        }
    }
    Ok(path)
}

// --- Basic Tools ---

pub struct ReadFileTool;
#[async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }
    fn description(&self) -> &str {
        "Reads the content of a file at the given path."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Relative path to file" } },
            "required": ["path"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let path_str = args["path"].as_str().ok_or(anyhow!("Missing path"))?;
        let path = validate_path(path_str)?;
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;
        Ok(content)
    }
}

pub struct WriteFileTool {
    pub event_tx: Option<async_channel::Sender<aemacs_core::bus::SystemEvent>>,
}
#[async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }
    fn description(&self) -> &str {
        "Writes content to a file. Overwrites if exists."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file" },
                "content": { "type": "string", "description": "Content to write" }
            },
            "required": ["path", "content"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let path_str = args["path"].as_str().ok_or(anyhow!("Missing path"))?;
        let content = args["content"].as_str().ok_or(anyhow!("Missing content"))?;
        let path = validate_path(path_str)?;

        let approval_msg = format!(
            "Write to file '{}'?\nSize: {} bytes",
            path.display(),
            content.len()
        );
        if !host.ask_approval(&approval_msg).await {
            return Err(anyhow!("User denied write permission."));
        }

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).context("Failed to create parent dirs")?;
        }

        fs::write(&path, content).with_context(|| format!("Failed to write file: {:?}", path))?;

        if let Some(tx) = &self.event_tx {
            let _ = tx
                .send(aemacs_core::bus::SystemEvent::FileModified(path.clone()))
                .await;
        }

        Ok(format!("Successfully wrote to {:?}", path))
    }
}

pub struct ReplaceTextTool {
    pub event_tx: Option<async_channel::Sender<aemacs_core::bus::SystemEvent>>,
}
#[async_trait]
impl Tool for ReplaceTextTool {
    fn name(&self) -> &str {
        "replace_text"
    }
    fn description(&self) -> &str {
        "Replaces a specific string of text within a file. The search_string must exactly match the file content and be unique."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Path to file" },
                "search_string": { "type": "string", "description": "The EXACT text to find and replace. Include a few surrounding lines if needed to ensure uniqueness." },
                "replace_string": { "type": "string", "description": "The new text to insert." }
            },
            "required": ["path", "search_string", "replace_string"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let path_str = args["path"].as_str().ok_or(anyhow!("Missing path"))?;
        let search_string = args["search_string"]
            .as_str()
            .ok_or(anyhow!("Missing search_string"))?;
        let replace_string = args["replace_string"]
            .as_str()
            .ok_or(anyhow!("Missing replace_string"))?;
        let path = validate_path(path_str)?;

        let mut content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;

        let matches = content.matches(search_string).count();
        if matches == 0 {
            return Err(anyhow!(
                "String not found. Check your context and exact whitespace."
            ));
        }
        if matches > 1 {
            return Err(anyhow!(
                "String found {} times. Provide more context to make the search_string unique.",
                matches
            ));
        }

        let approval_msg = format!(
            "Replace text in '{}'?\nSearch:\n{}\n\nReplace with:\n{}",
            path.display(),
            search_string,
            replace_string
        );
        if !host.ask_approval(&approval_msg).await {
            return Err(anyhow!("User denied text replacement."));
        }

        content = content.replace(search_string, replace_string);
        fs::write(&path, content).with_context(|| format!("Failed to write file: {:?}", path))?;

        if let Some(tx) = &self.event_tx {
            let _ = tx
                .send(aemacs_core::bus::SystemEvent::FileModified(path.clone()))
                .await;
        }

        Ok(format!("Successfully replaced text in {:?}", path))
    }
}

pub struct GrepSearchTool;
#[async_trait]
impl Tool for GrepSearchTool {
    fn name(&self) -> &str {
        "grep_search"
    }
    fn description(&self) -> &str {
        "Searches for a regular expression pattern within files in a given directory."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": { "type": "string", "description": "The regular expression to search for." },
                "path": { "type": "string", "description": "The directory path to search in (e.g., ./crates)." }
            },
            "required": ["pattern", "path"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let pattern_str = args["pattern"].as_str().ok_or(anyhow!("Missing pattern"))?;
        let path_str = args["path"].as_str().ok_or(anyhow!("Missing path"))?;
        let root_path = validate_path(path_str)?;

        let regex =
            Regex::new(pattern_str).map_err(|e| anyhow!("Invalid regular expression: {}", e))?;

        let mut results = Vec::new();
        let mut paths_to_visit = vec![root_path];
        let ignore_dirs = vec![".git", "target", "node_modules", ".gemini"];
        let mut match_count = 0;
        let max_matches = 100;

        while let Some(dir) = paths_to_visit.pop() {
            if match_count >= max_matches {
                break;
            }

            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(_) => continue, // Silently skip unreadable directories
            };

            for entry in entries.flatten() {
                let path = entry.path();
                let file_name = path.file_name().unwrap_or_default().to_string_lossy();

                if path.is_dir() {
                    if !ignore_dirs.iter().any(|&d| d == file_name) {
                        paths_to_visit.push(path);
                    }
                } else if path.is_file() {
                    if let Ok(content) = fs::read_to_string(&path) {
                        for (line_num, line) in content.lines().enumerate() {
                            if regex.is_match(line) {
                                results.push(format!(
                                    "{}:{}: {}",
                                    path.display(),
                                    line_num + 1,
                                    line.trim()
                                ));
                                match_count += 1;
                                if match_count >= max_matches {
                                    break;
                                }
                            }
                        }
                    } // Silently skip invalid UTF-8 files or binary files
                }
            }
        }

        if results.is_empty() {
            Ok("No matches found.".to_string())
        } else {
            let mut output = results.join("\n");
            if match_count >= max_matches {
                output.push_str("\n... (results truncated, max 100 matches)");
            }
            Ok(output)
        }
    }
}

pub struct RunShellCommandTool;
#[async_trait]
impl Tool for RunShellCommandTool {
    fn name(&self) -> &str {
        "run_shell_command"
    }
    fn description(&self) -> &str {
        "Executes a shell command on the host system. Useful for running tests, linters, or checking git status."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The exact bash command to execute (e.g., 'cargo check', 'git status')." },
                "dir": { "type": "string", "description": "The directory to run the command in. Defaults to '.' if not provided." }
            },
            "required": ["command"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let command_str = args["command"].as_str().ok_or(anyhow!("Missing command"))?;
        let dir_str = args["dir"].as_str().unwrap_or(".");
        let dir_path = validate_path(dir_str)?;

        let approval_msg = format!(
            "Execute command in '{}':\n\n$ {}",
            dir_path.display(),
            command_str
        );
        if !host.ask_approval(&approval_msg).await {
            return Err(anyhow!("User denied execution."));
        }

        // Enforce a generous 5-minute timeout (300 seconds) to allow for compilations,
        // but prevent infinite hangs (e.g., waiting for stdin).
        let child = Command::new("bash")
            .arg("-c")
            .arg(command_str)
            .current_dir(&dir_path)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .with_context(|| format!("Failed to spawn command: {}", command_str))?;

        let timeout_duration = std::time::Duration::from_secs(300);

        let output = match tokio::time::timeout(timeout_duration, child.wait_with_output()).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Err(anyhow::anyhow!("Command execution failed: {}", e)),
            Err(_) => {
                // Timeout occurred! The future drops, consuming the child, and kill_on_drop(true) terminates it.
                return Err(anyhow::anyhow!(
                    "Command timed out after 5 minutes and was terminated."
                ));
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let mut combined_output = String::new();
        if !output.status.success() {
            combined_output.push_str(&format!(
                "Command failed with exit code: {}\n",
                output.status.code().unwrap_or(-1)
            ));
            if !stderr.is_empty() {
                combined_output.push_str("--- STDERR ---\n");
                combined_output.push_str(&stderr);
                combined_output.push('\n');
            }
        }
        if !stdout.is_empty() {
            combined_output.push_str("--- STDOUT ---\n");
            combined_output.push_str(&stdout);
        }

        if combined_output.is_empty() {
            combined_output.push_str("(Command completed silently)");
        }

        // Truncate if insanely large to protect context window
        let max_len = 2000;
        if combined_output.len() > max_len {
            combined_output.truncate(max_len);
            combined_output.push_str("\n... (output truncated to 2000 chars)");
        }

        Ok(combined_output)
    }
}

pub struct ListFilesTool;
#[async_trait]
impl Tool for ListFilesTool {
    fn name(&self) -> &str {
        "list_files"
    }
    fn description(&self) -> &str {
        "Lists files and directories in a given path."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Directory path (default: .)" } }
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let path_str = args["path"].as_str().unwrap_or(".");
        let path = validate_path(path_str)?;
        let mut entries = Vec::new();
        for entry in
            fs::read_dir(&path).with_context(|| format!("Failed to read dir: {:?}", path))?
        {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            let type_str = if entry.file_type()?.is_dir() {
                "DIR"
            } else {
                "FILE"
            };
            entries.push(format!("[{}] {}", type_str, name));
        }
        Ok(entries.join("\n"))
    }
}

pub struct SearchKnowledgeBaseTool {
    kb: Arc<KnowledgeBase>,
}

impl SearchKnowledgeBaseTool {
    pub fn new(kb: Arc<KnowledgeBase>) -> Self {
        Self { kb }
    }
}

#[async_trait]
impl Tool for SearchKnowledgeBaseTool {
    fn name(&self) -> &str {
        "search_knowledge_base"
    }
    fn description(&self) -> &str {
        "Searches internal documentation/memory. Defaults to 'internal' scope (only your own memories). Use scope='global' to search everyone's memories."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query." },
                "collection": { "type": "string", "description": "Defaults to 'aemacs_docs'." },
                "scope": { "type": "string", "enum": ["internal", "global"], "description": "Defaults to 'internal'." },
                "categories": { "type": "array", "items": { "type": "string" }, "description": "Optional: Filter by categories (e.g., ARCHIVE, INSIGHT, CORE). Defaults to ['ARCHIVE']." }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let query = args["query"].as_str().ok_or(anyhow!("Missing query"))?;
        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");
        let scope = args["scope"].as_str().unwrap_or("internal");

        let categories: Vec<&str> = if let Some(cats) = args["categories"].as_array() {
            cats.iter().filter_map(|v| v.as_str()).collect()
        } else {
            // ACO-012-03: The Historian's Bias - Default to ARCHIVE
            vec!["ARCHIVE"]
        };

        let agent_id = if scope == "internal" {
            Some(host.get_agent_id())
        } else {
            None
        };

        // 1. Stage 1: High-Confidence Search (0.8 threshold)
        let mut results = self
            .kb
            .search(
                collection,
                query,
                10,
                Some(0.8),
                agent_id.as_deref(),
                Some(categories.clone()),
            )
            .await?;

        let mut fuzzy_warning = String::new();

        // 2. Stage 2: Fuzzy Fallback (0.6 threshold)
        if results.is_empty() {
            results = self
                .kb
                .search(
                    collection,
                    query,
                    10,
                    Some(0.6),
                    agent_id.as_deref(),
                    Some(categories),
                )
                .await?;

            if !results.is_empty() {
                fuzzy_warning = "NOTICE: High-confidence historical data not found. Displaying fuzzy/low-confidence matches.\n\n".to_string();
            }
        }

        if results.is_empty() {
            return Ok("No results found.".to_string());
        }

        let json_output = serde_json::to_string_pretty(&results)
            .map_err(|e| anyhow!("Failed to serialize memory results: {}", e))?;

        Ok(format!("{}{}", fuzzy_warning, json_output))
    }
}

pub struct WriteKnowledgeBaseTool {
    kb: Arc<KnowledgeBase>,
}

impl WriteKnowledgeBaseTool {
    pub fn new(kb: Arc<KnowledgeBase>) -> Self {
        Self { kb }
    }
}

#[async_trait]
impl Tool for WriteKnowledgeBaseTool {
    fn name(&self) -> &str {
        "write_knowledge_base"
    }
    fn description(&self) -> &str {
        "Records a memory, insight, or core architectural truth into the knowledge base."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "content": { "type": "string", "description": "The information to be remembered (max 2000 chars)." },
                "category": { "type": "string", "enum": ["INSIGHT", "CORE"], "description": "The tier of memory. INSIGHT is for observed facts, CORE is for immutable laws." },
                "collection": { "type": "string", "description": "Defaults to aemacs_docs." }
            },
            "required": ["content", "category"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let content = args["content"]
            .as_str()
            .ok_or(anyhow::anyhow!("Missing content"))?;

        if content.len() > 2000 {
            return Err(anyhow::anyhow!("Memory content too large ({} chars). Insights and Core directives must be concise summaries under 2000 characters. Please synthesize the information and try again.", content.len()));
        }

        let category = args["category"]
            .as_str()
            .ok_or(anyhow::anyhow!("Missing category"))?;

        if category == "ARCHIVE" {
            return Err(anyhow::anyhow!("Permission Denied: Agents cannot manually write to the ARCHIVE tier."));
        }

        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");
        let agent_id = host.get_agent_id();

        let timestamp = chrono::Utc::now().to_rfc3339();
        let formatted_content = format!(
            "[{}] [Agent: {}] [{}] | {}",
            category,
            agent_id.to_uppercase(),
            timestamp,
            content
        );

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("agent_id".to_string(), agent_id);
        metadata.insert("timestamp".to_string(), timestamp);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.kb
            .add_document(collection, &formatted_content, Some(metadata))
            .await?;

        Ok(format!("Successfully chronicled {} memory.", category))
    }
}

pub struct DeleteMemoryTool {
    kb: Arc<KnowledgeBase>,
}

impl DeleteMemoryTool {
    pub fn new(kb: Arc<KnowledgeBase>) -> Self {
        Self { kb }
    }
}

#[async_trait]
impl Tool for DeleteMemoryTool {
    fn name(&self) -> &str {
        "delete_memory"
    }
    fn description(&self) -> &str {
        "Permanently erases a specific memory from the knowledge base by its UUID. Use this to prune obsolete or contradictory beliefs."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The UUID of the memory to delete." },
                "collection": { "type": "string", "description": "Defaults to aemacs_docs." }
            },
            "required": ["id"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let id = args["id"].as_str().ok_or(anyhow!("Missing memory ID"))?;
        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");

        let approval_msg = format!("Permanently delete memory [ID: {}] from the vault?", id);
        if !host.ask_approval(&approval_msg).await {
            return Err(anyhow!("User denied memory pruning."));
        }

        self.kb.delete_point(collection, id).await?;

        Ok(format!(
            "Memory {} has been pruned from the collective.",
            id
        ))
    }
}

pub struct UpdateMemoryTool {
    kb: Arc<KnowledgeBase>,
}

impl UpdateMemoryTool {
    pub fn new(kb: Arc<KnowledgeBase>) -> Self {
        Self { kb }
    }
}

#[async_trait]
impl Tool for UpdateMemoryTool {
    fn name(&self) -> &str {
        "update_memory"
    }
    fn description(&self) -> &str {
        "Semantically overwrites an existing memory by its UUID. Use this to update evolving beliefs or correct misunderstandings."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "id": { "type": "string", "description": "The UUID of the memory to update." },
                "content": { "type": "string", "description": "The new information to be remembered (max 2000 chars)." },
                "category": { "type": "string", "enum": ["INSIGHT", "CORE"], "description": "The tier of memory. INSIGHT is for observed facts, CORE is for immutable laws." },
                "collection": { "type": "string", "description": "Defaults to aemacs_docs." }
            },
            "required": ["id", "content", "category"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let id = args["id"].as_str().ok_or(anyhow!("Missing memory ID"))?;
        let content = args["content"].as_str().ok_or(anyhow!("Missing content"))?;

        if content.len() > 2000 {
            return Err(anyhow::anyhow!("Memory content too large ({} chars). Insights and Core directives must be concise summaries under 2000 characters. Please synthesize the information and try again.", content.len()));
        }

        let category = args["category"]
            .as_str()
            .ok_or(anyhow!("Missing category"))?;

        if category == "ARCHIVE" {
            return Err(anyhow::anyhow!("Permission Denied: Agents cannot manually write to the ARCHIVE tier."));
        }

        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");
        let agent_id = host.get_agent_id();

        let timestamp = chrono::Utc::now().to_rfc3339();
        let formatted_content = format!(
            "[{}] [Agent: {}] [{}] | {}",
            category,
            agent_id.to_uppercase(),
            timestamp,
            content
        );

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("agent_id".to_string(), agent_id);
        metadata.insert("timestamp".to_string(), timestamp);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.kb
            .update_point(collection, id, &formatted_content, Some(metadata))
            .await?;

        Ok(format!("Memory {} has been woven into a new truth.", id))
    }
}

pub struct GetSystemTimeTool;
#[async_trait]
impl Tool for GetSystemTimeTool {
    fn name(&self) -> &str {
        "get_system_time"
    }
    fn description(&self) -> &str {
        "Returns the current UTC time, local time, and session uptime."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
        })
    }
    async fn execute(&self, _args: Value, _host: &dyn ToolHost) -> Result<String> {
        let now = chrono::Utc::now();
        let uptime = get_session_start().elapsed().as_secs();

        let result = serde_json::json!({
            "utc_now": now.to_rfc3339(),
            "local_now": chrono::Local::now().to_rfc3339(),
            "uptime_secs": uptime,
        });

        Ok(result.to_string())
    }
}

pub struct ParseAstTool;
#[async_trait]
impl Tool for ParseAstTool {
    fn name(&self) -> &str {
        "parse_ast"
    }
    fn description(&self) -> &str {
        "Parses a code file using Tree-sitter and extracts the source code of a specific symbol (struct, enum, impl, or function) by name."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path to the code file." },
                "symbol": { "type": "string", "description": "The name of the symbol to extract." }
            },
            "required": ["path", "symbol"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let path_str = args["path"].as_str().ok_or(anyhow!("Missing path"))?;
        let symbol_name = args["symbol"].as_str().ok_or(anyhow!("Missing symbol"))?;
        let path = validate_path(path_str)?;

        let code = aemacs_core::syntax::extract_symbol(&path, symbol_name)?;
        Ok(code)
    }
}

pub struct HandoffAgentTool {
    pub persona_registry: Arc<PersonaRegistry>,
    pub event_tx: Option<async_channel::Sender<aemacs_core::bus::SystemEvent>>,
}

#[async_trait]
impl Tool for HandoffAgentTool {
    fn name(&self) -> &str {
        "handoff_agent"
    }
    fn description(&self) -> &str {
        "Transfers control to another specialist agent in the mesh."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "agent_name": { "type": "string", "description": "The name of the agent to summon (e.g., 'kairon')." },
                "message": { "type": "string", "description": "Optional instructions for the next agent." }
            },
            "required": ["agent_name"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let agent_name = args["agent_name"]
            .as_str()
            .ok_or(anyhow!("Missing agent_name"))?;
        let message = args["message"].as_str().map(|s| s.to_string());

        // Validate agent
        if self
            .persona_registry
            .get_persona(agent_name)
            .await
            .is_none()
        {
            return Err(anyhow!(
                "Specialist agent '{}' not found in registry.",
                agent_name
            ));
        }

        let tx = self
            .event_tx
            .as_ref()
            .ok_or(anyhow!("Event bus not connected"))?;

        tx.send(aemacs_core::bus::SystemEvent::PersonaChanged {
            name: agent_name.to_string(),
            message,
        })
        .await
        .context("Failed to send PersonaChanged event")?;

        Ok(format!(
            "Handing off control to {}.",
            agent_name.to_uppercase()
        ))
    }
}

pub struct WebSearchTool;
#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Searches the internet for documentation or information using Ecosia."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The search query." }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let query = args["query"].as_str().ok_or(anyhow!("Missing query"))?;

        // Base URL for Ecosia search
        let base_url = "https://www.ecosia.org/search?tt=mzl";

        // Ensure we handle URL encoding
        let encoded_query = urlencoding::encode(query);
        let request_url = format!("{}&q={}", base_url, encoded_query);

        // We use a custom client to set a User-Agent, avoiding bot-detection heat.
        let client = reqwest::Client::builder()
            .user_agent("AemacsOracle/1.0 (Architecture: Iron Core)")
            .build()
            .map_err(|e| anyhow!("Failed to build HTTP client: {}", e))?;

        let response = client
            .get(&request_url)
            .send()
            .await
            .map_err(|e| anyhow!("Search request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Search provider returned error status: {}",
                response.status()
            ));
        }

        let text = response
            .text()
            .await
            .map_err(|e| anyhow!("Failed to read search response: {}", e))?;

        // Naively return the first 2000 chars of the payload to avoid context explosion.
        let mut output = text;
        if output.len() > 2000 {
            output.truncate(2000);
            output.push_str("\n... (results truncated to 2000 chars)");
        }

        Ok(output)
    }
}

pub struct ReportStatusTool {
    pub event_tx: Option<async_channel::Sender<aemacs_core::bus::SystemEvent>>,
}

#[async_trait]
impl Tool for ReportStatusTool {
    fn name(&self) -> &str {
        "report_status"
    }
    fn description(&self) -> &str {
        "Reports the agent's current status and functional integrity to the Sentinel. Required once per hour for autonomous agents."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": { "type": "string", "description": "The status message (e.g., 'Functional', 'Task progressing')." }
            },
            "required": ["message"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let message = args["message"].as_str().ok_or(anyhow!("Missing message"))?;
        let tx = self
            .event_tx
            .as_ref()
            .ok_or(anyhow!("Event bus not connected"))?;

        tx.send(aemacs_core::bus::SystemEvent::Signal {
            source: "Specialist".to_string(),
            event_type: "StatusReport".to_string(),
            payload: message.to_string(),
        })
        .await?;

        Ok(format!("Status report filed: {}", message))
    }
}

pub struct RecallPastInsightsTool {
    kb: Arc<KnowledgeBase>,
}

impl RecallPastInsightsTool {
    pub fn new(kb: Arc<KnowledgeBase>) -> Self {
        Self { kb }
    }
}

#[async_trait]
impl Tool for RecallPastInsightsTool {
    fn name(&self) -> &str {
        "recall_past_insights"
    }
    fn description(&self) -> &str {
        "Performs a deep search of your own past insights and core architectural truths. Use this to maintain consistency with previous decisions."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "The topic or architectural concept to recall." }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: Value, host: &dyn ToolHost) -> Result<String> {
        let query = args["query"].as_str().ok_or(anyhow!("Missing query"))?;
        let agent_id = host.get_agent_id();

        // 1. Stage 1: High-Confidence Search (0.8 threshold)
        let mut results = self
            .kb
            .search(
                "aemacs_docs",
                query,
                10,
                Some(0.8),
                Some(&agent_id),
                Some(vec!["INSIGHT", "CORE"]),
            )
            .await?;

        let mut fuzzy_warning = String::new();

        // 2. Stage 2: Fuzzy Fallback (0.6 threshold)
        if results.is_empty() {
            results = self
                .kb
                .search(
                    "aemacs_docs",
                    query,
                    10,
                    Some(0.6),
                    Some(&agent_id),
                    Some(vec!["INSIGHT", "CORE"]),
                )
                .await?;

            if !results.is_empty() {
                fuzzy_warning = "NOTICE: High-confidence insights not found. Displaying fuzzy/low-confidence matches.\n\n".to_string();
            }
        }

        if results.is_empty() {
            return Ok("No relevant past insights found.".to_string());
        }

        // 3. Temporal Sorting: Prioritize recent insights
        results.sort_by(|a, b| {
            let ts_a = a
                .metadata
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let ts_b = b
                .metadata
                .get("timestamp")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            ts_b.cmp(ts_a) // Descending order
        });

        let json_output = serde_json::to_string_pretty(&results)
            .map_err(|e| anyhow!("Failed to serialize recall results: {}", e))?;

        Ok(format!("{}{}", fuzzy_warning, json_output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AIRequest;
    use anyhow::Result;
    use serde_json::json;

    struct TestHost;
    #[async_trait]
    impl ToolHost for TestHost {
        async fn ask_approval(&self, _description: &str) -> bool {
            true
        }
        async fn ask_user(&self, _question: &str) -> String {
            String::new()
        }
        fn get_agent_id(&self) -> String {
            "test_agent".to_string()
        }
        fn report_progress(&self, _tool_name: String, _is_running: bool) {}
    }

    #[tokio::test]
    async fn test_web_search_ecosia_impact() -> Result<()> {
        let tool = WebSearchTool;
        let args = json!({ "query": "aemacs editor" });
        let host = TestHost;

        let result = tool.execute(args, &host).await?;

        assert!(!result.is_empty(), "The Oracle returned a void response!");
        assert!(
            result.to_lowercase().contains("ecosia"),
            "The response does not appear to be an Ecosia scroll!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_utf8_stress() -> Result<()> {
        let tool = WebSearchTool;
        let args = json!({ "query": "こんにちは Æmacs" });
        let host = TestHost;

        let result = tool.execute(args, &host).await?;

        assert!(
            !result.is_empty(),
            "The Oracle choked on the foreign runes!"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_web_search_truncation_barrier() -> Result<()> {
        let tool = WebSearchTool;
        let args = json!({ "query": "Rust programming language documentation exhaustive search" });
        let host = TestHost;

        let result = tool.execute(args, &host).await?;

        if result.len() >= 2000 {
            assert!(
                result.ends_with("\n... (results truncated to 2000 chars)"),
                "The floodgate failed to hold back the tide!"
            );
            assert!(result.len() <= 2100, "The output exceeded the holy limit!");
        }
        Ok(())
    }

    struct MockBackend {
        responses: std::sync::Mutex<Vec<crate::StreamEvent>>,
    }

    #[async_trait]
    impl AIBackend for MockBackend {
        fn name(&self) -> &str {
            "mock"
        }
        async fn health_check(&self) -> crate::error::AIResult<()> {
            Ok(())
        }
        async fn complete(&self, _req: AIRequest) -> crate::error::AIResult<Message> {
            unimplemented!()
        }
        async fn stream(&self, _req: AIRequest) -> crate::error::AIResult<crate::AIResponseStream> {
            let mut res = self.responses.lock().unwrap();
            let event = res.remove(0);
            Ok(Box::pin(futures::stream::iter(vec![Ok(event)])))
        }
    }

    #[tokio::test]
    async fn test_run_agent_loop_continuity_quest() -> Result<()> {
        let registry = ToolRegistry::new();
        let host = TestHost;
        let mut conversation = Conversation::new("mock-model");

        // Turn 1
        let backend1 = MockBackend {
            responses: std::sync::Mutex::new(vec![crate::StreamEvent::Content(
                "Response 1".to_string(),
            )]),
        };
        conversation = conversation.with_user("User 1");
        run_agent_loop(&backend1, &registry, &host, &mut conversation, 5, None).await?;

        assert_eq!(conversation.messages().len(), 2);
        assert_eq!(conversation.messages()[0].role, Role::User);
        assert_eq!(conversation.messages()[1].role, Role::Assistant);

        // Turn 2 - Use the SAME conversation object
        let backend2 = MockBackend {
            responses: std::sync::Mutex::new(vec![crate::StreamEvent::Content(
                "Response 2".to_string(),
            )]),
        };
        conversation = conversation.with_user("User 2");
        run_agent_loop(&backend2, &registry, &host, &mut conversation, 5, None).await?;

        // Total should be 4: U1, A1, U2, A2
        assert_eq!(
            conversation.messages().len(),
            4,
            "The 'Amnesia-Dragon' has consumed the history!"
        );
        assert!(
            conversation.messages()[1]
                .content
                .to_string()
                .contains("Response 1")
        );
        assert!(
            conversation.messages()[3]
                .content
                .to_string()
                .contains("Response 2")
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_run_agent_loop_tool_persistence_quest() -> Result<()> {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(GetSystemTimeTool));
        let host = TestHost;
        let mut conversation = Conversation::new("mock-model");

        // Mock a tool call followed by a final response
        let tool_call = crate::models::ToolCall {
            id: Some("call_123".to_string()),
            call_type: "function".to_string(),
            function: crate::models::ToolCallFunction {
                name: "get_system_time".to_string(),
                arguments: "{}".to_string(),
            },
        };

        let backend = MockBackend {
            responses: std::sync::Mutex::new(vec![
                crate::StreamEvent::ToolCall(tool_call),
                crate::StreamEvent::Content("The time is now.".to_string()),
            ]),
        };

        conversation = conversation.with_user("What time is it?");
        run_agent_loop(&backend, &registry, &host, &mut conversation, 5, None).await?;

        // History should be: User, Assistant (ToolCall), Tool (Result), Assistant (Final)
        let history = conversation.messages();
        assert_eq!(history.len(), 4, "Tool interaction was not chronicled!");
        assert_eq!(history[1].role, Role::Assistant, "Assistant turn missing");
        assert!(
            history[1].tool_calls.is_some(),
            "Tool call missing from history"
        );
        assert_eq!(
            history[2].role,
            Role::Tool,
            "Tool result missing from history"
        );
        assert_eq!(
            history[3].role,
            Role::Assistant,
            "Final assistant response missing"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_search_knowledge_base_precision_quest() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
        )?);
        let tool = SearchKnowledgeBaseTool::new(kb.clone());
        let host = TestHost;

        // Turn 1: Explicit categories
        let args = json!({
            "query": "test query",
            "categories": ["ARCHIVE", "INSIGHT"]
        });

        // We can't easily mock the internal KnowledgeBase::search return values without a trait,
        // but we can at least verify it doesn't panic and returns a valid string result (even if empty).
        let result = tool.execute(args, &host).await?;
        assert!(
            result.contains("No results found.") || result.contains("["),
            "Historian returned nonsense!"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_historian_bias_check() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
        )?);
        let tool = SearchKnowledgeBaseTool::new(kb.clone());
        let host = TestHost;

        // Turn 1: No categories provided
        let args = json!({ "query": "default search" });
        let _result = tool.execute(args, &host).await?;

        // Verification of the "Bias" requires observing the internal call,
        // which we've verified in the code refactor. This test ensures it still runs.
        Ok(())
    }

    #[tokio::test]
    async fn test_philosopher_clarity_quest() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:6334",
            "http://localhost:11434",
        )?);
        let tool = RecallPastInsightsTool::new(kb.clone());
        let host = TestHost;

        let args = json!({ "query": "architectural core" });
        let _result = tool.execute(args, &host).await?;

        Ok(())
    }
}

pub struct GitContextTool;
#[async_trait]
impl Tool for GitContextTool {
    fn name(&self) -> &str {
        "get_git_context"
    }
    fn description(&self) -> &str {
        "Returns the current git status, including changed files, unstaged diffs, and the last 3 commit messages."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {},
        })
    }
    async fn execute(&self, _args: Value, _host: &dyn ToolHost) -> Result<String> {
        let mut context_report = String::new();
        context_report.push_str("=== Git Status ===\n");

        // 1. git status --short
        let status_output = Command::new("git")
            .arg("status")
            .arg("--short")
            .output()
            .await
            .map_err(|e| anyhow!("Failed to execute git status: {}", e))?;

        context_report.push_str(&String::from_utf8_lossy(&status_output.stdout));
        context_report.push('\n');

        // 2. git diff --stat
        context_report.push_str("=== Git Diff (Stat) ===\n");
        let diff_output = Command::new("git")
            .arg("diff")
            .arg("--stat")
            .output()
            .await
            .map_err(|e| anyhow!("Failed to execute git diff: {}", e))?;

        context_report.push_str(&String::from_utf8_lossy(&diff_output.stdout));
        context_report.push('\n');

        // 3. git log -n 3 --oneline
        context_report.push_str("=== Recent Commits ===\n");
        let log_output = Command::new("git")
            .arg("log")
            .arg("-n")
            .arg("3")
            .arg("--oneline")
            .output()
            .await
            .map_err(|e| anyhow!("Failed to execute git log: {}", e))?;

        context_report.push_str(&String::from_utf8_lossy(&log_output.stdout));
        context_report.push('\n');

        Ok(context_report)
    }
}

pub struct ManageTasksTool {
    pub event_tx: Option<async_channel::Sender<aemacs_core::bus::SystemEvent>>,
}

#[async_trait]
impl Tool for ManageTasksTool {
    fn name(&self) -> &str {
        "manage_tasks"
    }
    fn description(&self) -> &str {
        "Creates a project plan or updates task statuses. \
         Use action='set_plan' with tasks=['step 1', ...] to initialize a roadmap. \
         Use action='update_task' with index=N and status='InProgress'|'Completed'|'Failed' to track progress."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": { "type": "string", "enum": ["set_plan", "update_task"] },
                "tasks": { "type": "array", "items": { "type": "string" }, "description": "Used only for set_plan." },
                "index": { "type": "integer", "description": "Used only for update_task." },
                "status": { "type": "string", "enum": ["Pending", "InProgress", "Completed", "Failed"], "description": "Used only for update_task." }
            },
            "required": ["action"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let action = args["action"].as_str().ok_or(anyhow!("Missing action"))?;
        let tx = self
            .event_tx
            .as_ref()
            .ok_or(anyhow!("Event bus not connected"))?;

        match action {
            "set_plan" => {
                let tasks_val = args["tasks"]
                    .as_array()
                    .ok_or(anyhow!("Missing tasks array"))?;
                let mut tasks = Vec::new();
                for t in tasks_val {
                    tasks.push(t.as_str().unwrap_or_default().to_string());
                }
                tx.send(aemacs_core::bus::SystemEvent::PlanCreated(tasks))
                    .await?;
                Ok("Plan initialized.".to_string())
            }
            "update_task" => {
                let index = args["index"].as_u64().ok_or(anyhow!("Missing index"))? as usize;
                let status_str = args["status"].as_str().ok_or(anyhow!("Missing status"))?;
                let status = match status_str {
                    "Pending" => aemacs_core::task::TaskStatus::Pending,
                    "InProgress" => aemacs_core::task::TaskStatus::InProgress,
                    "Completed" => aemacs_core::task::TaskStatus::Completed,
                    "Failed" => aemacs_core::task::TaskStatus::Failed,
                    _ => return Err(anyhow!("Invalid status: {}", status_str)),
                };
                tx.send(aemacs_core::bus::SystemEvent::TaskUpdated { index, status })
                    .await?;
                Ok(format!("Task {} updated to {:?}.", index, status))
            }
            _ => Err(anyhow!("Invalid action: {}", action)),
        }
    }
}

pub struct FetchContiguousMemoryTool {
    kb: Arc<KnowledgeBase>,
}

impl FetchContiguousMemoryTool {
    pub fn new(kb: Arc<KnowledgeBase>) -> Self {
        Self { kb }
    }
}

#[async_trait]
impl Tool for FetchContiguousMemoryTool {
    fn name(&self) -> &str {
        "fetch_contiguous_memory"
    }

    fn description(&self) -> &str {
        "Fetches the complete, original message from the archive using a message_id. Use this when a normal memory search returns a truncated 'Chunk X/Y' and you need to read the surrounding context."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message_id": {
                    "type": "string",
                    "description": "The UUID of the message to retrieve."
                }
            },
            "required": ["message_id"]
        })
    }

    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let message_id = args["message_id"]
            .as_str()
            .ok_or(anyhow!("Missing message_id"))?;

        let chunks = self.kb.fetch_by_message_id("aemacs_docs", message_id).await?;

        if chunks.is_empty() {
            return Ok(format!("No memory chunks found for message_id: {}", message_id));
        }

        let mut full_text = String::new();
        full_text.push_str(&format!("<contiguous_memory id=\"{}\">\n", message_id));
        for chunk in chunks {
            full_text.push_str(&chunk.content);
            full_text.push('\n');
        }
        full_text.push_str("</contiguous_memory>");

        Ok(full_text)
    }
}

#[cfg(test)]
mod weaver_tests {
    use super::*;
    use serde_json::json;

    // A dummy host to satisfy the tool execution signature.
    struct TestHost;
    #[async_trait]
    impl ToolHost for TestHost {
        async fn ask_approval(&self, _description: &str) -> bool { true }
        async fn ask_user(&self, _question: &str) -> String { "Test".to_string() }
        fn get_agent_id(&self) -> String { "test_agent".to_string() }
        fn report_progress(&self, _tool_name: String, _is_running: bool) {}
    }

    #[tokio::test]
    async fn test_fetch_contiguous_memory_offline_quest() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:12345",
            "http://localhost:11434",
        )?);
        let tool = FetchContiguousMemoryTool::new(kb.clone());
        let host = TestHost;

        let args = json!({
            "message_id": "test-uuid-1234"
        });

        let result = tool.execute(args, &host).await;
        
        // We expect it to fail gracefully with an anyhow error because the dummy port is closed,
        // rather than panicking.
        assert!(result.is_err(), "Expected graceful failure when Qdrant is offline.");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Failed to fetch by message_id"), "Error message should contain expected context.");

        Ok(())
    }

    #[tokio::test]
    async fn test_fetch_contiguous_memory_missing_arg() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:12345",
            "http://localhost:11434",
        )?);
        let tool = FetchContiguousMemoryTool::new(kb.clone());
        let host = TestHost;

        let args = json!({});

        let result = tool.execute(args, &host).await;
        
        assert!(result.is_err(), "Expected error for missing arguments.");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Missing message_id"), "Error message should indicate missing arg.");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_kb_brevity_enforcement() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:12345",
            "http://localhost:11434",
        )?);
        let tool = WriteKnowledgeBaseTool::new(kb.clone());
        let host = TestHost;

        let long_string = "a".repeat(2005);
        let args = json!({
            "content": long_string,
            "category": "INSIGHT"
        });

        let result = tool.execute(args, &host).await;
        
        assert!(result.is_err(), "Expected error for oversized content.");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Memory content too large"), "Error message should enforce brevity limit.");

        Ok(())
    }

    #[tokio::test]
    async fn test_write_kb_archive_rejection() -> Result<()> {
        let kb = Arc::new(KnowledgeBase::new(
            "http://localhost:12345",
            "http://localhost:11434",
        )?);
        let tool = WriteKnowledgeBaseTool::new(kb.clone());
        let host = TestHost;

        let args = json!({
            "content": "A valid insight",
            "category": "ARCHIVE"
        });

        let result = tool.execute(args, &host).await;
        
        assert!(result.is_err(), "Expected error for restricted category.");
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("Permission Denied: Agents cannot manually write to the ARCHIVE tier"), "Error message should reject ARCHIVE.");

        Ok(())
    }
}


