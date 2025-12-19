use anyhow::Result;
use log::{info, error};

use aemacs_core;
use aemacs_bridge;
use aemacs_gpui;
use aemacs_lsp;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Start the logger (RUST_LOG=info|debug controlls the log level)
    env_logger::init();

    info!("🚀 [APP] Æmacs Boot Sequence initiated.");

    // 2. Start the subsystems (Fail Fast: If one fails, all fail)
    if let Err(e) = boot_sequence().await {
        error!("💥 [APP] Critical System Failure: {}", e);
        // Clean exit with error code
        std::process::exit(1);
    }

    info!("✨ [APP] System fully operational. Waiting for Input.");

    // Here the event loop of GPUI would start and open the window
    // aemacs_gpui::run_app();

    Ok(())
}

/// Encapsulates the boot logic to easily propagate errors with '?'
async fn boot_sequence() -> Result<()> {
    // A. Core first (Config, State)
    aemacs_core::init()?;

    // B. Bridge (Python must run before loading plugins)
    aemacs_bridge::init()?;

    // C. LSP (Can wait in the background)
    aemacs_lsp::init()?;

    // D. UI Preparation
    aemacs_gpui::init()?;

    Ok(())
}
