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

    if !path.exists() {
        log::info!("No config.ron found at {:?}. Using default config.", path);
        return UserConfig::default();
    }

    match fs::read_to_string(&path) {
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
