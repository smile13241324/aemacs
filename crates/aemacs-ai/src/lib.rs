pub mod connectors;
pub mod conversation;
pub mod embeddings;
pub mod error;
pub mod loader;
pub mod mcp;
pub mod models;
pub mod persona;
pub mod rag;
pub mod registry;

pub use conversation::Conversation;
pub use error::{AIError, AIResult};
pub use models::{AIRequest, Content, ContentPart, ImageUrl, Message, Role};
pub use persona::Persona;
pub use registry::PersonaRegistry;

use async_trait::async_trait;
use futures::stream::BoxStream;

pub type AIResponseStream = BoxStream<'static, AIResult<StreamEvent>>;

#[derive(Debug, Clone)]
pub enum StreamEvent {
    Content(String),
    ToolCall(models::ToolCall),
}

#[async_trait]
pub trait AIBackend: Send + Sync {
    fn name(&self) -> &str;
    async fn health_check(&self) -> AIResult<()>;
    async fn complete(&self, request: AIRequest) -> AIResult<Message>;
    async fn stream(&self, request: AIRequest) -> AIResult<AIResponseStream>;
}
