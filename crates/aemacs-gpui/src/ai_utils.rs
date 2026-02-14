use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::{AIBackend, AIRequest};
use futures::StreamExt;
use gpui::{Context, Task};

pub enum StreamEvent {
    Chunk(String),
    Error(String),
    Done,
}

pub fn spawn_chat_stream<V, F>(
    cx: &mut Context<V>,
    backend: OpenAICompatibleBackend,
    request: AIRequest,
    update_fn: F,
) -> Task<()>
where
    V: 'static + Send + Sync,
    F: 'static + Send + Sync + Copy + Fn(&mut V, &mut Context<V>, StreamEvent),
{
    return cx.spawn(move |this, mut cx| async move {
        match backend.stream(request).await {
            Ok(mut stream) => {
                while let Some(result) = stream.next().await {
                    let event = match result {
                        Ok(token) => StreamEvent::Chunk(token),
                        Err(e) => StreamEvent::Error(e.to_string()),
                    };

                    // Use 'this' (WeakEntity) provided by spawn
                    // Remove leading & from cx as learned from Marjin's error
                    let _ = this.update(cx, |view, cx| {
                        update_fn(view, cx, event);
                    });
                }

                let _ = this.update(cx, |view, cx| {
                    update_fn(view, cx, StreamEvent::Done);
                });
            }
            Err(e) => {
                let _ = this.update(cx, |view, cx| {
                    update_fn(view, cx, StreamEvent::Error(e.to_string()));
                });
            }
        }
    });
}
