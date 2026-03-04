use aemacs_ai::Conversation;
use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::mcp::{ToolHost, ToolRegistry, run_agent_loop};
use aemacs_ai::rag::KnowledgeBase;
use aemacs_core::runtime::Tokio;
use gpui::{AsyncApp, Context, Task, WeakEntity};

use std::sync::Arc;

pub enum AgentEvent {
    StreamChunk(String),
    ToolStarted(String),
    ToolFinished(String, bool),
    Result(Conversation),
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
    V: 'static,
    F: 'static + Send + Sync + Copy + Fn(&mut V, &mut Context<V>, AgentEvent),
    H: 'static + ToolHost + Send + Sync,
{
    let (tx, rx) = async_channel::unbounded::<AgentEvent>();
    let tx_for_stream = tx.clone();

    let tokio_task = Tokio::spawn(cx, async move {
        // Create a proxy channel to map LoopSignals to AgentEvents
        let (signal_tx, signal_rx) = async_channel::unbounded::<aemacs_ai::mcp::LoopSignal>();
        let tx_proxy = tx_for_stream.clone();

        tokio::spawn(async move {
            while let Ok(signal) = signal_rx.recv().await {
                let event = match signal {
                    aemacs_ai::mcp::LoopSignal::Text(chunk) => AgentEvent::StreamChunk(chunk),
                    aemacs_ai::mcp::LoopSignal::ToolCall(name) => AgentEvent::ToolStarted(name),
                    aemacs_ai::mcp::LoopSignal::ToolResult(name, success) => {
                        AgentEvent::ToolFinished(name, success)
                    }
                };
                let _ = tx_proxy.send(event).await;
            }
        });

        match run_agent_loop(
            &backend,
            &registry,
            &host,
            &mut conversation,
            10,
            Some(signal_tx),
        )
        .await
        {
            Ok(_result) => {
                // Final Weld: Automatically archive conversation to memory (ACO-025 integration)
                let session_id = uuid::Uuid::new_v4().to_string();
                if let Err(e) = conversation.archive_to_memory(&kb, &session_id).await {
                    log::error!("⚠️ [AI] Failed to archive conversation to memory: {}", e);
                }

                let _ = tx_for_stream.send(AgentEvent::Result(conversation)).await;
            }
            Err(e) => {
                let _ = tx_for_stream.send(AgentEvent::Error(e.to_string())).await;
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
