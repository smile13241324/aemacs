use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::mcp::{ToolHost, ToolRegistry, run_agent_loop};
use aemacs_ai::rag::KnowledgeBase;
use aemacs_ai::Conversation;
use aemacs_core::runtime::Tokio;
use gpui::{AsyncApp, Context, Task, WeakEntity};

use std::sync::Arc;

pub enum AgentEvent {
    Result(String),
    Error(String),
}

pub fn spawn_agent_task<V, F, H>(
    cx: &mut Context<V>,
    backend: OpenAICompatibleBackend,
    registry: Arc<ToolRegistry>,
    kb: Arc<KnowledgeBase>,
    host: H,
    mut conversation: Conversation,
    update_fn: F,
) -> Task<()>
where
    V: 'static + Send + Sync,
    F: 'static + Send + Sync + Copy + Fn(&mut V, &mut Context<V>, AgentEvent),
    H: 'static + ToolHost + Send + Sync,
{
    let (tx, rx) = async_channel::unbounded();

    let tokio_task = Tokio::spawn(cx, async move {
        match run_agent_loop(&backend, &registry, &host, &mut conversation, 10).await {
            Ok(result) => {
                // Final Weld: Automatically archive conversation to memory (ACO-025 integration)
                let session_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = conversation.archive_to_memory(&kb, &session_id).await {
                    log::error!("⚠️ [AI] Failed to archive conversation to memory: {}", e);
                }

                let _ = tx.send(AgentEvent::Result(result)).await;
            }
            Err(e) => {
                let _ = tx.send(AgentEvent::Error(e.to_string())).await;
            }
        }
    });

    cx.spawn(move |this: WeakEntity<V>, cx: &mut AsyncApp| {
        let mut cx = cx.clone();
        async move {
            let _keep_alive = tokio_task;
            while let Ok(event) = rx.recv().await {
                let update_result = this.update(&mut cx, |view, cx| {
                    update_fn(view, cx, event);
                });
                if update_result.is_err() {
                    break;
                }
            }
        }
    })
}
