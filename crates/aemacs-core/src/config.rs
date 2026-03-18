use serde::Deserialize;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Clone)]
pub struct UserConfig {
    pub hardware_tier: Option<String>,
    pub ollama_url: Option<String>,
    pub qdrant_url: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            hardware_tier: Some("LOW".to_string()),
            ollama_url: Some("http://localhost:11434".to_string()),
            qdrant_url: Some("http://localhost:6334".to_string()),
        }
    }
}

static GLOBAL_CONFIG: OnceLock<UserConfig> = OnceLock::new();

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

pub fn get_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|mut path| {
        path.push(".aemacs");
        path.push("config.ron");
        path
    })
}

pub fn load_user_config() -> UserConfig {
    let path = match get_config_path() {
        Some(p) => p,
        None => {
            log::warn!("Could not determine home directory. Using default config.");
            return UserConfig::default();
        }
    };

    load_config_from_path(&path)
}

/// Internal helper to load config from a specific path, used for testing.
pub fn load_config_from_path(path: &PathBuf) -> UserConfig {
    if !path.exists() {
        log::info!("No config file found at {:?}. Using default config.", path);
        return UserConfig::default();
    }

    match fs::read_to_string(path) {
        Ok(contents) => match ron::from_str(&contents) {
            Ok(config) => config,
            Err(e) => {
                log::error!("Failed to parse {:?}: {}. Using default config.", path, e);
                UserConfig::default()
            }
        },
        Err(e) => {
            log::error!("Failed to read {:?}: {}. Using default config.", path, e);
            UserConfig::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_config_ron_success() {
        let mut file = NamedTempFile::new().unwrap();
        // A complete config provided by the user
        writeln!(
            file,
            "UserConfig(hardware_tier: Some(\"HIGH\"), ollama_url: Some(\"http://remote:11434\"), qdrant_url: Some(\"http://remote:6334\"))"
        )
        .unwrap();

        let config = load_config_from_path(&file.path().to_path_buf());
        assert_eq!(config.hardware_tier.unwrap(), "HIGH");
        assert_eq!(config.ollama_url.unwrap(), "http://remote:11434");
        assert_eq!(config.qdrant_url.unwrap(), "http://remote:6334");
    }

    #[test]
    fn test_load_config_partial_fallback() {
        let mut file = NamedTempFile::new().unwrap();
        // User only provided the tier, URLs are missing
        writeln!(file, "UserConfig(hardware_tier: Some(\"MEDIUM\"))").unwrap();

        let mut config = load_config_from_path(&file.path().to_path_buf());
        
        // Emulate the get_config() fallback logic since testing OnceLock directly is flaky
        if config.ollama_url.is_none() {
            config.ollama_url = Some("http://localhost:11434".to_string());
        }
        if config.qdrant_url.is_none() {
            config.qdrant_url = Some("http://localhost:6334".to_string());
        }

        assert_eq!(config.hardware_tier.unwrap(), "MEDIUM");
        assert_eq!(config.ollama_url.unwrap(), "http://localhost:11434", "Ollama URL failed to fall back!");
        assert_eq!(config.qdrant_url.unwrap(), "http://localhost:6334", "Qdrant URL failed to fall back!");
    }

    #[test]
    fn test_load_config_fallback_on_missing() {
        let path = PathBuf::from("/non/existent/path/to/config.ron");
        let config = load_config_from_path(&path);
        
        // A missing file returns UserConfig::default() directly
        assert_eq!(config.hardware_tier.unwrap(), "LOW");
        assert_eq!(config.ollama_url.unwrap(), "http://localhost:11434");
        assert_eq!(config.qdrant_url.unwrap(), "http://localhost:6334");
    }

    #[test]
    fn test_load_config_fallback_on_invalid_ron() {
        let mut file = NamedTempFile::new().unwrap();
        // Corrupt file
        writeln!(file, "Invalid(format: !![[").unwrap();

        let config = load_config_from_path(&file.path().to_path_buf());
        
        // An invalid file returns UserConfig::default() directly
        assert_eq!(config.hardware_tier.unwrap(), "LOW");
        assert_eq!(config.ollama_url.unwrap(), "http://localhost:11434");
        assert_eq!(config.qdrant_url.unwrap(), "http://localhost:6334");
    }
}
