use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Role {
    #[serde(rename = "system")]
    System,
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
        }
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, content)
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, content)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIRequest {
    pub model: String,
    pub messages: Vec<Message>,
    pub temperature: f32,
    pub stream: bool,
}

impl Default for AIRequest {
    fn default() -> Self {
        Self {
            model: "default".to_string(),
            messages: Vec::new(),
            temperature: 0.7,
            stream: true,
        }
    }
}
