use gpui::{App, AppContext, Global, Task};
use std::future::Future;

pub use tokio::task::JoinError;

/// Initializes the Tokio wrapper using a new Tokio runtime.
pub fn init(cx: &mut App) {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            log::error!("Failed to initialize Tokio: {e}");
            std::process::exit(1);
        }
    };

    cx.set_global(GlobalTokio::new(RuntimeHolder::Owned(runtime)));
}

/// Initializes the Tokio wrapper using an existing handle.
pub fn init_from_handle(cx: &mut App, handle: tokio::runtime::Handle) {
    cx.set_global(GlobalTokio::new(RuntimeHolder::Shared(handle)));
}

#[derive(Debug)]
enum RuntimeHolder {
    Owned(tokio::runtime::Runtime),
    Shared(tokio::runtime::Handle),
}

impl RuntimeHolder {
    pub(crate) fn handle(&self) -> &tokio::runtime::Handle {
        match self {
            Self::Owned(runtime) => runtime.handle(),
            Self::Shared(handle) => handle,
        }
    }
}

#[derive(Debug)]
struct GlobalTokio {
    runtime: RuntimeHolder,
}

impl Global for GlobalTokio {}

impl GlobalTokio {
    const fn new(runtime: RuntimeHolder) -> Self {
        Self { runtime }
    }
}

#[derive(Debug)]
pub struct Tokio {}

impl Tokio {
    /// Acquires the unified Tokio runtime handle.
    pub fn handle<C: AppContext<Result<tokio::runtime::Handle> = tokio::runtime::Handle>>(
        cx: &C,
    ) -> tokio::runtime::Handle {
        cx.read_global(|tokio: &GlobalTokio, _| tokio.runtime.handle().clone())
    }

    pub fn spawn<C, Fut, R>(cx: &C, f: Fut) -> C::Result<Task<Result<R, JoinError>>>
    where
        C: AppContext,
        Fut: Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        cx.read_global(|tokio: &GlobalTokio, cx| {
            let join_handle = tokio.runtime.handle().spawn(f);
            let abort_handle = join_handle.abort_handle();

            let cancel = Defer(Some(move || {
                abort_handle.abort();
            }));

            cx.background_spawn(async move {
                let result = join_handle.await;
                drop(cancel);
                result
            })
        })
    }
}

struct Defer<F: FnOnce()>(Option<F>);
impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}
