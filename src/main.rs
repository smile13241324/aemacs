use anyhow::Result;
use log::info;

use aemacs_gpui;

fn main() -> Result<()> {
    // 1. Initialize the logger with a default 'info' level
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("🚀 [APP] Æmacs Boot Sequence initiated.");

    // CLI Argument Handling: Check for file to open
    let args: Vec<String> = std::env::args().collect();
    let file_to_open = if args.len() > 1 {
        let path = std::path::PathBuf::from(&args[1]);
        info!("📂 [CLI] Requesting to open file: {:?}", path);
        Some(path)
    } else {
        None
    };

    // 2. Launch the UI Event Loop and Subsystems
    // This blocks the main thread until the window is closed.
    // It now handles its own runtime and boot sequence internally.
    aemacs_gpui::run_app(file_to_open);

    Ok(())
}
