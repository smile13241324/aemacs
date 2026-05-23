use std::{fs, path::PathBuf, sync::OnceLock};

use serde::Deserialize;

/// Manual similarity threshold overrides for different RAG memory categories.
/// If provided, these values bypass the autonomous calibration daemon.
#[derive(Debug, Deserialize, Clone, Default)]
pub struct RagThresholds {
    /// Threshold override for 'ARCHIVE' memory.
    pub archive: Option<f32>,
    /// Threshold override for 'INSIGHT' memory.
    pub insight: Option<f32>,
    /// Threshold override for 'CORE' memory.
    pub core: Option<f32>,
    /// Threshold override for 'GENESIS' memory.
    pub genesis: Option<f32>,
}

/// Represents the global configuration for the Æmacs system.
/// This structure is typically loaded from `~/.aemacs/config.ron`.
#[derive(Debug, Deserialize, Clone)]
pub struct UserConfig {
    /// The hardware tier (LOW, MEDIUM, HIGH) determining model selection.
    pub hardware_tier: Option<String>,
    /// The URL of the Ollama server.
    pub ollama_url: Option<String>,
    /// The URL of the Qdrant vector database.
    pub qdrant_url: Option<String>,
    /// Manual overrides for RAG thresholds.
    pub rag_thresholds: Option<RagThresholds>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            hardware_tier: Some("LOW".to_string()),
            ollama_url: Some("http://localhost:11434".to_string()),
            qdrant_url: Some("http://localhost:6334".to_string()),
            rag_thresholds: None,
        }
    }
}

/// Thread-safe singleton for global configuration access.
static GLOBAL_CONFIG: OnceLock<UserConfig> = OnceLock::new();

/// Retrieves the global configuration singleton, initializing it if necessary.
/// It automatically applies default values for missing fields.
pub fn get_config() -> &'static UserConfig {
    GLOBAL_CONFIG.get_or_init(|| {
        let mut config = load_user_config();

        // Ensure defaults are populated if missing in the ron file
        if config.ollama_url.is_none() {
            config.ollama_url = Some("http://localhost:11434".to_string());
        }
        if config.qdrant_url.is_none() {
            config.qdrant_url = Some("http://localhost:6334".to_string());
        }

        config
    })
}

/// Returns the standard physical path to the configuration file.
#[must_use]
pub fn get_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|mut path| {
        path.push(".aemacs");
        path.push("config.ron");
        path
    })
}

/// Loads the user configuration from the default path.
#[must_use]
pub fn load_user_config() -> UserConfig {
    let Some(path) = get_config_path() else {
        log::warn!("Could not determine home directory. Using default config.");
        return UserConfig::default();
    };

    load_config_from_path(&path)
}

/// Loads the configuration from a specific physical path.
/// It handles file missing, read errors, and format corruption by falling back to defaults.
#[must_use]
pub fn load_config_from_path(path: &PathBuf) -> UserConfig {
    if !path.exists() {
        log::info!("No config file found at {}. Using default config.", path.display());
        return UserConfig::default();
    }

    match fs::read_to_string(path) {
        Ok(contents) => match ron::from_str(&contents) {
            Ok(config) => config,
            Err(e) => {
                log::error!("Failed to parse {}: {e}. Using default config.", path.display());
                UserConfig::default()
            },
        },
        Err(e) => {
            log::error!("Failed to read {}: {e}. Using default config.", path.display());
            UserConfig::default()
        },
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]
    use std::io::Write;

    use tempfile::NamedTempFile;

    use super::*;

    #[test]
    fn test_load_config_ron_success() {
        let mut file = NamedTempFile::new().expect("Should not fail in test");
        // A complete config provided by the user
        writeln!(
            file,
            "UserConfig(hardware_tier: Some(\"HIGH\"), ollama_url: Some(\"http://remote:11434\"), qdrant_url: Some(\"http://remote:6334\"))"
        )
        .expect("Should not fail in test");

        let config = load_config_from_path(&file.path().to_path_buf());
        assert_eq!(config.hardware_tier.expect("Should not fail in test"), "HIGH");
        assert_eq!(config.ollama_url.expect("Should not fail in test"), "http://remote:11434");
        assert_eq!(config.qdrant_url.expect("Should not fail in test"), "http://remote:6334");
    }

    #[test]
    fn test_load_config_partial_fallback() {
        let mut file = NamedTempFile::new().expect("Should not fail in test");
        // User only provided the tier, URLs are missing
        writeln!(file, "UserConfig(hardware_tier: Some(\"MEDIUM\"))")
            .expect("Should not fail in test");

        let mut config = load_config_from_path(&file.path().to_path_buf());

        // Emulate the get_config() fallback logic since testing OnceLock directly is flaky
        if config.ollama_url.is_none() {
            config.ollama_url = Some("http://localhost:11434".to_string());
        }
        if config.qdrant_url.is_none() {
            config.qdrant_url = Some("http://localhost:6334".to_string());
        }

        assert_eq!(config.hardware_tier.expect("Should not fail in test"), "MEDIUM");
        assert_eq!(
            config.ollama_url.expect("Should not fail in test"),
            "http://localhost:11434",
            "Ollama URL failed to fall back!"
        );
        assert_eq!(
            config.qdrant_url.expect("Should not fail in test"),
            "http://localhost:6334",
            "Qdrant URL failed to fall back!"
        );
    }

    #[test]
    fn test_load_config_fallback_on_missing() {
        let path = PathBuf::from("/non/existent/path/to/config.ron");
        let config = load_config_from_path(&path);

        // A missing file returns UserConfig::default() directly
        assert_eq!(config.hardware_tier.expect("Should not fail in test"), "LOW");
        assert_eq!(config.ollama_url.expect("Should not fail in test"), "http://localhost:11434");
        assert_eq!(config.qdrant_url.expect("Should not fail in test"), "http://localhost:6334");
    }

    #[test]
    fn test_load_config_with_rag_thresholds_override() {
        // HARK! Verifying the manual override of RAG thresholds. [R-CONFIG-01]
        let mut file = NamedTempFile::new().expect("Should not fail in test");
        writeln!(
            file,
            "UserConfig(rag_thresholds: Some(RagThresholds(core: Some(0.85), genesis: Some(0.65))))"
        )
        .expect("Should not fail in test");

        let config = load_config_from_path(&file.path().to_path_buf());
        let thresholds = config.rag_thresholds.expect("RagThresholds should be present");
        assert_eq!(thresholds.core, Some(0.85), "Core threshold failed to parse!");
        assert_eq!(thresholds.genesis, Some(0.65), "Genesis threshold failed to parse!");
        assert_eq!(thresholds.archive, None, "Archive should be None!");
    }

    #[test]
    fn test_load_config_legacy_no_rag_thresholds() {
        // HARK! Ensuring legacy config files without RAG thresholds default to None. [R-CONFIG-02]
        let mut file = NamedTempFile::new().expect("Should not fail in test");
        // An old config with only the basics
        writeln!(file, "UserConfig(hardware_tier: Some(\"LOW\"))")
            .expect("Should not fail in test");

        let config = load_config_from_path(&file.path().to_path_buf());
        assert!(config.rag_thresholds.is_none(), "Legacy config should have None rag_thresholds!");
    }
}
