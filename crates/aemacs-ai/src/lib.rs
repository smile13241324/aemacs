pub mod connectors;
pub mod error;
pub mod models;

pub use error::{AIError, AIResult};
pub use models::{AIRequest, Message, Role};

use async_trait::async_trait;
use futures::stream::BoxStream;

pub type AIResponseStream = BoxStream<'static, AIResult<String>>;

#[async_trait]
pub trait AIBackend: Send + Sync {
    fn name(&self) -> &str;
    async fn health_check(&self) -> AIResult<()>;
    async fn complete(&self, request: AIRequest) -> AIResult<String>;
    async fn stream(&self, request: AIRequest) -> AIResult<AIResponseStream>;
}
