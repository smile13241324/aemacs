use anyhow::Result;
use log::info;

/// Initiate the Language Server Protocol client.
pub fn init() -> Result<()> {
    info!("📡 [LSP] Standing by for language server connections...");

    Ok(())
}
