use anyhow::Result;
use log::info;

/// Initializes the Language Server Protocol client.
///
/// # Errors
/// Returns an error if the initialization fails.
/// Initializes the component.
///
/// # Errors
/// Returns an error if the initialization fails.
pub fn init() -> Result<()> {
    info!("📡 [LSP] Standing by for language server connections...");

    Ok(())
}
