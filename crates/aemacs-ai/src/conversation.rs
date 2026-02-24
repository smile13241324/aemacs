use crate::loader::load_file;
use crate::{AIRequest, Content, ContentPart, Message, Role, models::OllamaOptions};
use anyhow::Result;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Conversation {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    stream: bool,
    options: OllamaOptions,
    tools: Option<Vec<serde_json::Value>>,
}

impl Conversation {
    /// Starts a new conversation with a specific model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            temperature: 0.7, // Default
            stream: true,     // Default to streaming
            options: OllamaOptions::default(),
            tools: None,
        }
    }

    /// Adds a system prompt to the beginning of the conversation.
    pub fn with_system(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::System, s));
        self
    }

    /// Adds a system prompt combined with a file context.
    pub fn with_system_and_file(
        mut self,
        text: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let file_part = load_file(path)?;
        let text_part = ContentPart::Text { text: text.into() };

        let content = Content::Parts(vec![text_part, file_part]);
        self.messages.push(Message {
            role: Role::System,
            content,
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        });
        Ok(self)
    }

    /// Adds a user message.
    pub fn with_user(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::User, s));
        self
    }

    /// Adds a user message with an image URL (for remote images).
    pub fn with_user_with_image(
        mut self,
        text: impl Into<String>,
        image_url: impl Into<String>,
    ) -> Self {
        self.messages
            .push(Message::user_with_image(text, image_url));
        self
    }

    /// Adds a user message combined with a local file content.
    pub fn with_user_and_file(
        mut self,
        text: impl Into<String>,
        path: impl AsRef<Path>,
    ) -> Result<Self> {
        let file_part = load_file(path)?;
        let text_part = ContentPart::Text { text: text.into() };

        let content = Content::Parts(vec![text_part, file_part]);
        self.messages.push(Message {
            role: Role::User,
            content,
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        });
        Ok(self)
    }

    /// Adds a file content as a standalone User message.
    pub fn with_file(mut self, path: impl AsRef<Path>) -> Result<Self> {
        let part = load_file(path)?;
        let content = Content::Parts(vec![part]);
        self.messages.push(Message {
            role: Role::User,
            content,
            tool_calls: None,
            tool_call_id: None,
            timestamp: chrono::Utc::now(),
        });
        Ok(self)
    }

    /// Adds an assistant message.
    pub fn with_assistant(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::Assistant, s));
        self
    }

    /// Manually adds a full message object.
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Sets the temperature (creativity).
    pub fn with_temperature(mut self, temp: f32) -> Self {
        self.temperature = temp;
        self
    }

    /// Enables or disables streaming (Default: true).
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }

    /// Sets the context window size (num_ctx).
    pub fn with_context_window(mut self, size: u32) -> Self {
        self.options.num_ctx = Some(size);
        self
    }

    /// Sets how long the model stays loaded in memory.
    pub fn with_keep_alive(mut self, duration: impl Into<String>) -> Self {
        self.options.keep_alive = Some(duration.into());
        self
    }

    /// Registers tools for the model to use (Builder pattern).
    pub fn with_tools(mut self, tools: Vec<serde_json::Value>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Sets tools on an existing mutable reference.
    pub fn set_tools(&mut self, tools: Vec<serde_json::Value>) {
        self.tools = Some(tools);
    }

    /// Archives the entire conversation history as individual episodic memory nodes in Qdrant.
    pub async fn archive_to_memory(
        &self,
        kb: &crate::rag::KnowledgeBase,
        session_id: &str,
    ) -> Result<()> {
        for (i, msg) in self.messages.iter().enumerate() {
            let mut metadata = std::collections::HashMap::new();
            metadata.insert("type".to_string(), "episodic_memory".to_string());
            metadata.insert("session_id".to_string(), session_id.to_string());
            metadata.insert("role".to_string(), format!("{:?}", msg.role));
            metadata.insert("timestamp".to_string(), msg.timestamp.to_rfc3339());
            metadata.insert("turn_index".to_string(), i.to_string());

            // Contextualize the chunk for the embedder
            let chunk = format!(
                "Session: {}\nTime: {}\nRole: {:?}\nContent: {}",
                session_id,
                msg.timestamp.to_rfc3339(),
                msg.role,
                msg.content
            );

            kb.add_document("aemacs_codebase", &chunk, Some(metadata))
                .await?;
        }
        Ok(())
    }

    /// Consumes the builder and returns the AIRequest.
    pub fn build(self) -> AIRequest {
        let messages = self
            .messages
            .into_iter()
            .map(|mut msg| {
                let timestamp_str = format!("[{}] ", msg.timestamp.to_rfc3339());
                msg.content = match msg.content {
                    Content::Text(text) => Content::Text(format!("{}{}", timestamp_str, text)),
                    Content::Parts(mut parts) => {
                        if !parts.is_empty() {
                            if let ContentPart::Text { text } = &mut parts[0] {
                                *text = format!("{}{}", timestamp_str, text);
                            } else {
                                parts.insert(
                                    0,
                                    ContentPart::Text {
                                        text: timestamp_str,
                                    },
                                );
                            }
                        } else {
                            parts.push(ContentPart::Text {
                                text: timestamp_str,
                            });
                        }
                        Content::Parts(parts)
                    }
                };
                msg
            })
            .collect();

        AIRequest {
            model: self.model,
            messages,
            temperature: self.temperature,
            stream: self.stream,
            options: Some(self.options),
            tools: self.tools,
        }
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new("mistral")
    }
}
