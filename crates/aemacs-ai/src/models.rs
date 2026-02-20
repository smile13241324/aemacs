use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "tool")] // New Role for Tool Outputs
    Tool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    Text(String),
    Parts(Vec<ContentPart>),
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::Text(s)
    }
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::Text(s.to_string())
    }
}

impl fmt::Display for Content {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Content::Text(s) => write!(f, "{}", s),
            Content::Parts(parts) => {
                for part in parts {
                    match part {
                        ContentPart::Text { text } => write!(f, "{}", text)?,
                        ContentPart::ImageUrl { .. } => {},
                    }
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    Text { text: String },
    ImageUrl { image_url: ImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageUrl {
    pub url: String,
}

// --- Tooling ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub call_type: String, // Usually "function"
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String, // JSON string
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: Content, // Content can be null for tool calls? No, usually empty string or null.
    // However, Rust needs a value. If deserializing from OpenAI, content might be null.
    // We handle this via custom deserializer or Option?
    // Let's make content optional? Or handle empty string.
    // For now, keep it Content, assuming text "" if null.
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    
    // For Tool Outputs:
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    pub fn new(role: Role, content: impl Into<Content>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    pub fn system(content: impl Into<Content>) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: impl Into<Content>) -> Self {
        Self::new(Role::User, content)
    }
    
    pub fn user_with_image(text: impl Into<String>, image_url: impl Into<String>) -> Self {
        let parts = vec![
            ContentPart::Text { text: text.into() },
            ContentPart::ImageUrl { 
                image_url: ImageUrl { url: image_url.into() } 
            }
        ];
        Self {
            role: Role::User,
            content: Content::Parts(parts),
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OllamaOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ctx: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keep_alive: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<OllamaOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>, // Tool Definitions
}

impl Default for AIRequest {
    fn default() -> Self {
        Self {
            model: "default".to_string(),
            messages: Vec::new(),
            temperature: 0.7,
            stream: true,
            options: None,
            tools: None,
        }
    }
}
