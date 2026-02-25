use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
                        ContentPart::ImageUrl { .. } => {}
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

    // The new temporal anchor:
    #[serde(default = "chrono::Utc::now")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl Message {
    pub fn new(role: Role, content: impl Into<Content>) -> Self {
        Self {
            role,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn system(content: impl Into<Content>) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: impl Into<Content>) -> Self {
        Self::new(Role::User, content)
    }

    pub fn assistant(content: impl Into<Content>) -> Self {
        Self::new(Role::Assistant, content)
    }

    pub fn user_with_image(text: impl Into<String>, image_url: impl Into<String>) -> Self {
        let parts = vec![
            ContentPart::Text { text: text.into() },
            ContentPart::ImageUrl {
                image_url: ImageUrl {
                    url: image_url.into(),
                },
            },
        ];
        Self {
            role: Role::User,
            content: Content::Parts(parts),
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
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

// --- Model Registry (ACO-035) ---

pub struct ModelDefinition {
    pub name: &'static str,
    pub label: &'static str,
    pub base_vram_gb: f32,
    pub kv_rate_gb_per_1k: f32,
    pub max_context: u32,
}

pub const MODELS: &[ModelDefinition] = &[
    ModelDefinition {
        name: "cognitivecomputations/dolphin-llama3.1",
        label: "Dolphin 8B",
        base_vram_gb: 5.0,
        kv_rate_gb_per_1k: 0.015,
        max_context: 131072,
    },
    ModelDefinition {
        name: "command-r",
        label: "Command R 35B",
        base_vram_gb: 20.0,
        kv_rate_gb_per_1k: 0.030,
        max_context: 131072,
    },
    ModelDefinition {
        name: "dolphin-llama3.1:70b",
        label: "Dolphin 70B",
        base_vram_gb: 40.0,
        kv_rate_gb_per_1k: 0.050,
        max_context: 131072,
    },
    ModelDefinition {
        name: "vanilj/midnight-miqu-70b-v1.5",
        label: "Midnight Miqu 70B",
        base_vram_gb: 40.0,
        kv_rate_gb_per_1k: 0.050,
        max_context: 32768,
    },
];

pub const CONTEXT_OPTIONS: &[u32] = &[4096, 8192, 16384, 32768, 65536, 131072];

