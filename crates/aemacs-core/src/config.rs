use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)]
pub struct UserConfig {
    pub hardware_tier: Option<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            hardware_tier: Some("LOW".to_string()),
        }
    }
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
        writeln!(
            file,
            "UserConfig(hardware_tier: Some(\"HIGH\"))"
        )
        .unwrap();

        let config = load_config_from_path(&file.path().to_path_buf());
        assert_eq!(config.hardware_tier, Some("HIGH".to_string()));
    }

    #[test]
    fn test_load_config_fallback_on_missing() {
        let path = PathBuf::from("/non/existent/path/to/config.ron");
        let config = load_config_from_path(&path);
        // Default is LOW
        assert_eq!(config.hardware_tier, Some("LOW".to_string()));
    }

    #[test]
    fn test_load_config_fallback_on_invalid_ron() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Invalid(format: !![[") .unwrap();

        let config = load_config_from_path(&file.path().to_path_buf());
        assert_eq!(config.hardware_tier, Some("LOW".to_string()));
    }
}
