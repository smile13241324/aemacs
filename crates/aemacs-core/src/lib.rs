use anyhow::Result;
use log::info;

/// Initiliase the Iron Core.
/// Here global states, configs and the buffer manager will be loaded later.
pub fn init() -> Result<()> {
    info!("⚙️ [CORE] Initializing System Kernel...");

    let version = env!("CARGO_PKG_VERSION");
    info!("⚙️ [CORE] Kernel Version {} ready.", version);

    Ok(())
}
