use anyhow::Result;
use log::info;
use pyo3::prelude::*;

/// Initializes the scripting bridge and verifies the Python connection.
pub fn init() -> Result<()> {
    info!("🌉 [BRIDGE] Connecting to Scripting Engines...");

    Python::attach(|py| -> PyResult<()> {
        let sys = py.import("sys")?;
        let version: String = sys.getattr("version")?.extract()?;

        info!("🐍 [BRIDGE] Python Runtime attached successfully!");

        let short_version = version.split_whitespace().next().unwrap_or("Unknown");
        info!("🐍 [BRIDGE] Version: {}", short_version);

        Ok(())
    }).map_err(|e| anyhow::anyhow!("Python Init Failed: {}", e))?;

    Ok(())
}
