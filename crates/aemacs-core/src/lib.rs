pub mod buffer;
pub mod bus;
pub mod command;
pub mod editor;
pub mod keymap;
pub mod mode;
pub mod runtime;
pub mod selection;

use anyhow::Result;
use log::info;

pub use buffer::Buffer;
pub use bus::EventBus;
pub use command::Command;
pub use editor::Editor;
pub use keymap::KeymapRegistry;
pub use mode::Mode;
pub use selection::Selection;

/// Initiliase the Iron Core.
/// Here global states, configs and the buffer manager will be loaded later.
pub fn init() -> Result<()> {
    info!("⚙️ [CORE] Initializing System Kernel...");

    let version = env!("CARGO_PKG_VERSION");
    info!("⚙️ [CORE] Kernel Version {} ready.", version);

    Ok(())
}
