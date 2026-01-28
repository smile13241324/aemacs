use anyhow::Result;
use log::{error, info};

use aemacs_bridge;
use aemacs_core;
use aemacs_gpui;
use aemacs_lsp;

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialize the logger
    // RUST_LOG environment variable controls the verbosity (info, debug, trace).
    env_logger::init();

    info!("🚀 [APP] Æmacs Boot Sequence initiated.");

    // 2. Start subsystems (Fail Fast Strategy)
    // If any critical subsystem fails to load, we abort immediately to prevent undefined state.
    if let Err(e) = boot_sequence().await {
        error!("💥 [APP] Critical System Failure: {}", e);
        // Exit with error code 1 to signal failure to the OS/CI
        std::process::exit(1);
    }

    info!("✨ [APP] System fully operational. Handing over main thread to GPU Interface.");

    // CLI Argument Handling: Check for file to open
    let args: Vec<String> = std::env::args().collect();
    let file_to_open = if args.len() > 1 {
        let path = std::path::PathBuf::from(&args[1]);
        info!("📂 [CLI] Requesting to open file: {:?}", path);
        Some(path)
    } else {
        None
    };

    // 3. Launch the UI Event Loop
    // This blocks the main thread until the window is closed.
    aemacs_gpui::run_app(file_to_open);

    Ok(())
}

/// Encapsulates the system boot logic to easily propagate errors with '?'.
/// Ensures a deterministic startup order.
async fn boot_sequence() -> Result<()> {
    // A. Core System (Configs, Global State, Buffer Manager)
    aemacs_core::init()?;

    // B. Legacy Bridge (Python environment must be ready before loading plugins)
    aemacs_bridge::init()?;

    // C. LSP Subsystem (Language Servers can start in background)
    aemacs_lsp::init()?;

    // D. UI Preparation (Load assets, cache fonts, compile shaders)
    aemacs_gpui::init()?;

    Ok(())
}
