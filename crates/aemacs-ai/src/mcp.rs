use crate::rag::KnowledgeBase;
use crate::{AIBackend, AIRequest, Content, ContentPart, Conversation, Message, Role};
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

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

    pub fn with_core_tools(kb: Arc<KnowledgeBase>) -> Self {
        let mut registry = Self::new();
        registry.register(Box::new(ReadFileTool));
        registry.register(Box::new(WriteFileTool));
        registry.register(Box::new(ListFilesTool));
        registry.register(Box::new(SearchKnowledgeBaseTool::new(kb)));
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

pub struct WriteFileTool;
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

        Ok(format!("Successfully wrote to {:?}", path))
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
        Ok(results.join("\n\n---\n\n"))
    }
}
