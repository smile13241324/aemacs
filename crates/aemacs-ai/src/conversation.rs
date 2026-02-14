use crate::{AIRequest, Message, Role, Content, ContentPart};
use crate::loader::load_file;
use std::path::Path;
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct Conversation {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    stream: bool,
}

impl Conversation {
    /// Starts a new conversation with a specific model.
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            messages: Vec::new(),
            temperature: 0.7, // Default
            stream: true,     // Default to streaming
        }
    }

    /// Adds a system prompt to the beginning of the conversation.
    pub fn with_system(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::System, s));
        self
    }

    /// Adds a system prompt combined with a file context.
    pub fn with_system_and_file(mut self, text: impl Into<String>, path: impl AsRef<Path>) -> Result<Self> {
        let file_part = load_file(path)?;
        let text_part = ContentPart::Text { text: text.into() };
        
        let content = Content::Parts(vec![text_part, file_part]);
        self.messages.push(Message {
            role: Role::System,
            content,
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
    pub fn with_user_with_image(mut self, text: impl Into<String>, image_url: impl Into<String>) -> Self {
        self.messages.push(Message::user_with_image(text, image_url));
        self
    }

    /// Adds a user message combined with a local file content.
    /// 
    /// This bundles the text prompt and the file (Image or Text) into a single
    /// multi-modal message.
    pub fn with_user_and_file(mut self, text: impl Into<String>, path: impl AsRef<Path>) -> Result<Self> {
        let file_part = load_file(path)?;
        let text_part = ContentPart::Text { text: text.into() };
        
        let content = Content::Parts(vec![text_part, file_part]);
        self.messages.push(Message {
            role: Role::User,
            content,
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
        });
        Ok(self)
    }

    /// Adds an assistant message.
    pub fn with_assistant(mut self, content: impl Into<String>) -> Self {
        let s: String = content.into();
        self.messages.push(Message::new(Role::Assistant, s));
        self
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

    /// Consumes the builder and returns the AIRequest.
    pub fn build(self) -> AIRequest {
        AIRequest {
            model: self.model,
            messages: self.messages,
            temperature: self.temperature,
            stream: self.stream,
        }
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new("mistral")
    }
}
