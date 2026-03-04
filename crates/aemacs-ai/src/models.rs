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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTier {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelRole {
    Logic,
    Creative,
    Roleplay,
}

pub struct ModelDefinition {
    pub name: &'static str,
    pub label: &'static str,
    pub tier: ModelTier,
    pub role: ModelRole,
    pub base_vram_gb: f32,
    pub kv_rate_gb_per_1k: f32,
    pub max_context: u32,
    pub license_constraints: &'static str,
    pub model_description: &'static str,
}

pub const MODELS: &[ModelDefinition] = &[
    // ==========================================
    // TIER: LOW (Laptops, < 8GB VRAM)
    // ==========================================
    ModelDefinition {
        name: "hermes3:8b-llama3.1-q4_K_M",
        label: "Hermes 3 8B",
        tier: ModelTier::Low,
        role: ModelRole::Logic,
        base_vram_gb: 5.0,
        kv_rate_gb_per_1k: 0.015,
        max_context: 131072,
        license_constraints: "Llama 3.1 Community License. Free for commercial and freelance use. Restriction only applies if your product exceeds 700 million monthly active users.",
        model_description: "The Surgical API. Engineered for strict JSON/XML tool execution and rigid system prompt obedience. It behaves less like a chatbot and more like a programmable state machine. Perfect for the 'Dispatcher' role to autonomously route fast, repetitive tasks.",
    },
    ModelDefinition {
        name: "dolphin3:8b",
        label: "Dolphin 3.0 8B",
        tier: ModelTier::Low,
        role: ModelRole::Creative,
        base_vram_gb: 5.0,
        kv_rate_gb_per_1k: 0.015,
        max_context: 131072,
        license_constraints: "Llama 3.1 Community License. Free for commercial and freelance use. Restriction only applies if your product exceeds 700 million monthly active users.",
        model_description: "The Unrestricted Sparring Partner. Built on Llama 3.1 but explicitly fine-tuned for high emotional intelligence, creative lateral thinking, and zero refusals. The perfect lightweight choice for local, highly conversational Pair-Programming and deep architectural brainstorming without moralizing guardrails.",
    },
    ModelDefinition {
        name: "stheno:8b",
        label: "Stheno 3.2 8B",
        tier: ModelTier::Low,
        role: ModelRole::Roleplay,
        base_vram_gb: 5.0,
        kv_rate_gb_per_1k: 0.015,
        max_context: 8192,
        license_constraints: "Llama 3 Community License. Free for commercial and freelance use. Restriction only applies if your product exceeds 700 million monthly active users.",
        model_description: "The Method Actor (Small). Explicitly fine-tuned for multi-turn roleplay and absolute persona adherence. It will adopt any system-prompt identity (e.g., a cynical senior architect) and never break character. Uncensored, with highly natural, expressive prose for its size.",
    },
    // ==========================================
    // TIER: MEDIUM (Workstations, ~12-26GB VRAM)
    // ==========================================
    ModelDefinition {
        name: "mixtral:8x7b-instruct-v0.1-q4_K_M",
        label: "Mixtral 8x7B Instruct",
        tier: ModelTier::Medium,
        role: ModelRole::Logic,
        base_vram_gb: 26.0,
        kv_rate_gb_per_1k: 0.025,
        max_context: 32768,
        license_constraints: "Apache 2.0 License. 100% free for unrestricted commercial and enterprise use.",
        model_description: "The Efficient Polymath. Utilizes a Mixture of Experts (MoE) architecture to provide the reasoning power of a massive model while remaining highly performant. Exceptional at processing large RAG contexts, maintaining deep knowledge base memory, and strict tool execution.",
    },
    ModelDefinition {
        name: "dolphin-mixtral:8x7b",
        label: "Dolphin Mixtral 8x7B",
        tier: ModelTier::Medium,
        role: ModelRole::Creative,
        base_vram_gb: 26.0,
        kv_rate_gb_per_1k: 0.025,
        max_context: 32768,
        license_constraints: "Apache 2.0 License. 100% free for unrestricted commercial and enterprise use.",
        model_description: "The Free-Thinking Polymath. Takes the massive MoE efficiency of Mixtral and completely removes all alignment constraints. It delivers the deep, nuanced conversational abilities and creative problem-solving of a much larger model, acting as an incredibly intelligent technical oracle.",
    },
    ModelDefinition {
        name: "magnum:12b",
        label: "Magnum v2 12B",
        tier: ModelTier::Medium,
        role: ModelRole::Roleplay,
        base_vram_gb: 7.5,
        kv_rate_gb_per_1k: 0.025,
        max_context: 128000,
        license_constraints: "Apache 2.0 License. 100% free for unrestricted commercial and enterprise use.",
        model_description: "The Contextual Method Actor. Built on Mistral Nemo, offering a massive 128k context window crucial for maintaining long-running personas and deep narrative states without forgetting details. Renowned for generating vivid text and staying flawlessly in character across massive sessions.",
    },
    // ==========================================
    // TIER: HIGH (Heavy Compute, 40GB+ VRAM)
    // ==========================================
    ModelDefinition {
        name: "hermes3:70b-llama3.1-q4_K_M",
        label: "Hermes 3 70B",
        tier: ModelTier::High,
        role: ModelRole::Logic,
        base_vram_gb: 40.0,
        kv_rate_gb_per_1k: 0.050,
        max_context: 131072,
        license_constraints: "Llama 3.1 Community License. Free for commercial and enterprise use.",
        model_description: "The Ultimate Orchestrator. Scales the surgical, system-obedient nature of Hermes to a massive 70B parameter space. The definitive choice for coordinating massive, complex tool chains (like nested bash scripts and AST manipulations) with zero alignment constraints.",
    },
    ModelDefinition {
        name: "llama3.3:70b",
        label: "Llama 3.3 70B",
        tier: ModelTier::High,
        role: ModelRole::Creative,
        base_vram_gb: 40.0,
        kv_rate_gb_per_1k: 0.050,
        max_context: 131072,
        license_constraints: "Llama 3.3 Community License. Free for commercial and enterprise use.",
        model_description: "The SOTA Powerhouse. Distills the reasoning capabilities of Meta's massive 405B model into a highly efficient 70B footprint. While fully aligned, its sheer intelligence makes it the ultimate conversational sparring partner for complex code refactoring, deep architectural planning, and lateral problem-solving.",
    },
    ModelDefinition {
        name: "euryale:70b",
        label: "Euryale v2.2 70B",
        tier: ModelTier::High,
        role: ModelRole::Roleplay,
        base_vram_gb: 40.0,
        kv_rate_gb_per_1k: 0.050,
        max_context: 131072,
        license_constraints: "Llama 3.1 Community License. Free for commercial and enterprise use.",
        model_description: "The Enterprise Persona. The absolute pinnacle of open-weights roleplay. It provides the deep, lateral thinking and conversational nuance of legacy models like Miqu, but is built on a clean Llama 3.1 foundation, making it fully compliant and safe for corporate environments.",
    },
];

pub const CONTEXT_OPTIONS: &[u32] = &[4096, 8192, 16384, 32768, 65536, 131072];

pub fn get_models_for_tier(tier_str: &str) -> Vec<&'static ModelDefinition> {
    let target_tier = match tier_str.to_uppercase().as_str() {
        "LOW" => ModelTier::Low,
        "MEDIUM" => ModelTier::Medium,
        "HIGH" => ModelTier::High,
        _ => {
            tracing::warn!("Unknown hardware tier '{}', defaulting to LOW", tier_str);
            ModelTier::Low
        }
    };

    MODELS.iter().filter(|m| m.tier == target_tier).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_models_for_tier_high() {
        let models = get_models_for_tier("HIGH");
        assert!(!models.is_empty(), "Should return high-tier models.");
        for model in models {
            assert_eq!(model.tier, ModelTier::High);
        }
    }

    #[test]
    fn test_get_models_for_tier_case_insensitive() {
        let models = get_models_for_tier("medium");
        assert!(!models.is_empty());
        for model in models {
            assert_eq!(model.tier, ModelTier::Medium);
        }
    }

    #[test]
    fn test_get_models_for_tier_invalid_fallback() {
        // Fallback to LOW
        let models = get_models_for_tier("TOASTER");
        assert!(!models.is_empty());
        for model in models {
            assert_eq!(model.tier, ModelTier::Low);
        }
    }
}

