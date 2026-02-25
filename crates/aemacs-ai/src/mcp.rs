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
        registry.register(Box::new(DeleteMemoryTool::new(kb)));
        registry.register(Box::new(GetSystemTimeTool));
        registry.register(Box::new(ParseAstTool));
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

/// Executes the Agentic Loop: Talk to AI -> Execute Tools -> Talk to AI -> Result.
/// Modifies the conversation history in-place.
pub async fn run_agent_loop(
    backend: &impl AIBackend,
    registry: &ToolRegistry,
    host: &dyn ToolHost,
    conversation: &mut Conversation, // Mutable reference
    max_turns: usize,
) -> Result<String> {
    let tool_defs = registry.list_definitions();
    conversation.set_tools(tool_defs); // Use setter

    for _ in 0..max_turns {
        // Clone conversation for the request (snapshot of current state)
        let request = conversation.clone().build();
        let response_msg = backend.complete(request).await?;

        // ALWAYS add the assistant's response to history (whether text or tool call)
        conversation.add_message(response_msg.clone());

        if let Some(calls) = &response_msg.tool_calls {
            if calls.is_empty() {
                return Ok(response_msg.content.to_string());
            }

            for call in calls {
                let tool_name = &call.function.name;
                let args_str = &call.function.arguments;

                let result = match registry.get(tool_name) {
                    Some(tool) => match serde_json::from_str::<Value>(args_str) {
                        Ok(args) => match tool.execute(args, host).await {
                            Ok(output) => output,
                            Err(e) => format!("Error executing tool: {}", e),
                        },
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
            return Ok(response_msg.content.to_string());
        }
    }

    Err(anyhow!("Max turns reached without final response."))
}

// --- Utils ---

fn validate_path(path_str: &str) -> Result<PathBuf> {
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
        "Searches internal documentation/memory."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" },
                "collection": { "type": "string" }
            },
            "required": ["query"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let query = args["query"]
            .as_str()
            .ok_or(anyhow::anyhow!("Missing query"))?;
        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");
        let results = self.kb.search(collection, query, 3, None).await?;
        if results.is_empty() {
            return Ok("No results.".to_string());
        }
        
        let json_output = serde_json::to_string_pretty(&results)
            .map_err(|e| anyhow::anyhow!("Failed to serialize memory results: {}", e))?;
            
        Ok(json_output)
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
                "content": { "type": "string", "description": "The information to be remembered." },
                "category": { "type": "string", "enum": ["ARCHIVE", "INSIGHT", "CORE"], "description": "The tier of memory." },
                "collection": { "type": "string", "description": "Defaults to aemacs_docs." }
            },
            "required": ["content", "category"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let content = args["content"]
            .as_str()
            .ok_or(anyhow::anyhow!("Missing content"))?;
        let category = args["category"]
            .as_str()
            .ok_or(anyhow::anyhow!("Missing category"))?;
        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");

        let timestamp = chrono::Utc::now().to_rfc3339();
        let formatted_content = format!("[{}] [{}] | {}", category, timestamp, content);

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("timestamp".to_string(), timestamp);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.kb.add_document(collection, &formatted_content, Some(metadata)).await?;

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

        Ok(format!("Memory {} has been pruned from the collective.", id))
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
                "content": { "type": "string", "description": "The new information to be remembered." },
                "category": { "type": "string", "enum": ["ARCHIVE", "INSIGHT", "CORE"], "description": "The tier of memory." },
                "collection": { "type": "string", "description": "Defaults to aemacs_docs." }
            },
            "required": ["id", "content", "category"]
        })
    }
    async fn execute(&self, args: Value, _host: &dyn ToolHost) -> Result<String> {
        let id = args["id"].as_str().ok_or(anyhow!("Missing memory ID"))?;
        let content = args["content"].as_str().ok_or(anyhow!("Missing content"))?;
        let category = args["category"].as_str().ok_or(anyhow!("Missing category"))?;
        let collection = args["collection"].as_str().unwrap_or("aemacs_docs");

        let timestamp = chrono::Utc::now().to_rfc3339();
        let formatted_content = format!("[{}] [{}] | {}", category, timestamp, content);

        let mut metadata = HashMap::new();
        metadata.insert("category".to_string(), category.to_string());
        metadata.insert("timestamp".to_string(), timestamp);
        metadata.insert("type".to_string(), "active_memory".to_string());

        self.kb.update_point(collection, id, &formatted_content, Some(metadata)).await?;

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
        "Parses a Rust file using Tree-sitter and extracts the source code of a specific symbol (struct, enum, impl, or function) by name."
    }
    fn parameters(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Relative path to the Rust file." },
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
        let agent_name = args["agent_name"].as_str().ok_or(anyhow!("Missing agent_name"))?;
        let message = args["message"].as_str().map(|s| s.to_string());

        // Validate agent
        if self.persona_registry.get_persona(agent_name).await.is_none() {
            return Err(anyhow!("Specialist agent '{}' not found in registry.", agent_name));
        }

        let tx = self.event_tx.as_ref().ok_or(anyhow!("Event bus not connected"))?;

        tx.send(aemacs_core::bus::SystemEvent::PersonaChanged {
            name: agent_name.to_string(),
            message,
        }).await.context("Failed to send PersonaChanged event")?;

        Ok(format!("Handing off control to {}.", agent_name.to_uppercase()))
    }
}

pub struct WebSearchTool;
#[async_trait]
impl Tool for WebSearchTool {
    fn name(&self) -> &str {
        "web_search"
    }
    fn description(&self) -> &str {
        "Searches the internet for documentation or information. Requires AEMACS_SEARCH_API environment variable."
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

        let api_url = match std::env::var("AEMACS_SEARCH_API") {
            Ok(url) => url,
            Err(_) => {
                return Err(anyhow!(
                    "Search API not configured. Set AEMACS_SEARCH_API to use the Oracle."
                ));
            }
        };

        // Ensure we handle URL encoding
        let encoded_query = urlencoding::encode(query);
        let request_url = format!("{}?q={}", api_url, encoded_query);

        let response = reqwest::get(&request_url)
            .await
            .map_err(|e| anyhow!("Search request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(anyhow!(
                "Search API returned error status: {}",
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
        let tx = self.event_tx.as_ref().ok_or(anyhow!("Event bus not connected"))?;

        match action {
            "set_plan" => {
                let tasks_val = args["tasks"].as_array().ok_or(anyhow!("Missing tasks array"))?;
                let mut tasks = Vec::new();
                for t in tasks_val {
                    tasks.push(t.as_str().unwrap_or_default().to_string());
                }
                tx.send(aemacs_core::bus::SystemEvent::PlanCreated(tasks)).await?;
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
                tx.send(aemacs_core::bus::SystemEvent::TaskUpdated { index, status }).await?;
                Ok(format!("Task {} updated to {:?}.", index, status))
            }
            _ => Err(anyhow!("Invalid action: {}", action)),
        }
    }
}
