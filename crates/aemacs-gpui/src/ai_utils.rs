use aemacs_ai::connectors::openai_compatible::OpenAICompatibleBackend;
use aemacs_ai::{AIBackend, AIRequest};
use aemacs_core::runtime::Tokio;
use futures::StreamExt;
use gpui::{AsyncApp, Context, Task, WeakEntity};

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
    // 1. Create the Channel to bridge the worlds
    // Unbounded is fine for chat tokens (low throughput).
    let (tx, rx) = async_channel::unbounded();

    // 2. Clone data to move into the Tokio (Backend) world
    let backend = backend.clone();
    let request = request.clone();

    // 3. Spawn the PRODUCER on the Global Tokio Runtime.
    // This runs on a background thread pool, completely detached from GPUI's executor.
    // Safe for blocking IO or heavy async work.
    let tokio_task = Tokio::spawn(cx, async move {
        match backend.stream(request).await {
            Ok(mut stream) => {
                while let Some(result) = stream.next().await {
                    let event = match result {
                        Ok(token) => StreamEvent::Chunk(token),
                        Err(e) => StreamEvent::Error(e.to_string()),
                    };

                    // If the receiver is closed (UI task dropped), we stop producing.
                    if tx.send(event).await.is_err() {
                        break;
                    }
                }
                // Send Done signal
                let _ = tx.send(StreamEvent::Done).await;
            }
            Err(e) => {
                let _ = tx.send(StreamEvent::Error(e.to_string())).await;
            }
        }
        // Channel tx drops here, closing the stream.
    });

    // 4. Spawn the CONSUMER on the GPUI Main/Background Executor.
    // This task has access to `this` (WeakEntity) and `cx` (AsyncContext).
    // We explicitly type 'this' to help type inference, but leave 'cx' inferred.
    // 'cx' here is expected to be `&mut AsyncApp` provided by GPUI's spawn.
    cx.spawn(move |this: WeakEntity<V>, cx: &mut AsyncApp| {
        let mut cx = cx.clone(); // Clone to break lifetime dependency
        async move {
            // Keep tokio_task alive by moving it here.
            // We don't await it directly because we want to process the channel stream.
            // But if this block finishes, tokio_task is dropped and aborted. Correct behavior.
            let _keep_alive = tokio_task;

            while let Ok(event) = rx.recv().await {
                // Update the UI on the Main Thread
                // cx is &mut AsyncApp. We must reborrow it to avoid moving the mutable reference.
                let update_result = this.update(&mut cx, |view, cx| {
                    update_fn(view, cx, event);
                });

                // If the entity is gone, stop consuming.
                if update_result.is_err() {
                    break;
                }
            }
        }
    })
}
