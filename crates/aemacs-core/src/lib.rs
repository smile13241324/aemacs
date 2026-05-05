pub mod buffer;
pub mod bus;
pub mod command;
pub mod config;
pub mod editor;
pub mod keymap;
pub mod mode;
pub mod observer;
pub mod runtime;
pub mod selection;
pub mod sentinel;
pub mod signals;
pub mod syntax;
pub mod task;
pub mod triage;
pub mod watcher;

use anyhow::Result;
use log::info;

pub use buffer::Buffer;
pub use bus::EventBus;
pub use command::Command;
pub use editor::Editor;
pub use keymap::KeymapRegistry;
pub use mode::Mode;
pub use selection::Selection;

/// Initializes the Iron Core.
/// Here global states, configs and the buffer manager will be loaded later.
///
/// # Errors
/// Returns an error if the initialization fails.
pub fn init() -> Result<()> {
    info!("⚙️ [CORE] Initializing System Kernel...");

    let version = env!("CARGO_PKG_VERSION");
    info!("⚙️ [CORE] Kernel Version {version} ready.");

    Ok(())
}
