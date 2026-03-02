use crate::error::AIError;
use crate::persona::Persona;
use notify::{EventKind, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// The PersonaRegistry indexes all available agents and watches for changes in the agents directory.
pub struct PersonaRegistry {
    personas: RwLock<HashMap<String, Persona>>,
    agents_dir: PathBuf,
    runtime_handle: tokio::runtime::Handle,
}

impl PersonaRegistry {
    /// Create a new registry and initialize the agents directory.
    pub async fn new(runtime_handle: tokio::runtime::Handle) -> Result<Arc<Self>, AIError> {
        // Priority 1: Current Working Directory
        let local_agents_dir = std::env::current_dir()?.join(".aemacs").join("agents");

        let agents_dir = if local_agents_dir.exists() {
            local_agents_dir
        } else {
            // Priority 2: Home Directory
            let home = dirs::home_dir()
                .ok_or_else(|| AIError::ConfigError("Could not find home directory".into()))?;
            home.join(".aemacs").join("agents")
        };

        if !agents_dir.exists() {
            std::fs::create_dir_all(&agents_dir)?;
            info!("Created agents directory: {:?}", agents_dir);
        }

        let registry = Arc::new(Self {
            personas: RwLock::new(HashMap::new()),
            agents_dir,
            runtime_handle,
        });

        // Initial load
        registry.load_all().await?;

        Ok(registry)
    }

    /// Load or reload all personas from the agents directory.
    pub async fn load_all(&self) -> Result<(), AIError> {
        let mut new_personas = HashMap::new();
        let entries = std::fs::read_dir(&self.agents_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("yaml") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    match std::fs::read_to_string(&path) {
                        Ok(content) => match Persona::from_yaml(&content) {
                            Ok(mut persona) => {
                                persona.name = name.to_string();
                                new_personas.insert(name.to_string(), persona);
                                info!("Loaded persona: {}", name);
                            }
                            Err(e) => warn!("Failed to parse persona YAML at {:?}: {}", path, e),
                        },
                        Err(e) => warn!("Failed to read persona file at {:?}: {}", path, e),
                    }
                }
            }
        }

        let mut lock = self.personas.write().await;
        *lock = new_personas;
        Ok(())
    }

    /// Retrieve a persona by name.
    pub async fn get_persona(&self, name: &str) -> Option<Persona> {
        let lock = self.personas.read().await;
        lock.get(name).cloned()
    }

    /// List all available personas.
    pub async fn list_personas(&self) -> Vec<String> {
        let lock = self.personas.read().await;
        lock.keys().cloned().collect()
    }

    /// Start watching the agents directory for changes.
    pub fn start_watching(self: Arc<Self>) -> Result<(), AIError> {
        let registry = self.clone();
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);

        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_) => {
                        let _ = tx.blocking_send(());
                    }
                    _ => {}
                }
            }
        })
        .map_err(|e| AIError::Persona(format!("Failed to create watcher: {}", e)))?;

        watcher
            .watch(&self.agents_dir, RecursiveMode::NonRecursive)
            .map_err(|e| AIError::Persona(format!("Failed to watch directory: {}", e)))?;

        // Keep the watcher alive in a background task
        self.runtime_handle.spawn(async move {
            // Keep a reference to the watcher so it doesn't get dropped
            let _watcher = watcher;
            while let Some(_) = rx.recv().await {
                info!("Agents directory changed, reloading personas...");
                if let Err(e) = registry.load_all().await {
                    error!("Failed to reload personas: {}", e);
                }
            }
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persona::Persona;
    use std::fs;

    #[tokio::test]
    async fn test_persona_registry_priority_quest() -> anyhow::Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let local_agents_dir = temp_dir.path().join(".aemacs").join("agents");
        fs::create_dir_all(&local_agents_dir)?;

        let test_persona =
            Persona::new("test-agent", "A test agent", "You are a test agent.", None);
        let yaml = serde_yaml::to_string(&test_persona)?;
        fs::write(local_agents_dir.join("test-agent.yaml"), yaml)?;

        // Force registry to use our temp dir as CWD for testing priority
        let current_dir = std::env::current_dir()?;
        std::env::set_current_dir(temp_dir.path())?;

        let registry = PersonaRegistry::new(tokio::runtime::Handle::current())
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create registry: {}", e))?;

        let personas = registry.list_personas().await;
        assert!(
            personas.contains(&"test-agent".to_string()),
            "Local test-agent was not loaded!"
        );

        // Restore CWD
        std::env::set_current_dir(current_dir)?;
        Ok(())
    }
}
